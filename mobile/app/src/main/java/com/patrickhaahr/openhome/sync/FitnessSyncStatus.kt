package com.patrickhaahr.openhome.sync

import com.patrickhaahr.openhome.storage.OutboxDao
import com.patrickhaahr.openhome.storage.OutboxKind
import com.patrickhaahr.openhome.storage.OutboxState
import com.patrickhaahr.openhome.storage.OutboxStatusRow
import com.patrickhaahr.openhome.storage.SINGLE_ROW_ID
import com.patrickhaahr.openhome.storage.SyncProblem
import java.time.LocalDate
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine

sealed interface SyncStatus {
    data object Synced : SyncStatus

    data object Syncing : SyncStatus

    data object WaitingForWifi : SyncStatus

    data object Queued : SyncStatus

    data class Retrying(val problem: SyncProblem) : SyncStatus

    data object Refused : SyncStatus
}

class SyncSnapshot(
    private val rows: List<OutboxStatusRow>,
    private val onWifi: Boolean,
    private val sendingId: Long?,
) {
    val overall: SyncStatus
        get() = if (rows.isEmpty()) SyncStatus.Synced else statusOf(rows)

    fun workout(workoutId: Long): SyncStatus =
        statusOfRecord(setOf(OutboxKind.CREATE_WORKOUT, OutboxKind.EDIT_WORKOUT), workoutId)

    fun exercise(exerciseId: Long): SyncStatus = statusOfRecord(setOf(OutboxKind.CREATE_EXERCISE), exerciseId)

    fun exerciseRefusal(exerciseId: Long): SyncProblem? =
        rows.firstOrNull {
            it.kind == OutboxKind.CREATE_EXERCISE && it.recordId == exerciseId && it.state == OutboxState.REFUSED
        }?.problem

    val profile: SyncStatus
        get() = statusOfRecord(setOf(OutboxKind.EDIT_PROFILE), SINGLE_ROW_ID.toLong())

    fun bodyWeight(date: String): SyncStatus = bodyWeightRows[date]?.let(::statusOf) ?: SyncStatus.Synced

    fun bodyWeightRefusal(date: String): SyncProblem? =
        bodyWeightRows[date]?.firstOrNull { it.state == OutboxState.REFUSED }?.problem

    val profileNeedsTheUser: Boolean
        get() =
            rows.any {
                it.state == OutboxState.REFUSED &&
                    (it.kind == OutboxKind.EDIT_PROFILE || it.kind == OutboxKind.CREATE_BODY_WEIGHT)
            }

    private val bodyWeightRows: Map<String, List<OutboxStatusRow>> by lazy {
        rows
            .filter { it.kind == OutboxKind.CREATE_BODY_WEIGHT }
            .groupBy { LocalDate.ofEpochDay(it.recordId).toString() }
    }

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

    suspend fun syncNow() {
        outbox.requeueRefused()
        scheduler.scheduleIfPending()
    }
}
