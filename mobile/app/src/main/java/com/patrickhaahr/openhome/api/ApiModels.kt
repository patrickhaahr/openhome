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
    @SerialName("client_revision") val clientRevision: Long = 0,
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
    @SerialName("client_revision") val clientRevision: Long = 0,
)

@Serializable
data class ApiProfileEdit(
    @SerialName("client_revision") val clientRevision: Long,
    @SerialName("height_cm") val heightCm: Double?,
    val sex: String?,
    @SerialName("hr_max_bpm") val hrMaxBpm: Long?,
    @SerialName("birth_date") val birthDate: String?,
)

@Serializable
data class ApiBodyWeight(
    val id: Long,
    val date: String,
    @SerialName("weight_kg") val weightKg: Double,
)

@Serializable
data class ApiBodyWeightCreate(
    @SerialName("request_id") val requestId: String,
    val date: String,
    @SerialName("weight_kg") val weightKg: Double,
)

@Serializable
data class ApiWorkoutCreate(
    @SerialName("request_id") val requestId: String,
    val date: String,
    val name: String? = null,
    val notes: String? = null,
    val exercises: List<ApiWorkoutCreateEntry>,
)

@Serializable
data class ApiWorkoutCreateEntry(
    @SerialName("exercise_id") val exerciseId: Long,
    @SerialName("order_index") val orderIndex: Long,
    val sets: List<ApiSet>,
    val notes: String? = null,
)

@Serializable
data class ApiWorkoutCreated(val id: Long)

@Serializable
data class ApiWorkoutEdit(
    @SerialName("client_revision") val clientRevision: Long,
    val date: String,
    val name: String?,
    val notes: String?,
    val exercises: List<ApiWorkoutCreateEntry>,
)

@Serializable
data class ApiExerciseCreate(
    @SerialName("request_id") val requestId: String,
    val name: String,
    val category: String,
    @SerialName("muscle_group") val muscleGroup: String? = null,
    val equipment: String? = null,
)

@Serializable
data class ApiIrStatus(val remotes: ApiIrRemotes)

@Serializable
data class ApiIrRemotes(val lgtv: List<String>, val edifier: List<String>)

@Serializable
data class ApiIrCommand(val command: String)

@Serializable
data class ApiIrAcknowledgement(val message: String)

enum class LightCommand(val path: String, val label: String) {
    On("on", "On"),
    Off("off", "Off"),
}
