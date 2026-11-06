package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.REST_STEP_SECONDS
import com.patrickhaahr.openhome.fitness.RestTimer
import com.patrickhaahr.openhome.fitness.RestTimers
import com.patrickhaahr.openhome.ui.appViewModel
import kotlin.math.ceil
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private const val MILLIS_PER_SECOND = 1_000L
private const val SECONDS_PER_MINUTE = 60L

/** What the bar shows of the rest at this moment. */
private sealed interface RestDisplay {
    val exerciseId: Long
    val exerciseName: String

    data class Running(
        override val exerciseId: Long,
        override val exerciseName: String,
        val remainingSeconds: Long,
    ) : RestDisplay

    data class Over(override val exerciseId: Long, override val exerciseName: String) : RestDisplay
}

/** Every reading comes from the clock and the stored deadline; nothing here counts down. */
private fun RestTimer.displays(): Flow<RestDisplay> =
    flow {
        while (true) {
            val remainingMillis = deadlineAt - System.currentTimeMillis()
            if (remainingMillis <= 0) {
                emit(RestDisplay.Over(exerciseId, exerciseName))
                return@flow
            }
            emit(RestDisplay.Running(exerciseId, exerciseName, ceil(remainingMillis / MILLIS_PER_SECOND.toDouble()).toLong()))
            // To the next whole second; never longer, so a moved clock shows within that second.
            delay(remainingMillis % MILLIS_PER_SECOND + 1)
        }
    }

private class RestTimerViewModel(private val timers: RestTimers, private val saveScope: CoroutineScope) : ViewModel() {
    @OptIn(ExperimentalCoroutinesApi::class)
    val display: StateFlow<RestDisplay?> =
        timers.observe()
            .flatMapLatest { timer -> timer?.displays() ?: flowOf(null) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    fun adjust(exerciseId: Long, deltaSeconds: Long) {
        saveScope.launch { timers.adjust(exerciseId, deltaSeconds) }
    }

    fun turnOff(exerciseId: Long) {
        saveScope.launch { timers.setEnabled(exerciseId, enabled = false) }
    }

    fun end() {
        saveScope.launch { timers.end() }
    }
}

/** Above the tabs on every destination, so resting never stands in the way of logging or moving around. */
@Composable
fun RestTimerBar(modifier: Modifier = Modifier) {
    val viewModel = appViewModel { RestTimerViewModel(it.restTimers, it.saveScope) }
    val display by viewModel.display.collectAsStateWithLifecycle()
    when (val current = display) {
        null -> Unit
        is RestDisplay.Running -> RunningBar(current, viewModel, modifier)
        is RestDisplay.Over -> OverBar(current, viewModel, modifier)
    }
}

@Composable
private fun RunningBar(rest: RestDisplay.Running, viewModel: RestTimerViewModel, modifier: Modifier) {
    BarSurface(modifier) {
        Column(Modifier.weight(1f)) {
            Text(
                text = formatRest(rest.remainingSeconds),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.primary,
            )
            RestOfLine(rest.exerciseName)
        }
        BarAction("−15", "Less rest") { viewModel.adjust(rest.exerciseId, -REST_STEP_SECONDS) }
        BarAction("+15", "More rest") { viewModel.adjust(rest.exerciseId, REST_STEP_SECONDS) }
        BarAction("Skip", "Skip rest", viewModel::end)
        BarAction("Off", "Turn off rest for ${rest.exerciseName}") { viewModel.turnOff(rest.exerciseId) }
    }
}

@Composable
private fun OverBar(rest: RestDisplay.Over, viewModel: RestTimerViewModel, modifier: Modifier) {
    BarSurface(modifier) {
        Column(Modifier.weight(1f).semantics { liveRegion = LiveRegionMode.Polite }) {
            Text(
                text = "Rest over",
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.primary,
            )
            RestOfLine(rest.exerciseName)
        }
        BarAction("Dismiss", "Dismiss rest", viewModel::end)
    }
}

@Composable
private fun BarSurface(modifier: Modifier, content: @Composable RowScope.() -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = modifier.padding(start = 16.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            content = content,
        )
    }
}

@Composable
private fun RestOfLine(exerciseName: String) {
    Text(
        text = "Rest · $exerciseName",
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
private fun BarAction(label: String, description: String, onClick: () -> Unit) {
    TextButton(
        onClick = onClick,
        modifier = Modifier.widthIn(min = 48.dp).heightIn(min = 48.dp).semantics { contentDescription = description },
    ) {
        Text(label)
    }
}

internal fun formatRest(seconds: Long): String =
    "${seconds / SECONDS_PER_MINUTE}:${(seconds % SECONDS_PER_MINUTE).toString().padStart(2, '0')}"
