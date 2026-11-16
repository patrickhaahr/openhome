package com.patrickhaahr.openhome.ui.workout

import android.os.SystemClock
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.fitness.ActiveEntry
import com.patrickhaahr.openhome.fitness.PreviousPerformance
import com.patrickhaahr.openhome.fitness.REST_STEP_SECONDS
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.design.FigureKey
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.SegmentRing
import com.patrickhaahr.openhome.ui.design.SlabKey
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.design.monogramOf
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import kotlinx.coroutines.flow.map

private val RAIL_RING = 52.dp
private const val COMPLETE_GUARD_MILLIS = 400L

/** Every Exercise of the session as a little plate that lights as it fills. Going to one only moves the focus. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ExerciseRail(
    entries: List<ActiveEntry>,
    current: ActiveEntry?,
    onGoTo: (Long) -> Unit,
    onAddExercise: () -> Unit,
    modifier: Modifier = Modifier,
) {
    FlowRow(
        modifier = modifier,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        entries.forEach { entry ->
            val confirmed = entry.sets.count { it.completed }
            val isCurrent = entry.id == current?.id
            val complete = entry.sets.isNotEmpty() && confirmed == entry.sets.size
            Box(
                Modifier
                    .size(RAIL_RING)
                    .key("Go to ${entry.exerciseName}", { onGoTo(entry.id) })
                    .background(if (isCurrent) Lit.Lamp else Color.Transparent, CircleShape)
                    .semantics {
                        val progress = "$confirmed of ${entry.sets.size} Sets confirmed"
                        stateDescription = if (isCurrent) "Current, $progress" else progress
                    },
                contentAlignment = Alignment.Center,
            ) {
                SegmentRing(
                    confirmed,
                    entry.sets.size,
                    Modifier.fillMaxSize().padding(if (isCurrent) 3.dp else 0.dp),
                    thickness = 4.dp,
                    lit = if (isCurrent) Lit.OnLamp else Lit.Lamp,
                    unlit = if (isCurrent) Lit.LampDeep else Lit.Lifted,
                )
                if (complete && !isCurrent) {
                    GlyphIcon(Glyph.Check, tint = Lit.Lamp, size = 22.dp)
                } else {
                    Text(
                        monogramOf(entry.exerciseName),
                        style = Type.Small,
                        color = if (isCurrent) Lit.OnLamp else Lit.Muted,
                    )
                }
                Spoken("${entry.exerciseName} $confirmed/${entry.sets.size}", Modifier.align(Alignment.TopStart))
            }
        }
        GlyphKey(Glyph.Plus, "Add Exercise", onAddExercise, tone = KeyTone.Slab, size = RAIL_RING)
    }
}

/** The one Exercise being lifted, as large as the screen allows. */
@Composable
internal fun ExerciseFocus(
    entry: ActiveEntry,
    previousPerformance: PreviousPerformance?,
    refusedSets: Set<Long>,
    viewModel: ActiveWorkoutViewModel,
) {
    val selectedMeasure = entry.sets.map { it.draft.measure }.distinct().singleOrNull()
    Column(Modifier.fillMaxWidth().padding(horizontal = ScreenMargin), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.Top, modifier = Modifier.padding(start = 4.dp)) {
            Column(Modifier.weight(1f).padding(top = 2.dp)) {
                Text(entry.exerciseName, style = Type.Display, color = Lit.Ink)
                Spoken(entry.exerciseCategory)
            }
            GlyphKey(Glyph.Trash, "Remove ${entry.exerciseName}", { viewModel.removeEntry(entry.id) }, tone = KeyTone.Quiet, size = 44.dp)
        }
        previousPerformance?.takeIf { it.exerciseId == entry.exerciseId }?.let { PreviousPerformanceStrip(it) }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            entry.sets.forEach { set ->
                SetRow(entry.exerciseName, set, showMissingValue = set.id in refusedSets, viewModel)
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            MeasureSwitch(
                selected = selectedMeasure,
                exerciseName = entry.exerciseName,
                onSelect = { viewModel.setMeasure(entry.id, it) },
            )
            Spacer(Modifier.weight(1f))
            if (entry.sets.size > 1) {
                GlyphKey(Glyph.Minus, "Remove last set of ${entry.exerciseName}", { viewModel.removeLastSet(entry.id) }, tone = KeyTone.Slab)
            }
            GlyphKey(Glyph.Copy, "Repeat last set of ${entry.exerciseName}", { viewModel.repeatLastSet(entry.id) }, tone = KeyTone.Slab)
            GlyphKey(Glyph.Plus, "Add set to ${entry.exerciseName}", { viewModel.addSet(entry.id) }, tone = KeyTone.Raised)
        }
        RestPreferenceRow(entry, viewModel)
    }
}

/** Reps or a hold, by their symbols. */
@Composable
internal fun MeasureSwitch(selected: SetMeasure?, exerciseName: String, onSelect: (SetMeasure) -> Unit) {
    Row(Modifier.background(Lit.Slab, CircleShape).padding(4.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        GlyphKey(
            Glyph.Close,
            "Log $exerciseName as Reps",
            { onSelect(SetMeasure.REPS) },
            tone = if (selected == SetMeasure.REPS) KeyTone.Raised else KeyTone.Quiet,
            size = 48.dp,
            glyphSize = 18.dp,
        )
        GlyphKey(
            Glyph.Timer,
            "Log $exerciseName as Hold",
            { onSelect(SetMeasure.HOLD) },
            tone = if (selected == SetMeasure.HOLD) KeyTone.Raised else KeyTone.Quiet,
            size = 48.dp,
            glyphSize = 22.dp,
        )
    }
}

/** What this Exercise's rests will be, remembered from Workout to Workout. */
@Composable
private fun RestPreferenceRow(entry: ActiveEntry, viewModel: ActiveWorkoutViewModel) {
    val spoken = if (entry.rest.enabled) "Rest ${formatRest(entry.rest.seconds)}" else "Rest off"
    Slab(padding = 8.dp, modifier = Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(
                Modifier.padding(start = 10.dp).semantics { contentDescription = spoken },
                contentAlignment = Alignment.Center,
            ) {
                GlyphIcon(Glyph.Timer, tint = if (entry.rest.enabled) Lit.Lamp else Lit.Faint, size = 24.dp)
            }
            if (entry.rest.enabled) {
                // Primes rather than a colon: the running countdown is the only m:ss on screen.
                Text(
                    "${entry.rest.seconds / 60}′${(entry.rest.seconds % 60).toString().padStart(2, '0')}",
                    style = Type.Figure,
                    color = Lit.Ink,
                    modifier = Modifier.padding(start = 4.dp),
                )
            }
            Spacer(Modifier.weight(1f))
            if (entry.rest.enabled) {
                FigureKey("−15", "Less rest for ${entry.exerciseName}", { viewModel.adjustRest(entry.exerciseId, -REST_STEP_SECONDS) }, size = 48.dp)
                FigureKey("+15", "More rest for ${entry.exerciseName}", { viewModel.adjustRest(entry.exerciseId, REST_STEP_SECONDS) }, size = 48.dp)
            }
            LampSwitch(
                checked = entry.rest.enabled,
                onCheckedChange = { viewModel.setRestEnabled(entry.exerciseId, it) },
                description = "Rest timer for ${entry.exerciseName}",
                modifier = Modifier.padding(start = 4.dp, end = 6.dp),
            )
        }
    }
}

@Composable
internal fun LampSwitch(checked: Boolean, onCheckedChange: (Boolean) -> Unit, description: String, modifier: Modifier = Modifier, enabled: Boolean = true) {
    Switch(
        checked = checked,
        onCheckedChange = onCheckedChange,
        enabled = enabled,
        colors =
            SwitchDefaults.colors(
                checkedTrackColor = Lit.Lamp,
                checkedThumbColor = Lit.OnLamp,
                uncheckedTrackColor = Lit.Raised,
                uncheckedThumbColor = Lit.Muted,
                uncheckedBorderColor = Color.Transparent,
            ),
        modifier = modifier.semantics { contentDescription = description },
    )
}

/**
 * The explicit confirmation of everything shown. The next Exercise appears under the same button, so
 * the second half of a double tap must not confirm it unseen.
 */
@Composable
internal fun CompleteExerciseButton(entry: ActiveEntry, viewModel: ActiveWorkoutViewModel, modifier: Modifier) {
    val focusManager = LocalFocusManager.current
    val shownAt = remember(entry.id) { SystemClock.uptimeMillis() }
    val onClick = {
        if (SystemClock.uptimeMillis() - shownAt >= COMPLETE_GUARD_MILLIS) {
            focusManager.clearFocus()
            viewModel.completeExercise(entry.id)
        }
    }
    Box(modifier.fillMaxWidth()) {
        SlabKey(
            Glyph.CheckDouble,
            "Complete ${entry.exerciseName}",
            onClick,
            modifier = Modifier.fillMaxWidth(),
            tone = if (entry.sets.all { it.completed }) KeyTone.Raised else KeyTone.Lamp,
        )
        // Centred, so a tap on these words lands on the key beneath them.
        Spoken("Complete Exercise", Modifier.align(Alignment.Center))
    }
}
