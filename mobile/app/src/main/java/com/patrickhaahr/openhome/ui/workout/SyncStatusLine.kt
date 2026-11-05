package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.sync.SyncStatus

/** A quiet line: routine states never interrupt, and only a failed attempt offers an action. */
@Composable
fun SyncStatusLine(status: SyncStatus, modifier: Modifier = Modifier, onSyncNow: (() -> Unit)? = null) {
    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = status.label(),
            style = MaterialTheme.typography.bodyMedium,
            color =
                if (status.needsTheUser()) {
                    MaterialTheme.colorScheme.error
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
            modifier = Modifier.weight(1f, fill = false),
        )
        if (onSyncNow != null && (status is SyncStatus.Retrying || status == SyncStatus.Refused)) {
            TextButton(onClick = onSyncNow) { Text("Sync now") }
        }
    }
}

private fun SyncStatus.needsTheUser(): Boolean =
    this == SyncStatus.Refused ||
        this is SyncStatus.Retrying &&
        (problem == SyncProblem.UNAUTHORIZED || problem == SyncProblem.API_KEY_UNAVAILABLE)

private fun SyncStatus.label(): String =
    when (this) {
        SyncStatus.Synced -> "Synced with OpenHome"
        SyncStatus.Syncing -> "Syncing with OpenHome"
        SyncStatus.WaitingForWifi -> "Saved on device · Waiting for Wi-Fi"
        SyncStatus.Queued -> "Saved on device · Syncing soon"
        SyncStatus.Refused -> "Saved on device · OpenHome refused this change"
        is SyncStatus.Retrying ->
            when (problem) {
                SyncProblem.UNREACHABLE -> "Saved on device · Can't reach OpenHome, will retry"
                SyncProblem.SERVER_ERROR -> "Saved on device · OpenHome had a problem, will retry"
                SyncProblem.UNAUTHORIZED -> "Saved on device · OpenHome rejected the API Key"
                SyncProblem.API_KEY_UNAVAILABLE -> "Saved on device · The saved API Key can't be read"
                SyncProblem.EXERCISE_NOT_SYNCED -> "Saved on device · Waiting for its Exercise to sync"
                SyncProblem.WORKOUT_NOT_SYNCED -> "Saved on device · Waiting for this Workout's first sync"
                SyncProblem.REFUSED,
                SyncProblem.NAME_TAKEN,
                -> "Saved on device · OpenHome refused this change"
            }
    }
