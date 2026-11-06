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
    /** Where this client's corrections of this Workout continue from. Absent until one has reached Axum. */
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
    /** Where this client's Profile edits continue from. Absent until one has reached Axum. */
    @SerialName("client_revision") val clientRevision: Long = 0,
)

/**
 * A whole Profile edit: Axum applies it only if [clientRevision] is newer than the last it applied. No
 * defaults, so a null value is sent and clears it rather than leaving it as it was.
 */
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

/** A new Body Weight; [requestId] makes it safe to retry, and a new one for a recorded date is a conflict. */
@Serializable
data class ApiBodyWeightCreate(
    @SerialName("request_id") val requestId: String,
    val date: String,
    @SerialName("weight_kg") val weightKg: Double,
)

/** A Workout create; [requestId] makes it safe to retry after an uncertain outcome. */
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

/**
 * A whole correction: Axum applies it only if [clientRevision] is newer than the last it applied. No
 * defaults, so a null [name] or [notes] is sent and clears the value rather than leaving it as it was.
 */
@Serializable
data class ApiWorkoutEdit(
    @SerialName("client_revision") val clientRevision: Long,
    val date: String,
    val name: String?,
    val notes: String?,
    val exercises: List<ApiWorkoutCreateEntry>,
)

/** An Exercise create; [requestId] makes it safe to retry after an uncertain outcome. */
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
data class ApiIrRemotes(val lgtv: List<String>)

@Serializable
data class ApiIrCommand(val command: String)

@Serializable
data class ApiIrAcknowledgement(val message: String)
