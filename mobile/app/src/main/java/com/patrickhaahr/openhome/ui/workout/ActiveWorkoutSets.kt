package com.patrickhaahr.openhome.ui.workout

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.fitness.ActiveSet
import com.patrickhaahr.openhome.fitness.PreviousPerformance
import com.patrickhaahr.openhome.fitness.SetCheck
import com.patrickhaahr.openhome.fitness.SetField
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.Motion
import com.patrickhaahr.openhome.ui.design.pressable
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatShortDay
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

private val CONFIRM = 52.dp
private val CELL_HEIGHT = 68.dp
private val SET_SHAPE = RoundedCornerShape(24.dp)
private val CELL_SHAPE = RoundedCornerShape(16.dp)

/** Last time, at a glance: the numbers of each Set in a line, marked by the rewind symbol and its day. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun PreviousPerformanceStrip(previous: PreviousPerformance) {
    Row(
        Modifier.padding(start = 4.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        GlyphIcon(Glyph.Rewind, tint = Lit.Muted, size = 18.dp, modifier = Modifier.padding(top = 1.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(formatShortDay(previous.date), style = Type.Small, color = Lit.Muted, modifier = Modifier.padding(top = 2.dp))
            Spoken("Last time · ${formatDay(previous.date)}")
            previous.sets.forEachIndexed { index, set ->
                Text(shortPerformance(set), style = Type.Body, color = Lit.Ink)
                Spoken(
                    listOfNotNull(
                        "Set ${index + 1}: ${describePerformance(set)}",
                        set.rpe?.let { "RPE $it" },
                    ).joinToString(separator = " · "),
                )
            }
        }
    }
}

/** A Set in the fewest marks: 8 ×, 30 s, +20. */
internal fun shortPerformance(set: com.patrickhaahr.openhome.fitness.WorkoutSet): String =
    listOfNotNull(
        set.reps?.let { "$it×" },
        set.durationSeconds?.let { "${it}s" },
        set.addedWeightKg?.let { "+${com.patrickhaahr.openhome.ui.plainNumber(it)}" },
    ).joinToString(" ")

/** A confirmed Set lights up. Nothing else on this screen is as bright, and nothing else needs saying. */
@Composable
internal fun SetRow(
    exerciseName: String,
    set: ActiveSet,
    showMissingValue: Boolean,
    viewModel: ActiveWorkoutViewModel,
) {
    val problem = set.draft.check() as? SetCheck.Problem
    val label = "$exerciseName set ${set.number}"
    val fill by animateColorAsState(if (set.completed) Lit.Lamp else Lit.Slab, tween(Motion.PANEL), label = "set fill")
    val haptics = LocalHapticFeedback.current
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth().background(fill, SET_SHAPE).padding(6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = set.number.toString(),
                style = Type.Name,
                color = if (set.completed) Lit.OnLamp else Lit.Muted,
                textAlign = TextAlign.Center,
                modifier = Modifier.width(26.dp),
            )
            if (set.draft.measure.usesReps) {
                SetValueField(set, SetField.REPS, set.draft.reps, "$label reps", "×", KeyboardType.Number, viewModel)
            }
            if (set.draft.measure.usesSeconds) {
                SetValueField(set, SetField.DURATION, set.draft.durationSeconds, "$label seconds", "s", KeyboardType.Number, viewModel)
            }
            SetValueField(set, SetField.ADDED_WEIGHT, set.draft.addedWeightKg, "$label added kg", "kg", KeyboardType.Decimal, viewModel)
            SetValueField(set, SetField.RPE, set.draft.rpe, "$label RPE", "rpe", KeyboardType.Number, viewModel)
            val source = remember { MutableInteractionSource() }
            Box(
                Modifier
                    .size(CONFIRM)
                    .pressable(source)
                    .clip(CircleShape)
                    .background(if (set.completed) Lit.OnLamp else Lit.Raised)
                    .toggleable(
                        value = set.completed,
                        interactionSource = source,
                        indication = LocalIndication.current,
                        role = Role.Checkbox,
                        onValueChange = {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            viewModel.setCompleted(set.id, it)
                        },
                    ).semantics { contentDescription = "$label performed" },
                contentAlignment = Alignment.Center,
            ) {
                GlyphIcon(Glyph.Check, tint = if (set.completed) Lit.Lamp else Lit.Muted, size = 26.dp)
            }
        }
        if (problem != null && (!problem.isMissingValue || showMissingValue)) {
            ProblemText(problem.message, Modifier.padding(start = 12.dp, bottom = 4.dp))
        }
    }
}

/** Keeps its own text, so a save finishing later can never move the cursor or undo typing. */
@Composable
private fun RowScope.SetValueField(
    set: ActiveSet,
    field: SetField,
    stored: String,
    description: String,
    unit: String,
    keyboard: KeyboardType,
    viewModel: ActiveWorkoutViewModel,
) {
    var text by rememberSaveable(set.id, field) { mutableStateOf(stored) }
    CompactValueField(
        value = text,
        onValueChange = {
            text = it
            viewModel.editSet(set.id, field, it)
        },
        description = description,
        unit = unit,
        keyboard = keyboard,
        highlighted = set.completed,
        modifier = Modifier.weight(1f),
    )
}

/** A figure in a well, with its unit tucked in the corner. Its description labels it the way a heading would. */
@Composable
internal fun CompactValueField(
    value: String,
    onValueChange: (String) -> Unit,
    description: String,
    unit: String,
    keyboard: KeyboardType,
    highlighted: Boolean,
    modifier: Modifier = Modifier,
) {
    val ink = if (highlighted) Lit.OnLamp else Lit.Ink
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        textStyle = Type.Figure.copy(color = ink, textAlign = TextAlign.Center),
        cursorBrush = SolidColor(if (highlighted) Lit.OnLamp else Lit.Lamp),
        keyboardOptions = KeyboardOptions(keyboardType = keyboard),
        modifier =
            modifier
                .heightIn(min = CELL_HEIGHT)
                .background(if (highlighted) Lit.LampDeep else Lit.Raised, CELL_SHAPE),
        decorationBox = { inner ->
            Box(
                modifier =
                    Modifier
                        .heightIn(min = CELL_HEIGHT)
                        .padding(horizontal = 4.dp)
                        .semantics { contentDescription = description },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    unit,
                    style = Type.Small,
                    color = if (highlighted) Lit.OnLamp else Lit.Muted,
                    modifier = Modifier.align(Alignment.TopCenter).padding(top = 7.dp),
                )
                Box(Modifier.padding(top = 14.dp), contentAlignment = Alignment.Center) {
                    if (value.isEmpty()) {
                        Text("–", style = Type.Figure, color = if (highlighted) Lit.OnLamp.copy(alpha = 0.45f) else Lit.Faint)
                    }
                    inner()
                }
            }
        },
    )
}
