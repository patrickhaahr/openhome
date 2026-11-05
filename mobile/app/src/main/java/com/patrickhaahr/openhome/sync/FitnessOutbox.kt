package com.patrickhaahr.openhome.sync

import androidx.room.withTransaction
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.api.ApiSet
import com.patrickhaahr.openhome.api.ApiWorkoutCreate
import com.patrickhaahr.openhome.api.ApiWorkoutCreateEntry
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.OutboxEntity
import com.patrickhaahr.openhome.storage.OutboxKind
import com.patrickhaahr.openhome.storage.SyncProblem
import java.util.UUID
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Statuses Axum answers only to a request that can never succeed as sent. */
private val REFUSING_STATUSES = setOf(400, 409, 410, 413, 415, 422)

/**
 * A finished Workout as queued. Exercises are local ids, resolved when sent, because an Exercise can
 * be queued too and have no server id yet. Queued rows outlive app updates: only add optional fields.
 */
@Serializable
data class QueuedWorkout(
    val date: String,
    val name: String?,
    val notes: String?,
    val entries: List<QueuedEntry>,
)

@Serializable
data class QueuedEntry(val localExerciseId: Long, val orderIndex: Long, val sets: List<ApiSet>)

enum class DeliveryRun {
    /** Nothing is left that a later attempt could deliver. */
    FINISHED,
    RETRY_LATER,
}

private sealed interface Delivery {
    data object Acknowledged : Delivery

    data class Retry(val problem: SyncProblem) : Delivery

    data object Refused : Delivery
}

/** The durable queue of fitness changes and how each kind is delivered to Axum. */
class FitnessOutbox(private val database: OpenHomeDatabase) {
    private val dao = database.outboxDao()
    private val fitnessDao = database.fitnessDao()

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

    suspend fun hasPending(): Boolean = dao.pendingCount() > 0

    suspend fun recordProblemOnPending(problem: SyncProblem) = dao.recordProblemOnPending(problem)

    suspend fun requeueRefused() = dao.requeueRefused()

    /**
     * Delivers in queue order and stops at the first change that may yet succeed, so a dependent
     * change can never overtake what it depends on. A refused change is set aside for the user.
     */
    suspend fun deliverPending(api: OpenHomeApi, onSending: (Long) -> Unit): DeliveryRun {
        var lastId = 0L
        while (true) {
            val change = dao.nextPending(afterId = lastId) ?: return DeliveryRun.FINISHED
            lastId = change.id
            onSending(change.id)
            when (val delivery = deliver(change, api)) {
                Delivery.Acknowledged -> Unit
                Delivery.Refused -> dao.markRefused(change.id)
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
        }

    private suspend fun deliverWorkout(change: OutboxEntity, api: OpenHomeApi): Delivery {
        val queued = Json.decodeFromString<QueuedWorkout>(change.payload)
        val entries =
            queued.entries.map { entry ->
                val serverId =
                    fitnessDao.exerciseServerId(entry.localExerciseId)
                        ?: return Delivery.Retry(SyncProblem.EXERCISE_NOT_SYNCED)
                ApiWorkoutCreateEntry(serverId, entry.orderIndex, entry.sets)
            }
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

    private fun ApiFailure.toDelivery(): Delivery =
        when (this) {
            is ApiFailure.Unreachable -> Delivery.Retry(SyncProblem.UNREACHABLE)
            is ApiFailure.Unauthorized -> Delivery.Retry(SyncProblem.UNAUTHORIZED)
            is ApiFailure.Rejected ->
                if (status in REFUSING_STATUSES) Delivery.Refused else Delivery.Retry(SyncProblem.SERVER_ERROR)
            is ApiFailure.UnreadableResponse,
            is ApiFailure.HistoryCannotBePaged,
            -> Delivery.Retry(SyncProblem.SERVER_ERROR)
        }
}
