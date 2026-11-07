package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.sync.SyncStatus
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

/** A quiet cloud: routine states never interrupt, and only a failed attempt offers an action or uses words. */
@Composable
fun SyncStatusLine(status: SyncStatus, modifier: Modifier = Modifier, onSyncNow: (() -> Unit)? = null) {
    val needsTheUser = status.needsTheUser()
    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.size(32.dp).semantics { contentDescription = status.label() }, contentAlignment = Alignment.Center) {
            GlyphIcon(
                status.glyph(),
                tint =
                    when {
                        needsTheUser -> Lit.Alarm
                        status == SyncStatus.Synced -> Lit.Faint
                        else -> Lit.Lamp
                    },
                size = 22.dp,
            )
        }
        if (needsTheUser) {
            Text(
                text = status.label().substringAfter("· "),
                style = Type.Small,
                color = Lit.Alarm,
                modifier = Modifier.weight(1f, fill = false),
            )
        }
        if (onSyncNow != null && (status is SyncStatus.Retrying || status == SyncStatus.Refused)) {
            GlyphKey(Glyph.Refresh, "Sync now", onSyncNow, tone = KeyTone.Raised, size = 44.dp)
        }
    }
}

private fun SyncStatus.glyph(): Glyph =
    when (this) {
        SyncStatus.Synced -> Glyph.CloudDone
        SyncStatus.Syncing -> Glyph.CloudUp
        SyncStatus.WaitingForWifi -> Glyph.Wifi
        SyncStatus.Queued -> Glyph.CloudWait
        SyncStatus.Refused -> Glyph.Warn
        is SyncStatus.Retrying -> if (needsTheUser()) Glyph.Warn else Glyph.CloudWait
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
                SyncProblem.DATE_TAKEN,
                -> "Saved on device · OpenHome refused this change"
            }
    }
