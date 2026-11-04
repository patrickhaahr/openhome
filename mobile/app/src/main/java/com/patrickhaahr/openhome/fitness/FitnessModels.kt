package com.patrickhaahr.openhome.fitness

/**
 * Fitness domain as the UI sees it, matching CONTEXT.md. Dates are `YYYY-MM-DD` calendar days and are
 * never shifted through a time zone. Null means absent; it is never replaced with zero or a default.
 */
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

/**
 * A Set has reps or a duration; a Timed Hold has a duration and null reps.
 * [addedWeightKg] is Added Weight: null means bodyweight, a value is external kilograms.
 */
data class WorkoutSet(
    val id: Long,
    val setNumber: Long,
    val reps: Long?,
    val addedWeightKg: Double?,
    val durationSeconds: Long?,
    val rpe: Long?,
    val notes: String?,
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
