package com.patrickhaahr.openhome.sync

import androidx.room.withTransaction
import com.patrickhaahr.openhome.api.ApiBodyWeightCreate
import com.patrickhaahr.openhome.api.ApiExercise
import com.patrickhaahr.openhome.api.ApiExerciseCreate
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.api.ApiProfileEdit
import com.patrickhaahr.openhome.api.ApiSet
import com.patrickhaahr.openhome.api.ApiWorkoutCreate
import com.patrickhaahr.openhome.api.ApiWorkoutCreateEntry
import com.patrickhaahr.openhome.api.ApiWorkoutEdit
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.OutboxEntity
import com.patrickhaahr.openhome.storage.OutboxKind
import com.patrickhaahr.openhome.storage.SINGLE_ROW_ID
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.storage.bodyWeightRecordId
import java.time.LocalDate
import java.util.UUID
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Statuses Axum answers only to a request that can never succeed as sent. */
private val REFUSING_STATUSES = setOf(400, 409, 410, 413, 415, 422)
private const val CONFLICT = 409
private const val NOT_FOUND = 404

/**
 * A whole Workout as queued, whether created or corrected. Exercises are local ids, resolved when sent,
 * because an Exercise can be queued too and have no server id yet. [clientRevision] is set on a
 * correction, which states everything rather than what changed, so the newest one alone is enough.
 * Queued rows outlive app updates: only add optional fields.
 */
@Serializable
data class QueuedWorkout(
    val date: String,
    val name: String?,
    val notes: String?,
    val entries: List<QueuedEntry>,
    val clientRevision: Long? = null,
)

@Serializable
data class QueuedEntry(
    val localExerciseId: Long,
    val orderIndex: Long,
    val sets: List<ApiSet>,
    val notes: String? = null,
)

/**
 * A new Exercise as queued. [useExisting] means the user chose the Exercise Axum already holds under
 * this exact name, so delivery looks it up rather than creating one.
 */
@Serializable
data class QueuedExercise(
    val name: String,
    val category: String,
    val muscleGroup: String? = null,
    val equipment: String? = null,
    val useExisting: Boolean = false,
)

/**
 * A new Body Weight as queued. [useExisting] means the user chose the record Axum already holds for
 * this date, so delivery takes its value rather than recording one.
 */
@Serializable
data class QueuedBodyWeight(val date: String, val weightKg: Double, val useExisting: Boolean = false)

/** A whole Profile as edited, so the newest edit alone is enough. Queued rows outlive app updates: only add optional fields. */
@Serializable
data class QueuedProfile(
    val heightCm: Double?,
    val sex: String?,
    val hrMaxBpm: Long?,
    val birthDate: String?,
    val clientRevision: Long,
)

enum class DeliveryRun {
    /** Nothing is left that a later attempt could deliver. */
    FINISHED,
    RETRY_LATER,
}

private sealed interface Delivery {
    data object Acknowledged : Delivery

    data class Retry(val problem: SyncProblem) : Delivery

    data class Refused(val problem: SyncProblem = SyncProblem.REFUSED) : Delivery

    /** Sending it now would break a reference. It stays queued and costs nothing but its turn. */
    data class Waiting(val problem: SyncProblem) : Delivery
}

/** The durable queue of fitness changes and how each kind is delivered to Axum. */
class FitnessOutbox(private val database: OpenHomeDatabase) {
    private val dao = database.outboxDao()
    private val fitnessDao = database.fitnessDao()
    private val restDao = database.restDao()

    /** Call inside the transaction that creates the Workout, so neither can exist without the other. */
    suspend fun enqueueWorkout(workoutId: Long, workout: QueuedWorkout) {
        dao.insert(
            OutboxEntity(
                kind = OutboxKind.CREATE_WORKOUT,
                recordId = workoutId,
                requestId = UUID.randomUUID().toString(),
                payload = Json.encodeToString(workout),
            ),
        )
    }

    /**
     * Call inside the transaction that saves a correction with [workout]'s revision. It supersedes any
     * correction still queued, even one in flight, which Axum then keeps from taking effect afterwards.
     * A create Axum refused was never accepted, so replace it at the queue's tail after any new
     * Exercises it now uses. Its new `request_id` also leaves any old receipt behind. Anything else
     * queues behind the create, whose frozen arguments may already have committed without an answer.
     */
    suspend fun enqueueWorkoutCorrection(workoutId: Long, workout: QueuedWorkout) {
        dao.deleteAllOf(OutboxKind.EDIT_WORKOUT, workoutId)
        val payload = Json.encodeToString(workout)
        val replacedRefusedCreate = dao.deleteRefused(OutboxKind.CREATE_WORKOUT, workoutId) > 0
        dao.insert(
            OutboxEntity(
                kind = if (replacedRefusedCreate) OutboxKind.CREATE_WORKOUT else OutboxKind.EDIT_WORKOUT,
                recordId = workoutId,
                requestId = UUID.randomUUID().toString(),
                payload = payload,
            ),
        )
    }

    /** Call inside the transaction that creates the Exercise, so neither can exist without the other. */
    suspend fun enqueueExercise(exerciseId: Long, exercise: QueuedExercise) {
        dao.insert(
            OutboxEntity(
                kind = OutboxKind.CREATE_EXERCISE,
                recordId = exerciseId,
                requestId = UUID.randomUUID().toString(),
                payload = Json.encodeToString(exercise),
            ),
        )
    }

    suspend fun refusalOfExercise(exerciseId: Long): SyncProblem? =
        dao.refusalOf(OutboxKind.CREATE_EXERCISE, exerciseId)

    /**
     * Sends a refused Exercise again with corrected arguments, from the same place in the queue. A new
     * `request_id` because the old one may hold a receipt whose Exercise has since been deleted. Returns
     * false, changing nothing, when the Exercise is no longer refused.
     */
    suspend fun replaceRefusedExercise(exerciseId: Long, exercise: QueuedExercise): Boolean =
        dao.replaceRefused(
            kind = OutboxKind.CREATE_EXERCISE,
            recordId = exerciseId,
            requestId = UUID.randomUUID().toString(),
            payload = Json.encodeToString(exercise),
        ) > 0

    /** Call inside the transaction that records the Body Weight, so neither can exist without the other. */
    suspend fun enqueueBodyWeight(entry: QueuedBodyWeight) {
        dao.insert(
            OutboxEntity(
                kind = OutboxKind.CREATE_BODY_WEIGHT,
                recordId = bodyWeightRecordId(LocalDate.parse(entry.date)),
                requestId = UUID.randomUUID().toString(),
                payload = Json.encodeToString(entry),
            ),
        )
    }

    /**
     * The user's answer to a taken date: drop this value for the record Axum holds, from the same place
     * in the queue. A new `request_id`, since this one is only ever sent if Axum turns out to hold none.
     * Returns false, changing nothing, when the record is no longer refused.
     */
    suspend fun useBodyWeightOnServer(date: String, weightKg: Double): Boolean =
        dao.replaceRefused(
            kind = OutboxKind.CREATE_BODY_WEIGHT,
            recordId = bodyWeightRecordId(LocalDate.parse(date)),
            requestId = UUID.randomUUID().toString(),
            payload = Json.encodeToString(QueuedBodyWeight(date, weightKg, useExisting = true)),
        ) > 0

    /**
     * Call inside the transaction that saves a Profile edit with [profile]'s revision. It supersedes any
     * edit still queued, even one in flight, which Axum then keeps from taking effect afterwards.
     */
    suspend fun enqueueProfileEdit(profile: QueuedProfile) {
        dao.deleteAllOf(OutboxKind.EDIT_PROFILE, SINGLE_ROW_ID.toLong())
        dao.insert(
            OutboxEntity(
                kind = OutboxKind.EDIT_PROFILE,
                recordId = SINGLE_ROW_ID.toLong(),
                requestId = UUID.randomUUID().toString(),
                payload = Json.encodeToString(profile),
            ),
        )
    }

    suspend fun hasPending(): Boolean = dao.pendingCount() > 0

    suspend fun recordProblemOnPending(problem: SyncProblem) = dao.recordProblemOnPending(problem)

    suspend fun requeueRefused() = dao.requeueRefused()

    /**
     * Delivers in queue order and stops at the first change that may yet succeed, so a dependent
     * change can never overtake what it depends on. A refused change is set aside for the user, and
     * only what depends on it waits with it; everything else carries on.
     *
     * Each row ends in exactly one [Delivery]. The per-kind deliverers apply [Delivery.Acknowledged] themselves, in
     * the transaction that deletes the row; this loop applies the other variants (refuse, record a problem, stop the
     * pass). A row's acknowledgement and its deletion commit together (or, for the idempotent kinds, the repeat under
     * the same `request_id` is itself acknowledged), so a crash or a replaced pass either leaves the row
     * to be sent again unchanged or leaves it gone. A row superseded while in flight makes the updates
     * here no-ops. Cancellation propagates out of the in-flight call rather than being recorded as a
     * problem; the run that replaced this one starts over from the stored queue.
     */
    suspend fun deliverPending(api: OpenHomeApi, onSending: (Long) -> Unit): DeliveryRun {
        var lastId = 0L
        while (true) {
            val change = dao.nextPending(afterId = lastId) ?: return DeliveryRun.FINISHED
            lastId = change.id
            onSending(change.id)
            when (val delivery = deliver(change, api)) {
                Delivery.Acknowledged -> Unit
                is Delivery.Refused -> dao.markRefused(change.id, delivery.problem)
                is Delivery.Waiting -> dao.recordProblem(change.id, delivery.problem)
                is Delivery.Retry -> {
                    dao.recordProblem(change.id, delivery.problem)
                    return DeliveryRun.RETRY_LATER
                }
            }
        }
    }

    private suspend fun deliver(change: OutboxEntity, api: OpenHomeApi): Delivery =
        when (change.kind) {
            OutboxKind.CREATE_WORKOUT -> deliverWorkout(change, api)
            OutboxKind.CREATE_EXERCISE -> deliverExercise(change, api)
            OutboxKind.EDIT_WORKOUT -> deliverWorkoutEdit(change, api)
            OutboxKind.CREATE_BODY_WEIGHT -> deliverBodyWeight(change, api)
            OutboxKind.EDIT_PROFILE -> deliverProfileEdit(change, api)
        }

    private suspend fun deliverWorkout(change: OutboxEntity, api: OpenHomeApi): Delivery {
        val queued = Json.decodeFromString<QueuedWorkout>(change.payload)
        val entries = resolveEntries(queued) ?: return Delivery.Waiting(SyncProblem.EXERCISE_NOT_SYNCED)
        val request = ApiWorkoutCreate(change.requestId, queued.date, queued.name, queued.notes, entries)
        val created =
            try {
                api.createWorkout(request)
            } catch (failure: ApiFailure) {
                return failure.toDelivery()
            }
        database.withTransaction {
            fitnessDao.setWorkoutServerId(change.recordId, created.id)
            dao.delete(change.id)
        }
        return Delivery.Acknowledged
    }

    /**
     * The revision makes a repeat harmless, so a lost answer is simply retried. Its create is queued
     * ahead of it, so reaching here without a server id means that create is waiting or refused.
     */
    private suspend fun deliverWorkoutEdit(change: OutboxEntity, api: OpenHomeApi): Delivery {
        val queued = Json.decodeFromString<QueuedWorkout>(change.payload)
        val serverId =
            fitnessDao.workoutServerId(change.recordId) ?: return Delivery.Waiting(SyncProblem.WORKOUT_NOT_SYNCED)
        val entries = resolveEntries(queued) ?: return Delivery.Waiting(SyncProblem.EXERCISE_NOT_SYNCED)
        val edit =
            ApiWorkoutEdit(checkNotNull(queued.clientRevision), queued.date, queued.name, queued.notes, entries)
        try {
            api.editWorkout(serverId, edit)
        } catch (failure: ApiFailure) {
            // Retrying would hold up the whole queue for a Workout deleted elsewhere, which stays deleted.
            return if (failure is ApiFailure.Rejected && failure.status == NOT_FOUND) Delivery.Refused() else failure.toDelivery()
        }
        dao.delete(change.id)
        return Delivery.Acknowledged
    }

    /** Null when an Exercise has no server id: it is queued ahead of what uses it, so it was refused. */
    private suspend fun resolveEntries(queued: QueuedWorkout): List<ApiWorkoutCreateEntry>? =
        queued.entries.map { entry ->
            val serverId = fitnessDao.exerciseServerId(entry.localExerciseId) ?: return null
            ApiWorkoutCreateEntry(serverId, entry.orderIndex, entry.sets, entry.notes)
        }

    private suspend fun deliverExercise(change: OutboxEntity, api: OpenHomeApi): Delivery {
        val queued = Json.decodeFromString<QueuedExercise>(change.payload)
        val server =
            try {
                if (queued.useExisting) {
                    api.exercises().firstOrNull { it.name == queued.name } ?: return Delivery.Refused()
                } else {
                    api.createExercise(
                        ApiExerciseCreate(change.requestId, queued.name, queued.category, queued.muscleGroup, queued.equipment),
                    )
                }
            } catch (failure: ApiFailure) {
                return if (failure is ApiFailure.Rejected && failure.status == CONFLICT) {
                    whyConflicted(queued, api)
                } else {
                    failure.toDelivery()
                }
            }
        database.withTransaction {
            reconcileExercise(change.recordId, server)
            dao.delete(change.id)
        }
        return Delivery.Acknowledged
    }

    /** Reads the answer from the Library itself: the name is taken, or its earlier receipt is stale. */
    private suspend fun whyConflicted(queued: QueuedExercise, api: OpenHomeApi): Delivery =
        try {
            val taken = api.exercises().any { it.name == queued.name }
            Delivery.Refused(if (taken) SyncProblem.NAME_TAKEN else SyncProblem.REFUSED)
        } catch (failure: ApiFailure) {
            failure.toDelivery()
        }

    /**
     * Records which Axum Exercise this local row is. Should another local row already hold that id,
     * they are one Exercise: everything moves to that row, so no Workout entry breaks and no duplicate
     * stays behind.
     */
    private suspend fun reconcileExercise(localId: Long, server: ApiExercise) {
        val holder = fitnessDao.exerciseHoldingServerId(server.id)
        if (holder != null && holder != localId) mergeExercise(from = localId, into = holder)
        fitnessDao.applyServerExercise(
            exerciseId = holder ?: localId,
            serverId = server.id,
            name = server.name,
            category = server.category,
            muscleGroup = server.muscleGroup,
            equipment = server.equipment,
        )
    }

    /** Queued Workouts and corrections are rewritten only in their local ids, which resolve to the same Axum Exercise. */
    private suspend fun mergeExercise(from: Long, into: Long) {
        fitnessDao.moveWorkoutEntries(from, into)
        fitnessDao.moveActiveEntries(from, into)
        restDao.movePreference(from, into)
        restDao.moveTimer(from, into)
        for (row in dao.allOfKinds(listOf(OutboxKind.CREATE_WORKOUT, OutboxKind.EDIT_WORKOUT))) {
            val queued = Json.decodeFromString<QueuedWorkout>(row.payload)
            if (queued.entries.none { it.localExerciseId == from }) continue
            val entries = queued.entries.map { if (it.localExerciseId == from) it.copy(localExerciseId = into) else it }
            dao.updatePayload(row.id, Json.encodeToString(queued.copy(entries = entries)))
        }
        fitnessDao.deleteExercise(from)
    }

    /**
     * A repeat under this `request_id` is acknowledged with the same record, so a lost answer is simply
     * retried. A conflict is therefore a date Axum holds under some other record, which stays as it is
     * and leaves this one for the user. The local row is the one being sent, so an answer changes nothing.
     */
    private suspend fun deliverBodyWeight(change: OutboxEntity, api: OpenHomeApi): Delivery {
        val queued = Json.decodeFromString<QueuedBodyWeight>(change.payload)
        try {
            val existing = if (queued.useExisting) api.bodyWeightOn(queued.date) else null
            if (existing != null) {
                database.withTransaction {
                    fitnessDao.setBodyWeight(queued.date, existing.weightKg)
                    dao.delete(change.id)
                }
                return Delivery.Acknowledged
            }
            api.createBodyWeight(ApiBodyWeightCreate(change.requestId, queued.date, queued.weightKg))
        } catch (failure: ApiFailure) {
            return if (failure is ApiFailure.Rejected && failure.status == CONFLICT) {
                Delivery.Refused(SyncProblem.DATE_TAKEN)
            } else {
                failure.toDelivery()
            }
        }
        dao.delete(change.id)
        return Delivery.Acknowledged
    }

    /** The revision makes a repeat harmless. The answer is not stored: a newer edit may be saved here by now. */
    private suspend fun deliverProfileEdit(change: OutboxEntity, api: OpenHomeApi): Delivery {
        val queued = Json.decodeFromString<QueuedProfile>(change.payload)
        try {
            api.editProfile(
                ApiProfileEdit(queued.clientRevision, queued.heightCm, queued.sex, queued.hrMaxBpm, queued.birthDate),
            )
        } catch (failure: ApiFailure) {
            return failure.toDelivery()
        }
        dao.delete(change.id)
        return Delivery.Acknowledged
    }

    private fun ApiFailure.toDelivery(): Delivery =
        when (this) {
            is ApiFailure.Unreachable -> Delivery.Retry(SyncProblem.UNREACHABLE)
            is ApiFailure.Unauthorized -> Delivery.Retry(SyncProblem.UNAUTHORIZED)
            is ApiFailure.Rejected ->
                if (status in REFUSING_STATUSES) Delivery.Refused() else Delivery.Retry(SyncProblem.SERVER_ERROR)
            is ApiFailure.UnreadableResponse,
            is ApiFailure.HistoryCannotBePaged,
            -> Delivery.Retry(SyncProblem.SERVER_ERROR)
        }
}
