package com.patrickhaahr.openhome.sync

import com.patrickhaahr.openhome.storage.OutboxDao
import com.patrickhaahr.openhome.storage.OutboxKind
import com.patrickhaahr.openhome.storage.OutboxState
import com.patrickhaahr.openhome.storage.OutboxStatusRow
import com.patrickhaahr.openhome.storage.SyncProblem
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine

sealed interface SyncStatus {
    data object Synced : SyncStatus

    data object Syncing : SyncStatus

    data object WaitingForWifi : SyncStatus

    /** On Wi-Fi with nothing wrong yet; a pass is due. */
    data object Queued : SyncStatus

    /** The last attempt failed; retries continue on their own. */
    data class Retrying(val problem: SyncProblem) : SyncStatus

    /** Axum refused it; only the user's retry sends it again. */
    data object Refused : SyncStatus
}

/** Everything not yet acknowledged, read once per change in the outbox, connectivity or sending. */
class SyncSnapshot(
    private val rows: List<OutboxStatusRow>,
    private val onWifi: Boolean,
    private val sendingId: Long?,
) {
    /** The whole queue in one status: what most needs the user's attention, then what happens next. */
    val overall: SyncStatus
        get() = if (rows.isEmpty()) SyncStatus.Synced else statusOf(rows)

    fun workout(workoutId: Long): SyncStatus =
        statusOfRecord(setOf(OutboxKind.CREATE_WORKOUT, OutboxKind.EDIT_WORKOUT), workoutId)

    fun exercise(exerciseId: Long): SyncStatus = statusOfRecord(setOf(OutboxKind.CREATE_EXERCISE), exerciseId)

    /** Why Axum refused this Exercise, while it waits for the user to act on that. */
    fun exerciseRefusal(exerciseId: Long): SyncProblem? =
        rows.firstOrNull {
            it.kind == OutboxKind.CREATE_EXERCISE && it.recordId == exerciseId && it.state == OutboxState.REFUSED
        }?.problem

    private fun statusOfRecord(kinds: Set<OutboxKind>, recordId: Long): SyncStatus {
        val own = rows.filter { it.kind in kinds && it.recordId == recordId }
        return if (own.isEmpty()) SyncStatus.Synced else statusOf(own)
    }

    private fun statusOf(changes: List<OutboxStatusRow>): SyncStatus {
        val pending = changes.filter { it.state == OutboxState.PENDING }
        return when {
            changes.any { it.state == OutboxState.REFUSED } -> SyncStatus.Refused
            pending.any { it.id == sendingId } -> SyncStatus.Syncing
            !onWifi -> SyncStatus.WaitingForWifi
            else -> pending.firstNotNullOfOrNull { it.problem }?.let(SyncStatus::Retrying) ?: SyncStatus.Queued
        }
    }
}

class FitnessSyncStatus(
    private val dao: OutboxDao,
    private val wifi: WifiNetworks,
    private val sending: StateFlow<Long?>,
    private val outbox: FitnessOutbox,
    private val scheduler: FitnessSyncScheduler,
) {
    fun observe(): Flow<SyncSnapshot> =
        combine(dao.observeStatus(), wifi.observeOnWifi(), sending, ::SyncSnapshot)

    /** The user's "try again": refused changes rejoin the queue and any waiting retry is skipped. */
    suspend fun syncNow() {
        outbox.requeueRefused()
        scheduler.scheduleIfPending()
    }
}
