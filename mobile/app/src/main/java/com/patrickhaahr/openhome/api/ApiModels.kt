package com.patrickhaahr.openhome.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
internal data class HealthResponse(val status: String)

@Serializable
data class ApiExercise(
    val id: Long,
    val name: String,
    val category: String,
    @SerialName("muscle_group") val muscleGroup: String? = null,
    val equipment: String? = null,
)

@Serializable
data class ApiWorkoutSummary(
    val id: Long,
    val date: String,
    val name: String? = null,
)

@Serializable
data class ApiWorkoutDetail(
    val id: Long,
    val date: String,
    val name: String? = null,
    val notes: String? = null,
    val exercises: List<ApiWorkoutEntry>,
)

@Serializable
data class ApiWorkoutEntry(
    @SerialName("order_index") val orderIndex: Long,
    val notes: String? = null,
    val exercise: ApiEntryExercise,
    val sets: List<ApiSet>,
)

@Serializable
data class ApiEntryExercise(val id: Long)

/** [weightKg] is Added Weight: null means bodyweight, never zero. */
@Serializable
data class ApiSet(
    @SerialName("set_number") val setNumber: Long,
    val reps: Long? = null,
    @SerialName("weight_kg") val weightKg: Double? = null,
    @SerialName("duration_seconds") val durationSeconds: Long? = null,
    val rpe: Long? = null,
    val notes: String? = null,
)

@Serializable
data class ApiProfile(
    @SerialName("height_cm") val heightCm: Double? = null,
    val sex: String? = null,
    @SerialName("hr_max_bpm") val hrMaxBpm: Long? = null,
    @SerialName("birth_date") val birthDate: String? = null,
)

@Serializable
data class ApiBodyWeight(
    val id: Long,
    val date: String,
    @SerialName("weight_kg") val weightKg: Double,
)
