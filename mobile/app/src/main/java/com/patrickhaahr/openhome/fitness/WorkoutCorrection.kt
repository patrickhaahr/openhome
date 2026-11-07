package com.patrickhaahr.openhome.fitness

import androidx.room.withTransaction
import com.patrickhaahr.openhome.api.ApiSet
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.storage.WorkoutEntryEntity
import com.patrickhaahr.openhome.storage.WorkoutSetEntity
import com.patrickhaahr.openhome.sync.FitnessOutbox
import com.patrickhaahr.openhome.sync.FitnessSyncScheduler
import com.patrickhaahr.openhome.sync.QueuedEntry
import com.patrickhaahr.openhome.sync.QueuedWorkout
import kotlinx.serialization.Serializable

/** A finished Workout being corrected, as typed. Its notes on entries and Sets are kept, not edited. */
@Serializable
data class CorrectionDraft(
    val date: String,
    val name: String,
    val notes: String,
    val entries: List<CorrectionEntry>,
    /** The next row's key, so a row added to the draft never shares one with another. */
    val firstFreeKey: Long,
) {
    val hasSets: Boolean
        get() = entries.any { it.sets.isNotEmpty() }

    fun updatingSet(entryKey: Long, setKey: Long, change: (SetDraft) -> SetDraft): CorrectionDraft =
        updatingEntry(entryKey) { entry ->
            entry.copy(sets = entry.sets.map { if (it.key == setKey) it.copy(values = change(it.values)) else it })
        }

    fun settingMeasure(entryKey: Long, measure: SetMeasure): CorrectionDraft =
        updatingEntry(entryKey) { entry ->
            entry.copy(sets = entry.sets.map { it.copy(values = it.values.copy(measure = measure)) })
        }

    /** A new row continues the measure of the row before it, as when logging. */
    fun addingSet(entryKey: Long): CorrectionDraft =
        updatingEntry(entryKey) { entry ->
            val measure = entry.sets.lastOrNull()?.values?.measure ?: entry.newSetMeasure
            entry.copy(sets = entry.sets + CorrectionSet(firstFreeKey, emptySetDraft(measure), notes = null))
        }.copy(firstFreeKey = firstFreeKey + 1)

    fun removingSet(entryKey: Long, setKey: Long): CorrectionDraft =
        updatingEntry(entryKey) { entry -> entry.copy(sets = entry.sets.filterNot { it.key == setKey }) }

    fun addingEntry(exercise: PickedExercise): CorrectionDraft {
        val set = CorrectionSet(firstFreeKey + 1, emptySetDraft(exercise.measure), notes = null)
        val entry = CorrectionEntry(firstFreeKey, exercise.id, exercise.name, notes = null, exercise.measure, listOf(set))
        return copy(entries = entries + entry, firstFreeKey = firstFreeKey + 2)
    }

    fun removingEntry(entryKey: Long): CorrectionDraft = copy(entries = entries.filterNot { it.key == entryKey })

    /** [offset] is -1 for earlier and +1 for later; there is nowhere to go past either end. */
    fun movingEntry(entryKey: Long, offset: Int): CorrectionDraft {
        val from = entries.indexOfFirst { it.key == entryKey }
        val to = from + offset
        if (from < 0 || to !in entries.indices) return this
        return copy(entries = entries.toMutableList().apply { add(to, removeAt(from)) })
    }

    private fun updatingEntry(entryKey: Long, change: (CorrectionEntry) -> CorrectionEntry) =
        copy(entries = entries.map { if (it.key == entryKey) change(it) else it })
}

@Serializable
data class CorrectionEntry(
    val key: Long,
    val exerciseId: Long,
    val exerciseName: String,
    val notes: String?,
    /** Which measure a first row starts on, should this entry have none left to continue from. */
    val newSetMeasure: SetMeasure,
    val sets: List<CorrectionSet>,
)

@Serializable
data class CorrectionSet(val key: Long, val values: SetDraft, val notes: String?)

/** An Exercise chosen from the Library, with the measure it was last logged in. */
data class PickedExercise(val id: Long, val name: String, val measure: SetMeasure)

private fun emptySetDraft(measure: SetMeasure) =
    SetDraft(measure, reps = "", addedWeightKg = "", durationSeconds = "", rpe = "")

sealed interface CorrectionSave {
    data object Saved : CorrectionSave

    /** Nothing was saved: at least one row does not parse, and it says why itself. */
    data object InvalidSets : CorrectionSave

    /** Nothing was saved: taking the last Set away would leave no record of the session. */
    data object NoSetsLeft : CorrectionSave

    data object WorkoutGone : CorrectionSave
}

/**
 * Corrections of finished Workouts. Each commits together with its queued sync, on the same local
 * Workout, so History has it at once and whatever already refers to the Workout still does.
 */
class WorkoutCorrections(
    private val database: OpenHomeDatabase,
    private val outbox: FitnessOutbox,
    private val scheduler: FitnessSyncScheduler,
) {
    private val dao = database.fitnessDao()

    suspend fun load(workoutId: Long): CorrectionDraft? {
        val stored = dao.loadWorkout(workoutId) ?: return null
        var key = 0L
        val entries =
            stored.entries
                .sortedWith(compareBy({ it.entry.orderIndex }, { it.entry.id }))
                .map { row ->
                    CorrectionEntry(
                        key = key++,
                        exerciseId = row.entry.exerciseId,
                        exerciseName = row.exercise.name,
                        notes = row.entry.notes,
                        newSetMeasure = measureLastLogged(row.entry.exerciseId),
                        sets =
                            row.sets
                                .sortedWith(compareBy({ it.setNumber }, { it.id }))
                                .map { CorrectionSet(key++, it.toDraft(), it.notes) },
                    )
                }
        return CorrectionDraft(
            date = stored.workout.date,
            name = stored.workout.name.orEmpty(),
            notes = stored.workout.notes.orEmpty(),
            entries = entries,
            firstFreeKey = key,
        )
    }

    suspend fun pick(exerciseId: Long): PickedExercise? {
        val exercise = dao.loadExercise(exerciseId) ?: return null
        return PickedExercise(exercise.id, exercise.name, measureLastLogged(exerciseId))
    }

    /** Taken from what was actually logged, never from the Exercise Category. */
    private suspend fun measureLastLogged(exerciseId: Long): SetMeasure =
        dao.latestSetOfExercise(exerciseId)?.measure() ?: SetMeasure.REPS

    suspend fun save(workoutId: Long, draft: CorrectionDraft): CorrectionSave {
        val saved =
            database.withTransaction {
                val stored = dao.loadWorkout(workoutId) ?: return@withTransaction CorrectionSave.WorkoutGone
                if (!draft.hasSets && stored.entries.any { it.sets.isNotEmpty() }) {
                    return@withTransaction CorrectionSave.NoSetsLeft
                }
                val entries = draft.toQueuedEntries() ?: return@withTransaction CorrectionSave.InvalidSets
                val revision = stored.workout.clientRevision + 1
                val name = draft.name.trim().ifEmpty { null }
                val notes = draft.notes.trim().ifEmpty { null }
                dao.updateWorkoutDetails(workoutId, draft.date, name, notes, revision)
                dao.deleteEntriesOf(workoutId)
                entries.forEach { store(workoutId, it) }
                outbox.enqueueWorkoutCorrection(workoutId, QueuedWorkout(draft.date, name, notes, entries, revision))
                CorrectionSave.Saved
            }
        // After the commit: if the process dies first, the next app start schedules it from the outbox.
        if (saved == CorrectionSave.Saved) scheduler.schedule()
        return saved
    }

    private suspend fun store(workoutId: Long, entry: QueuedEntry) {
        val entryId =
            dao.insertEntry(
                WorkoutEntryEntity(
                    workoutId = workoutId,
                    exerciseId = entry.localExerciseId,
                    orderIndex = entry.orderIndex,
                    notes = entry.notes,
                ),
            )
        dao.insertSets(
            entry.sets.map {
                WorkoutSetEntity(
                    entryId = entryId,
                    setNumber = it.setNumber,
                    reps = it.reps,
                    weightKg = it.weightKg,
                    durationSeconds = it.durationSeconds,
                    rpe = it.rpe,
                    notes = it.notes,
                )
            },
        )
    }
}

/** In the order shown, numbered from it. Null when a row does not parse. */
private fun CorrectionDraft.toQueuedEntries(): List<QueuedEntry>? =
    entries.mapIndexed { entryIndex, entry ->
        val sets =
            entry.sets.mapIndexed { setIndex, set ->
                val valid = set.values.check() as? SetCheck.Valid ?: return null
                ApiSet(
                    setNumber = setIndex + 1L,
                    reps = valid.reps,
                    weightKg = valid.addedWeightKg,
                    durationSeconds = valid.durationSeconds,
                    rpe = valid.rpe,
                    notes = set.notes,
                )
            }
        QueuedEntry(entry.exerciseId, entryIndex.toLong(), sets, entry.notes)
    }
