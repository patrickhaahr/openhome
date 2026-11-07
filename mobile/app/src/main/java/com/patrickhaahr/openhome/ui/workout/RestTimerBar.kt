package com.patrickhaahr.openhome.ui.workout

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.REST_STEP_SECONDS
import com.patrickhaahr.openhome.fitness.RestTimer
import com.patrickhaahr.openhome.fitness.RestTimers
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.FigureKey
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.Motion
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import kotlin.math.ceil
import kotlin.math.max
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
private val KEY = 44.dp

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

/** A strip of lamp that burns down, over the countdown in type large enough to read from the floor. */
@Composable
private fun RunningBar(rest: RestDisplay.Running, viewModel: RestTimerViewModel, modifier: Modifier) {
    // Only the deadline is stored, so the longest reading so far stands for the whole rest.
    var longest by remember(rest.exerciseId) { mutableLongStateOf(rest.remainingSeconds) }
    longest = max(longest, rest.remainingSeconds)
    val left by animateFloatAsState(
        rest.remainingSeconds.toFloat() / max(longest, 1L),
        tween(Motion.PANEL, easing = Motion.Ease),
        label = "rest left",
    )
    Column(Modifier.fillMaxWidth().background(Lit.Slab).then(modifier)) {
        Canvas(Modifier.fillMaxWidth().height(4.dp)) {
            drawRect(Lit.Lamp, Offset.Zero, Size(size.width * left, size.height))
        }
        Row(
            Modifier.fillMaxWidth().padding(start = ScreenMargin + 4.dp, end = ScreenMargin, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(formatRest(rest.remainingSeconds), style = Type.Display, color = Lit.Lamp)
            Spoken("Rest · ${rest.exerciseName}")
            Spacer(Modifier.weight(1f))
            FigureKey("−15", "Less rest", { viewModel.adjust(rest.exerciseId, -REST_STEP_SECONDS) }, size = KEY + 8.dp)
            FigureKey("+15", "More rest", { viewModel.adjust(rest.exerciseId, REST_STEP_SECONDS) }, size = KEY + 8.dp)
            GlyphKey(Glyph.Skip, "Skip rest", viewModel::end, size = KEY + 8.dp)
            OffKey("Turn off rest for ${rest.exerciseName}") { viewModel.turnOff(rest.exerciseId) }
        }
    }
}

@Composable
private fun OffKey(description: String, onClick: () -> Unit) {
    GlyphKey(Glyph.TimerOff, description, onClick, tone = KeyTone.Quiet, size = KEY)
}

/** Rest is over: the whole bar comes on. */
@Composable
private fun OverBar(rest: RestDisplay.Over, viewModel: RestTimerViewModel, modifier: Modifier) {
    Row(
        Modifier
            .fillMaxWidth()
            .background(Lit.Lamp)
            .then(modifier)
            .padding(start = ScreenMargin + 4.dp, end = ScreenMargin, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.semantics { contentDescription = "Rest over"; liveRegion = LiveRegionMode.Polite }) {
            GlyphIcon(Glyph.Timer, tint = Lit.OnLamp, size = 36.dp)
        }
        Spoken("Rest · ${rest.exerciseName}")
        Spacer(Modifier.weight(1f))
        GlyphKey(Glyph.Check, "Dismiss rest", viewModel::end, tone = KeyTone.Slab, size = 52.dp)
    }
}

internal fun formatRest(seconds: Long): String =
    "${seconds / SECONDS_PER_MINUTE}:${(seconds % SECONDS_PER_MINUTE).toString().padStart(2, '0')}"
