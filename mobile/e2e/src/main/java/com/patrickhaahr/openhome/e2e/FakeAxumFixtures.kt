package com.patrickhaahr.openhome.e2e

data class FixtureSet(
    val setNumber: Int,
    val reps: Int? = null,
    val weightKg: Double? = null,
    val durationSeconds: Int? = null,
    val rpe: Int? = null,
    val notes: String? = null,
)

data class FixtureEntry(val exerciseId: Long, val sets: List<FixtureSet>, val notes: String? = null)

data class FixtureWorkout(
    val id: Long,
    val date: String,
    val name: String?,
    val notes: String? = null,
    val entries: List<FixtureEntry> = emptyList(),
    val clientRevision: Long? = null,
)

data class FixtureBodyWeight(val id: Long, val date: String, val weightKg: Double)

data class FixtureExercise(
    val id: Long,
    val name: String,
    val category: String = "calisthenics",
    val muscleGroup: String? = null,
    val equipment: String? = null,
)

data class FixtureProfile(
    val heightCm: Double? = null,
    val sex: String? = null,
    val hrMaxBpm: Int? = null,
    val birthDate: String? = null,
    val clientRevision: Long? = null,
)
