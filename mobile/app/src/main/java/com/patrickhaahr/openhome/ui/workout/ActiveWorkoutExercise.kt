package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.fitness.ActiveEntry
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.SegmentRing
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.design.monogramOf
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

private val RAIL_RING = 52.dp

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
