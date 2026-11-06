package com.patrickhaahr.openhome.fitness

import com.patrickhaahr.openhome.storage.BodyWeightEntity
import com.patrickhaahr.openhome.storage.EntryWithDetails
import com.patrickhaahr.openhome.storage.ExerciseEntity
import com.patrickhaahr.openhome.storage.FitnessDao
import com.patrickhaahr.openhome.storage.LoggedEntryWithSets
import com.patrickhaahr.openhome.storage.ProfileEntity
import com.patrickhaahr.openhome.storage.WorkoutSetEntity
import com.patrickhaahr.openhome.storage.WorkoutSummaryRow
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map

/** The UI's source of fitness data: local storage only, so every read works offline. */
class FitnessRepository(private val dao: FitnessDao) {
    fun observeOverview(): Flow<FitnessOverview> =
        combine(dao.observeWorkoutCount(), dao.observeExerciseCount(), dao.observeLatestWorkout()) {
                workoutCount,
                exerciseCount,
                latest,
            ->
            FitnessOverview(workoutCount, exerciseCount, latest?.toDomain())
        }

    fun observeHistory(): Flow<List<WorkoutSummary>> =
        dao.observeHistory().map { rows -> rows.map { it.toDomain() } }

    fun observeWorkout(id: Long): Flow<Workout?> =
        dao.observeWorkout(id).map { stored ->
            stored?.let {
                Workout(
                    id = it.workout.id,
                    date = it.workout.date,
                    name = it.workout.name,
                    notes = it.workout.notes,
                    entries =
                        it.entries
                            .sortedWith(compareBy({ row -> row.entry.orderIndex }, { row -> row.entry.id }))
                            .map(EntryWithDetails::toDomain),
                )
            }
        }

    /** Includes Workouts finished on this device that are still waiting to synchronize. */
    fun observePreviousPerformance(exerciseId: Long): Flow<PreviousPerformance?> =
        dao.observeLatestLoggedEntry(exerciseId).map { it?.toPreviousPerformance() }

    fun observeExercises(): Flow<List<Exercise>> =
        dao.observeExercises().map { rows -> rows.map(ExerciseEntity::toDomain) }

    fun observeProfile(): Flow<Profile?> = dao.observeProfile().map { it?.toDomain() }

    /** Newest first, because the most recent weigh-in is what the Profile needs at a glance. */
    fun observeBodyWeight(): Flow<List<BodyWeight>> =
        dao.observeBodyWeight().map { rows -> rows.map(BodyWeightEntity::toDomain) }
}

private fun WorkoutSummaryRow.toDomain() =
    WorkoutSummary(
        id = id,
        date = date,
        name = name,
        exerciseNames = exerciseNames,
        setCount = setCount,
    )

private fun EntryWithDetails.toDomain() =
    WorkoutEntry(
        id = entry.id,
        exerciseName = exercise.name,
        exerciseCategory = exercise.category,
        notes = entry.notes,
        sets = sets.toOrderedDomain(),
    )

private fun LoggedEntryWithSets.toPreviousPerformance() =
    PreviousPerformance(entry.exerciseId, workoutDate, sets.toOrderedDomain())

private fun List<WorkoutSetEntity>.toOrderedDomain() =
    sortedWith(compareBy({ it.setNumber }, { it.id }))
        .map { set ->
            WorkoutSet(
                id = set.id,
                setNumber = set.setNumber,
                reps = set.reps,
                addedWeightKg = set.weightKg,
                durationSeconds = set.durationSeconds,
                rpe = set.rpe,
                notes = set.notes,
            )
        }

private fun ExerciseEntity.toDomain() = Exercise(id, name, category, muscleGroup, equipment)

internal fun ProfileEntity.toDomain() =
    Profile(
        heightCm = heightCm,
        sex = sex,
        birthDate = birthDate,
        maxHeartRateBpm = hrMaxBpm,
    )

internal fun BodyWeightEntity.toDomain() = BodyWeight(date, weightKg)
