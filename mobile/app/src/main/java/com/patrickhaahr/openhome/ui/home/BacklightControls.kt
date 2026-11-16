package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.api.BacklightRgb
import com.patrickhaahr.openhome.api.BacklightSelection
import com.patrickhaahr.openhome.api.BacklightStatus
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import com.patrickhaahr.openhome.ui.workout.CompactValueField
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin

private val WHEEL = 176.dp
private val THUMB_ROOM = 12.dp
private const val FULL_TURN = 360f

/** A draft of light: hue wheel and dimmer change nothing until their key is pressed. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun BacklightInputs(status: BacklightStatus?, enabled: Boolean, select: (BacklightSelection) -> Unit) {
    // Drafts are deliberately separate from server intent; editing never sends a command.
    var red by remember { mutableStateOf("255") }
    var green by remember { mutableStateOf("0") }
    var blue by remember { mutableStateOf("0") }
    var brightness by remember { mutableStateOf("50") }
    val level = brightness.toIntOrNull()?.takeIf { it in 0..MAX_LEVEL }
    val rgb = listOf(red, green, blue).map { it.toIntOrNull()?.takeIf { channel -> channel in 0..MAX_CHANNEL } }
    val target = status?.intended
    val draftColor = if (rgb.all { it != null }) Color(rgb[0]!!, rgb[1]!!, rgb[2]!!) else Lit.Faint

    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        HueWheel(
            color = draftColor,
            onPick = { picked ->
                red = (picked.red * MAX_CHANNEL).roundToInt().toString()
                green = (picked.green * MAX_CHANNEL).roundToInt().toString()
                blue = (picked.blue * MAX_CHANNEL).roundToInt().toString()
            },
            modifier = Modifier.size(WHEEL),
        )
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                CompactValueField(red, { red = it }, "Red", "R", KeyboardType.Number, false, Modifier.weight(1f))
                CompactValueField(green, { green = it }, "Green", "G", KeyboardType.Number, false, Modifier.weight(1f))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                CompactValueField(blue, { blue = it }, "Blue", "B", KeyboardType.Number, false, Modifier.weight(1f))
                Box(Modifier.weight(1f).height(68.dp), contentAlignment = Alignment.Center) {
                    GlyphKey(
                        Glyph.Arrow,
                        "Apply RGB",
                        { select(BacklightSelection.Color(BacklightRgb(rgb[0]!!, rgb[1]!!, rgb[2]!!), level!!)) },
                        tone = KeyTone.Lamp,
                        size = 64.dp,
                        enabled = enabled && level != null && rgb.all { it != null },
                        dimmed = false,
                    )
                }
            }
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Dimmer(
            level = level ?: 0,
            onLevel = { brightness = it.toString() },
            modifier = Modifier.weight(1f).height(68.dp),
        )
        CompactValueField(brightness, { brightness = it }, "Backlight brightness (0–100)", "%", KeyboardType.Number, false, Modifier.width(68.dp))
        GlyphKey(
            Glyph.Check,
            "Apply brightness to intended mode",
            {
                when (target) {
                    is BacklightSelection.TvSync -> select(target.copy(brightness = level!!))
                    is BacklightSelection.Color -> select(target.copy(brightness = level!!))
                    is BacklightSelection.Effect -> select(target.copy(brightness = level!!))
                    else -> Unit
                }
            },
            tone = KeyTone.Raised,
            size = 56.dp,
            enabled = enabled && level != null && target != null && target != BacklightSelection.Off,
        )
    }
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        for (effect in status?.effects.orEmpty()) {
            EffectDisc(
                name = effect.name,
                colors = EFFECT_LIGHT.entries.firstOrNull { effect.id.contains(it.key, ignoreCase = true) }?.value ?: DEFAULT_EFFECT_LIGHT,
                selected = (target as? BacklightSelection.Effect)?.effect == effect.id,
                enabled = enabled && level != null,
                onClick = { select(BacklightSelection.Effect(effect.id, level!!)) },
            )
        }
    }
}

/** An effect is a little disc of its own light, with the name it goes by. */
@Composable
private fun EffectDisc(name: String, colors: List<Color>, selected: Boolean, enabled: Boolean, onClick: () -> Unit) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.width(66.dp)) {
        Canvas(
            Modifier
                .size(56.dp)
                .key("Backlight $name", onClick, enabled)
                .semantics { this.selected = selected },
        ) {
            drawCircle(Brush.sweepGradient(colors + colors.first()))
            if (selected) {
                drawCircle(Lit.Ground, radius = size.minDimension / 2f - 4.dp.toPx(), style = Stroke(3.dp.toPx()))
            }
        }
        Text(name, style = Type.Small, color = if (selected) Lit.Lamp else Lit.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
    }
}

/** Hue around, white at the heart. Choosing paints the draft; it sends nothing. */
@Composable
private fun HueWheel(color: Color, onPick: (Color) -> Unit, modifier: Modifier) {
    val hues = remember { (0..6).map { Color.hsv(it * 60f % FULL_TURN, 1f, 1f) } }
    val hsv = FloatArray(3)
    android.graphics.Color.RGBToHSV(
        (color.red * MAX_CHANNEL).roundToInt(),
        (color.green * MAX_CHANNEL).roundToInt(),
        (color.blue * MAX_CHANNEL).roundToInt(),
        hsv,
    )
    Canvas(
        modifier
            .pointerInput(Unit) {
                fun pick(position: Offset) {
                    val x = position.x - size.width / 2f
                    val y = position.y - size.height / 2f
                    val radius = min(size.width, size.height) / 2f - THUMB_ROOM.toPx()
                    val hue = (atan2(y, x) * 180f / PI.toFloat() + FULL_TURN) % FULL_TURN
                    onPick(Color.hsv(hue, (hypot(x, y) / radius).coerceIn(0f, 1f), 1f))
                }
                awaitEachGesture {
                    val down = awaitFirstDown()
                    down.consume()
                    pick(down.position)
                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.first()
                        if (!change.pressed) break
                        if (change.positionChange() != Offset.Zero) {
                            change.consume()
                            pick(change.position)
                        }
                    }
                }
            },
    ) {
        val radius = size.minDimension / 2f - THUMB_ROOM.toPx()
        drawCircle(Brush.sweepGradient(hues), radius = radius)
        drawCircle(Brush.radialGradient(listOf(Color.White, Color.Transparent), center, radius), radius = radius)
        val turn = hsv[0] * PI.toFloat() / 180f
        val thumb = Offset(center.x + cos(turn) * hsv[1] * radius, center.y + sin(turn) * hsv[1] * radius)
        drawCircle(Lit.Ground, radius = 12.dp.toPx(), center = thumb)
        drawCircle(color, radius = 8.dp.toPx(), center = thumb)
    }
}

/** A thick bar of light that fills with the finger. */
@Composable
internal fun Dimmer(level: Int, onLevel: (Int) -> Unit, modifier: Modifier, fill: Color = Lit.Lamp) {
    Canvas(
        modifier
            .clip(androidx.compose.foundation.shape.RoundedCornerShape(16.dp))
            .pointerInput(Unit) {
                fun pick(position: Offset) = onLevel((position.x / size.width * MAX_LEVEL).roundToInt().coerceIn(0, MAX_LEVEL))
                awaitEachGesture {
                    val down = awaitFirstDown()
                    down.consume()
                    pick(down.position)
                    while (true) {
                        val change = awaitPointerEvent().changes.first()
                        if (!change.pressed) break
                        if (change.positionChange() != Offset.Zero) {
                            change.consume()
                            pick(change.position)
                        }
                    }
                }
            },
    ) {
        drawRect(Lit.Raised)
        drawRoundRect(fill, size = Size(size.width * level / MAX_LEVEL, size.height), cornerRadius = CornerRadius.Zero)
        val x = size.width * level / MAX_LEVEL
        if (level in 1 until MAX_LEVEL) {
            drawLine(Lit.Ink, Offset(x, size.height * 0.25f), Offset(x, size.height * 0.75f), 3.dp.toPx(), StrokeCap.Round)
        }
    }
}
