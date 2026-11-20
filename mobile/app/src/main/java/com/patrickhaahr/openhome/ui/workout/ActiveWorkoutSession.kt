package com.patrickhaahr.openhome.ui.workout

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.fitness.ActiveWorkout
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.MarkDot
import com.patrickhaahr.openhome.ui.design.Motion
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatShortDay
import com.patrickhaahr.openhome.ui.pluralize
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

@Composable
internal fun SessionBar(
    workout: ActiveWorkout,
    saveState: SaveState,
    viewModel: ActiveWorkoutViewModel,
    onChooseDate: () -> Unit,
    onDiscard: () -> Unit,
    onFinish: () -> Unit,
) {
    var name by rememberSaveable { mutableStateOf(workout.name) }
    var notes by rememberSaveable { mutableStateOf(workout.notes) }
    var editing by rememberSaveable { mutableStateOf(false) }
    Column(
        Modifier.fillMaxWidth().padding(start = ScreenMargin, end = ScreenMargin, top = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Spoken("Active Workout", isHeading = true)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(
                Modifier
                    .height(48.dp)
                    .key(formatDay(workout.date), onChooseDate, shape = CircleShape)
                    .background(Lit.Slab, CircleShape)
                    .padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                GlyphIcon(Glyph.Calendar, tint = Lit.Muted, size = 18.dp)
                Text(formatShortDay(workout.date), style = Type.Name, color = Lit.Ink)
            }
            GlyphKey(
                if (editing) Glyph.Check else if (notes.isNotBlank() && name.isBlank()) Glyph.Note else Glyph.Pencil,
                when {
                    editing -> "Done"
                    name.isNotBlank() -> name
                    notes.isNotBlank() -> "Edit notes"
                    else -> "Add name or notes"
                },
                { editing = !editing },
                tone = if (editing) KeyTone.Lamp else KeyTone.Slab,
                size = 48.dp,
            )
            SaveMark(saveState)
            Spacer(Modifier.weight(1f))
            GlyphKey(Glyph.Trash, "Discard Workout", onDiscard, tone = KeyTone.Quiet, size = 48.dp)
            GlyphKey(
                Glyph.Flag,
                "Finish Workout",
                onFinish,
                tone = if (workout.hasOnlyConfirmedSets()) KeyTone.Lamp else KeyTone.Raised,
                size = 56.dp,
            )
        }
        if (saveState == SaveState.FAILED) ProblemText("Couldn't save on device. Your last change may be lost.")
        if (editing) {
            FormField(
                value = name,
                onValueChange = {
                    name = it
                    viewModel.setName(it)
                },
                label = "Name (optional)",
                glyph = Glyph.Pencil,
            )
            FormField(
                value = notes,
                onValueChange = {
                    notes = it
                    viewModel.setNotes(it)
                },
                label = "Notes (optional)",
                glyph = Glyph.Note,
                singleLine = false,
            )
        }
    }
}

@Composable
private fun SaveMark(state: SaveState) {
    val label =
        when (state) {
            SaveState.SAVING -> "Saving on device"
            SaveState.SAVED -> "Saved on device"
            SaveState.FAILED -> "Couldn't save on device. Your last change may be lost."
        }
    val color by animateColorAsState(
        when (state) {
            SaveState.SAVING -> Lit.Lamp
            SaveState.SAVED -> Lit.Lifted
            SaveState.FAILED -> Lit.Alarm
        },
        tween(Motion.QUICK),
        label = "save mark",
    )
    Box(Modifier.size(20.dp).semantics { contentDescription = label }, contentAlignment = Alignment.Center) {
        MarkDot(color)
    }
}

@Composable
internal fun FinishReview(workout: ActiveWorkout, onReturn: () -> Unit, onOmitAndFinish: () -> Unit) {
    val confirmed = workout.entries.sumOf { entry -> entry.sets.count { it.completed } }
    val unchecked =
        workout.entries.mapNotNull { entry ->
            entry.sets.filterNot { it.completed }.takeIf { it.isNotEmpty() }?.let { sets -> entry.exerciseName to sets.size }
        }
    KeyDialog(
        onDismiss = onReturn,
        title = if (confirmed == 0) "No confirmed Sets" else "Finish with unchecked Sets?",
        message =
            if (confirmed == 0) {
                "Check at least one performed Set to finish this Workout."
            } else {
                "${pluralize(confirmed, "confirmed Set")} will be recorded. These will be left out:"
            },
        glyph = Glyph.Flag,
        confirm = {
            if (confirmed > 0) GlyphKey(Glyph.Flag, "Omit and finish", onOmitAndFinish, tone = KeyTone.Lamp)
        },
        dismiss = { GlyphKey(Glyph.Back, "Return to logging", onReturn) },
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            GlyphIcon(Glyph.Check, tint = Lit.Lamp, size = 32.dp)
            Text(confirmed.toString(), style = Type.Display, color = Lit.Ink)
        }
        if (confirmed > 0) {
            unchecked.forEach { (name, count) ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    GlyphIcon(Glyph.Close, tint = Lit.Alarm, size = 20.dp)
                    Text(count.toString(), style = Type.Name, color = Lit.Ink)
                    Text(name, style = Type.Body, color = Lit.Muted)
                    Spoken("$name: ${pluralize(count, "unchecked Set")}")
                }
            }
        }
    }
}
