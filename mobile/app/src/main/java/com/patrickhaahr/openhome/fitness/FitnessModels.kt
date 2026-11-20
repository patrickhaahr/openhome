package com.patrickhaahr.openhome.fitness

data class WorkoutSummary(
    val id: Long,
    val date: String,
    val name: String?,
    val exerciseNames: String?,
    val setCount: Int,
)

data class Workout(
    val id: Long,
    val date: String,
    val name: String?,
    val notes: String?,
    val entries: List<WorkoutEntry>,
)

data class WorkoutEntry(
    val id: Long,
    val exerciseName: String,
    val exerciseCategory: String,
    val notes: String?,
    val sets: List<WorkoutSet>,
)

data class WorkoutSet(
    val id: Long,
    val setNumber: Long,
    val reps: Long?,
    val addedWeightKg: Double?,
    val durationSeconds: Long?,
    val rpe: Long?,
    val notes: String?,
)

data class PreviousPerformance(val exerciseId: Long, val date: String, val sets: List<WorkoutSet>)

data class Exercise(
    val id: Long,
    val name: String,
    val category: String,
    val muscleGroup: String?,
    val equipment: String?,
)

data class FitnessOverview(
    val workoutCount: Int,
    val exerciseCount: Int,
    val latestWorkout: WorkoutSummary?,
)

data class Profile(
    val heightCm: Double?,
    val sex: String?,
    val birthDate: String?,
    val maxHeartRateBpm: Long?,
)

data class BodyWeight(val date: String, val weightKg: Double)
