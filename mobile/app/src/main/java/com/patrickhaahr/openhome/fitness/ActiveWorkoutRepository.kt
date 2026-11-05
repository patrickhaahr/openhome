package com.patrickhaahr.openhome.fitness

import androidx.room.withTransaction
import com.patrickhaahr.openhome.api.ApiSet
import com.patrickhaahr.openhome.storage.ActiveEntryEntity
import com.patrickhaahr.openhome.storage.ActiveEntryWithDetails
import com.patrickhaahr.openhome.storage.ActiveSetEntity
import com.patrickhaahr.openhome.storage.ActiveWorkoutEntity
import com.patrickhaahr.openhome.storage.ActiveWorkoutWithEntries
import com.patrickhaahr.openhome.storage.EntryWithDetails
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.storage.WorkoutEntity
import com.patrickhaahr.openhome.storage.WorkoutEntryEntity
import com.patrickhaahr.openhome.storage.WorkoutSetEntity
import com.patrickhaahr.openhome.storage.WorkoutWithEntries
import com.patrickhaahr.openhome.sync.FitnessOutbox
import com.patrickhaahr.openhome.sync.FitnessSyncScheduler
import com.patrickhaahr.openhome.sync.QueuedEntry
import com.patrickhaahr.openhome.sync.QueuedWorkout
import java.math.BigDecimal
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * The one Active Workout, stored on every change. Each function returns after its change committed.
 * Writes run strictly in call order, so fast typing cannot be saved out of order and finishing always
 * sees every edit made before it.
 */
class ActiveWorkoutRepository(
    private val database: OpenHomeDatabase,
    private val outbox: FitnessOutbox,
    private val scheduler: FitnessSyncScheduler,
) {
    private val dao = database.activeWorkoutDao()
    private val fitnessDao = database.fitnessDao()
    private val writes = Mutex()

    fun observe(): Flow<ActiveWorkout?> = dao.observe().map { it?.toDomain() }

    /** Does nothing when an Active Workout already exists, so it can never strand or replace one. */
    suspend fun startEmpty(date: String) = write {
        dao.insertWorkout(ActiveWorkoutEntity(date = date, name = "", notes = ""))
    }

    /**
     * Starts a Repeated Workout on [date]: the past Workout's name, Exercise order and Set values, with
     * nothing confirmed. Returns whether an Active Workout now exists; one that already did is left
     * exactly as it was, so a second tap can never append another copy to it.
     */
    suspend fun repeat(workoutId: Long, date: String): Boolean = write {
        if (dao.load() != null) return@write true
        val source = fitnessDao.loadWorkout(workoutId) ?: return@write false
        dao.insertWorkout(ActiveWorkoutEntity(date = date, name = source.workout.name.orEmpty(), notes = ""))
        val entryIds = source.orderedEntries().mapIndexed { index, row -> copyEntry(row, index.toLong()) }
        dao.setCurrentEntry(entryIds.firstOrNull())
        true
    }

    private suspend fun copyEntry(row: EntryWithDetails, orderIndex: Long): Long {
        val entryId = dao.insertEntry(ActiveEntryEntity(exerciseId = row.entry.exerciseId, orderIndex = orderIndex))
        val sets = row.sets.sortedWith(compareBy({ it.setNumber }, { it.id }))
        sets.forEachIndexed { index, set -> dao.insertSet(set.toUnconfirmedRow(entryId, index + 1L)) }
        // The API allows an entry without Sets; logging needs a row to start from.
        if (sets.isEmpty()) {
            dao.insertSet(ActiveSetEntity(entryId = entryId, setNumber = 1, measure = lastMeasureOf(row.entry.exerciseId)))
        }
        return entryId
    }

    suspend fun goToEntry(entryId: Long) = write { dao.setCurrentEntry(entryId) }

    suspend fun setDate(date: String) = write { dao.setDate(date) }

    suspend fun setName(name: String) = write { dao.setName(name) }

    suspend fun setNotes(notes: String) = write { dao.setNotes(notes) }

    suspend fun addExercise(exerciseId: Long) = write {
        val entryId = dao.insertEntry(ActiveEntryEntity(exerciseId = exerciseId, orderIndex = dao.nextOrderIndex()))
        dao.insertSet(ActiveSetEntity(entryId = entryId, setNumber = 1, measure = lastMeasureOf(exerciseId)))
        dao.setCurrentEntry(entryId)
    }

    /** The focus stays where it is, unless it was on this Exercise and moves to its neighbour. */
    suspend fun removeEntry(entryId: Long) = write {
        val active = dao.load() ?: return@write
        val ordered = active.orderedEntries()
        val index = ordered.indexOfFirst { it.entry.id == entryId }
        if (index < 0) return@write
        dao.deleteEntry(entryId)
        if (active.workout.currentEntryId == entryId) {
            dao.setCurrentEntry((ordered.getOrNull(index + 1) ?: ordered.getOrNull(index - 1))?.entry?.id)
        }
    }

    /** A new row continues the measure of the row before it, and starts unconfirmed. */
    suspend fun addSet(entryId: Long) = write {
        val last = dao.loadSets(entryId).last()
        dao.insertSet(ActiveSetEntity(entryId = entryId, setNumber = last.setNumber + 1, measure = last.measure))
    }

    /** A new row with the preceding row's values exactly as typed, not yet confirmed. */
    suspend fun repeatLastSet(entryId: Long) = write {
        val last = dao.loadSets(entryId).last()
        dao.insertSet(last.copy(id = 0, setNumber = last.setNumber + 1, completed = false))
    }

    /** For doing fewer Sets than planned. An Exercise keeps its one row; removing it is another action. */
    suspend fun removeLastSet(entryId: Long) = write {
        val sets = dao.loadSets(entryId)
        if (sets.size > 1) dao.deleteSet(sets.last().id)
    }

    suspend fun setMeasure(entryId: Long, measure: SetMeasure) = write {
        dao.loadSets(entryId).forEach { saveKeepingConfirmationHonest(it.copy(measure = measure)) }
    }

    suspend fun editSet(setId: Long, field: SetField, text: String) = write {
        val set = dao.loadSet(setId) ?: return@write
        saveKeepingConfirmationHonest(
            when (field) {
                SetField.REPS -> set.copy(reps = text)
                SetField.ADDED_WEIGHT -> set.copy(weightKg = text)
                SetField.DURATION -> set.copy(durationSeconds = text)
                SetField.RPE -> set.copy(rpe = text)
            },
        )
    }

    /** Returns false, changing nothing, when a row that does not parse is being confirmed. */
    suspend fun setCompleted(setId: Long, completed: Boolean): Boolean = write {
        val set = dao.loadSet(setId) ?: return@write false
        if (completed && set.draft().check() !is SetCheck.Valid) return@write false
        dao.updateSet(set.copy(completed = completed))
        true
    }

    /**
     * The explicit confirmation of an Exercise: in one transaction, every row of [entryId] that parses
     * becomes performed and the focus advances to the next Exercise with an unconfirmed row, wrapping
     * around. With no row to confirm nothing changes, so the user stays to see what is missing.
     */
    suspend fun completeExercise(entryId: Long): ExerciseCompletion = write {
        val (valid, refused) = dao.loadSets(entryId).partition { it.draft().check() is SetCheck.Valid }
        val unconfirmed = valid.filterNot { it.completed }
        unconfirmed.forEach { dao.updateSet(it.copy(completed = true)) }
        if (valid.isNotEmpty()) nextUnconfirmedEntryAfter(entryId)?.let { dao.setCurrentEntry(it) }
        ExerciseCompletion(confirmed = unconfirmed.size, refused = refused.mapTo(mutableSetOf()) { it.id })
    }

    private suspend fun nextUnconfirmedEntryAfter(entryId: Long): Long? {
        val ordered = dao.load()?.orderedEntries().orEmpty()
        val index = ordered.indexOfFirst { it.entry.id == entryId }
        return (ordered.drop(index + 1) + ordered.take(index))
            .firstOrNull { row -> row.sets.any { !it.completed } }
            ?.entry
            ?.id
    }

    suspend fun discard() = write { dao.deleteWorkout() }

    /**
     * Records only confirmed rows, in one transaction with queuing the sync and ending the Active
     * Workout. Unchecked rows are left out only when [omitUnchecked] says the user chose that.
     */
    suspend fun finish(omitUnchecked: Boolean): FinishOutcome {
        val outcome =
            write {
                val active = dao.load() ?: return@write FinishOutcome.NoActiveWorkout
                val performed = active.performedEntries()
                val rows = active.entries.sumOf { it.sets.size }
                val confirmed = performed.sumOf { it.sets.size }
                when {
                    confirmed == 0 -> FinishOutcome.NothingConfirmed
                    confirmed < rows && !omitUnchecked -> FinishOutcome.HasUncheckedSets
                    else -> FinishOutcome.Finished(record(active.workout, performed))
                }
            }
        // After the commit: if the process dies first, the next app start schedules it from the outbox.
        if (outcome is FinishOutcome.Finished) scheduler.schedule()
        return outcome
    }

    private suspend fun record(active: ActiveWorkoutEntity, performed: List<PerformedEntry>): Long {
        val name = active.name.trim().ifEmpty { null }
        val notes = active.notes.trim().ifEmpty { null }
        val workoutId =
            fitnessDao.insertWorkout(WorkoutEntity(serverId = null, date = active.date, name = name, notes = notes))
        val queued =
            performed.mapIndexed { index, entry ->
                val orderIndex = index.toLong()
                val entryId =
                    fitnessDao.insertEntry(
                        WorkoutEntryEntity(
                            workoutId = workoutId,
                            exerciseId = entry.exerciseId,
                            orderIndex = orderIndex,
                            notes = null,
                        ),
                    )
                val sets =
                    entry.sets.mapIndexed { setIndex, set ->
                        ApiSet(
                            setNumber = setIndex + 1L,
                            reps = set.reps,
                            weightKg = set.addedWeightKg,
                            durationSeconds = set.durationSeconds,
                            rpe = set.rpe,
                        )
                    }
                fitnessDao.insertSets(
                    sets.map {
                        WorkoutSetEntity(
                            entryId = entryId,
                            setNumber = it.setNumber,
                            reps = it.reps,
                            weightKg = it.weightKg,
                            durationSeconds = it.durationSeconds,
                            rpe = it.rpe,
                            notes = null,
                        )
                    },
                )
                QueuedEntry(localExerciseId = entry.exerciseId, orderIndex = orderIndex, sets = sets)
            }
        outbox.enqueueWorkout(workoutId, QueuedWorkout(active.date, name, notes, queued))
        dao.deleteWorkout()
        return workoutId
    }

    /** An edit that makes a confirmed row invalid also unconfirms it, so it can never be recorded. */
    private suspend fun saveKeepingConfirmationHonest(set: ActiveSetEntity) {
        val stillValid = set.draft().check() is SetCheck.Valid
        dao.updateSet(set.copy(completed = set.completed && stillValid))
    }

    /** Taken from what was actually logged, never from the Exercise Category. */
    private suspend fun lastMeasureOf(exerciseId: Long): SetMeasure =
        fitnessDao.latestSetOfExercise(exerciseId)?.measure() ?: SetMeasure.REPS

    private suspend fun <T> write(block: suspend () -> T): T =
        writes.withLock { database.withTransaction { block() } }
}

private class PerformedEntry(val exerciseId: Long, val sets: List<SetCheck.Valid>)

private fun ActiveWorkoutWithEntries.orderedEntries(): List<ActiveEntryWithDetails> =
    entries.sortedWith(compareBy({ it.entry.orderIndex }, { it.entry.id }))

private fun WorkoutWithEntries.orderedEntries(): List<EntryWithDetails> =
    entries.sortedWith(compareBy({ it.entry.orderIndex }, { it.entry.id }))

internal fun WorkoutSetEntity.measure(): SetMeasure =
    when {
        reps != null && durationSeconds != null -> SetMeasure.REPS_AND_HOLD
        durationSeconds != null -> SetMeasure.HOLD
        else -> SetMeasure.REPS
    }

/** A stored zero means bodyweight in older logs; the domain writes that as a blank Added Weight. */
internal fun WorkoutSetEntity.toDraft() =
    SetDraft(
        measure = measure(),
        reps = reps?.toString().orEmpty(),
        addedWeightKg = weightKg?.takeIf { it != 0.0 }?.let { BigDecimal.valueOf(it).stripTrailingZeros().toPlainString() }.orEmpty(),
        durationSeconds = durationSeconds?.toString().orEmpty(),
        rpe = rpe?.toString().orEmpty(),
    )

private fun WorkoutSetEntity.toUnconfirmedRow(entryId: Long, setNumber: Long): ActiveSetEntity {
    val draft = toDraft()
    return ActiveSetEntity(
        entryId = entryId,
        setNumber = setNumber,
        measure = draft.measure,
        reps = draft.reps,
        weightKg = draft.addedWeightKg,
        durationSeconds = draft.durationSeconds,
        rpe = draft.rpe,
    )
}

private fun ActiveWorkoutWithEntries.performedEntries(): List<PerformedEntry> =
    orderedEntries()
        .mapNotNull { row ->
            val sets =
                row.sets
                    .sortedBy { it.setNumber }
                    .filter { it.completed }
                    .mapNotNull { it.draft().check() as? SetCheck.Valid }
            if (sets.isEmpty()) null else PerformedEntry(row.entry.exerciseId, sets)
        }

private fun ActiveSetEntity.draft() = SetDraft(measure, reps, weightKg, durationSeconds, rpe)

private fun ActiveWorkoutWithEntries.toDomain() =
    ActiveWorkout(
        date = workout.date,
        name = workout.name,
        notes = workout.notes,
        entries = orderedEntries().map(ActiveEntryWithDetails::toDomain),
        currentEntryId = workout.currentEntryId,
    )

private fun ActiveEntryWithDetails.toDomain() =
    ActiveEntry(
        id = entry.id,
        exerciseId = entry.exerciseId,
        exerciseName = exercise.name,
        exerciseCategory = exercise.category,
        sets = sets.sortedBy { it.setNumber }.map { ActiveSet(it.id, it.setNumber, it.draft(), it.completed) },
    )
