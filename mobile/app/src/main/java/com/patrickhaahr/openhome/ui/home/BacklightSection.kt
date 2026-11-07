package com.patrickhaahr.openhome.ui.home

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.patrickhaahr.openhome.api.BacklightRgb
import com.patrickhaahr.openhome.api.BacklightSelection
import com.patrickhaahr.openhome.api.BacklightStatus
import com.patrickhaahr.openhome.ui.ProblemTag
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.WithSpoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.MarkDot
import com.patrickhaahr.openhome.ui.design.SendingArc
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import com.patrickhaahr.openhome.ui.workout.CompactValueField
import java.time.Instant
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlinx.coroutines.delay

private val WHEEL = 176.dp
private val THUMB_ROOM = 12.dp
private const val FULL_TURN = 360f
private const val MAX_CHANNEL = 255
private const val MAX_LEVEL = 100
private const val STALE_AFTER_SECONDS = 30L

/** Colours for each effect's halo, by what its light does. An effect not listed gets a neutral warm drift. */
private val EFFECT_LIGHT =
    mapOf(
        "rainbow" to listOf(Color(0xFFFF3B30), Color(0xFF34C759), Color(0xFF0A84FF)),
        "aurora" to listOf(Color(0xFF30D5A0), Color(0xFF0A84FF), Color(0xFF64D2FF)),
        "forest" to listOf(Color(0xFF34C759), Color(0xFF8FB339), Color(0xFF1F7A4D)),
        "fire" to listOf(Color(0xFFFF3B30), Color(0xFFFF9F0A), Color(0xFFFFD60A)),
        "party" to listOf(Color(0xFFFF2D55), Color(0xFF0A84FF), Color(0xFFFFD60A)),
    )
private val DEFAULT_EFFECT_LIGHT = listOf(Lit.Lamp, Color(0xFFFF9F0A), Color(0xFFFFF4C2))
private val TV_SYNC_LIGHT = listOf(Color(0xFFFF7A1A), Color(0xFF3A6EFF), Color(0xFFFF3B6A))

/** The Backlight is the one place colour other than the lamp belongs, because colour is the thing itself. */
private fun BacklightSelection?.lightColors(): List<Color> =
    when (this) {
        null, BacklightSelection.Off -> emptyList()
        is BacklightSelection.Color -> listOf(rgb.toColor())
        is BacklightSelection.TvSync -> TV_SYNC_LIGHT
        is BacklightSelection.Effect -> EFFECT_LIGHT.entries.firstOrNull { effect.contains(it.key, ignoreCase = true) }?.value ?: DEFAULT_EFFECT_LIGHT
    }

private fun BacklightRgb.toColor() = Color(red.coerceIn(0, MAX_CHANNEL), green.coerceIn(0, MAX_CHANNEL), blue.coerceIn(0, MAX_CHANNEL))

private fun BacklightSelection?.level(): Int? =
    when (this) {
        is BacklightSelection.Color -> brightness
        is BacklightSelection.Effect -> brightness
        is BacklightSelection.TvSync -> brightness
        else -> null
    }

@Composable
fun BacklightSection(onOpenAdvanced: () -> Unit = {}, advanced: Boolean = false) {
    val model: BacklightViewModel = appViewModel { BacklightViewModel(it.setup, it.homeHttpClient) }
    val state by model.state.collectAsStateWithLifecycle()
    LaunchedEffect(model) { model.load() }
    var now by remember { mutableStateOf(Instant.now()) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(5000)
            now = Instant.now()
        }
    }
    val status = state.status
    val enabled = state.pending == null
    val observationOld = status.observationIsOld(state.outdated, now)
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (advanced) AdvancedBacklightInputs(status, enabled, model::saveBedtime, model::saveProfile)
        WithSpoken(
            spoken = {
                if (status == null) Spoken("Backlight status unavailable. Refresh to try again.")
                state.pending?.let { Spoken("$it…", live = true) }
                state.message?.let { Spoken(it, live = true) }
                if (status != null) BacklightFacts(status, state.outdated, observationOld)
                if (!advanced) {
                    Spoken("Automatic returns to the server routine. TV Sync restores its saved brightness.")
                    Spoken("RGB channels: 0–255. Brightness: 0–100%. Edits apply only when you choose a control.")
                    Spoken("Available effects")
                    if (status?.effects.isNullOrEmpty()) Spoken("No effects reported. Refresh to check availability.")
                    status?.effects?.forEach { Spoken(it.description) }
                }
            },
        ) {
        Slab(Modifier.fillMaxWidth(), padding = 14.dp) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    Modifier
                        .padding(start = 6.dp)
                        .semantics {
                            contentDescription = "Backlight"
                            heading()
                        },
                ) { GlyphIcon(Glyph.Sun, tint = Lit.Lamp, size = 26.dp) }
                Spacer(Modifier.width(10.dp))
                if (status != null) StatusMarks(status, state.outdated, observationOld)
                Spacer(Modifier.weight(1f))
                GlyphKey(Glyph.Refresh, "Refresh Backlight status", model::refresh, tone = KeyTone.Quiet, size = 44.dp, enabled = enabled, dimmed = false)
                if (!advanced) {
                    GlyphKey(Glyph.Sliders, "Advanced Backlight", onOpenAdvanced, tone = KeyTone.Raised, size = 44.dp, enabled = enabled, dimmed = false)
                }
            }
            LitTv(status?.intended, sending = !enabled, stale = observationOld)
            state.problem?.let { ProblemTag(it) }
            if (advanced && status != null) VisibleFacts(status, observationOld, state.outdated)
            if (!advanced) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    ModeDisc("Off", Glyph.Power, enabled, status?.intended == BacklightSelection.Off) { model.select(BacklightSelection.Off) }
                    ModeDisc("TV Sync", Glyph.Tv, enabled, status?.intended is BacklightSelection.TvSync) { model.select(BacklightSelection.TvSync()) }
                    ModeDisc("Automatic", Glyph.Auto, enabled, status?.control_source == "automatic") { model.automatic() }
                }
                BacklightInputs(status, enabled, model::select)
            }
        }
        }
    }
}

private fun BacklightStatus?.observationIsOld(outdated: Boolean, now: Instant): Boolean {
    if (this == null) return true
    val at = observed?.observed_at ?: return true
    val old = runCatching { Instant.parse(at).plusSeconds(STALE_AFTER_SECONDS).isBefore(now) }.getOrDefault(true)
    return outdated || stale || old
}

/** Marks, not sentences: is the controller there, is the reading old, who is in charge. */
@Composable
private fun StatusMarks(status: BacklightStatus, outdated: Boolean, stale: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        MarkDot(if (status.reachability == "reachable") Lit.Lamp else Lit.Alarm)
        if (stale || outdated) GlyphIcon(Glyph.Rewind, tint = Lit.Muted, size = 18.dp)
        if (status.setup != "ready") GlyphIcon(Glyph.Warn, tint = Lit.Alarm, size = 18.dp)
    }
}

/**
 * The Backlight drawn as what it is: a dark TV on a dark wall with the requested light behind it. It
 * shows what was asked for, never what the strip is known to be doing; a stale reading dims the wall's mark.
 */
@Composable
private fun LitTv(intended: BacklightSelection?, sending: Boolean, stale: Boolean) {
    val colors = intended.lightColors()
    val level = intended.level()
    val moving = intended is BacklightSelection.Effect || intended is BacklightSelection.TvSync
    val orbit = rememberInfiniteTransition(label = "light")
    val angle by orbit.animateFloat(
        0f,
        FULL_TURN,
        infiniteRepeatable(tween(9_000, easing = LinearEasing), RepeatMode.Restart),
        label = "light angle",
    )
    Box(Modifier.fillMaxWidth().height(184.dp), contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) { drawLight(colors, if (moving) angle else 0f, ((level ?: 80) / MAX_LEVEL.toFloat()).coerceIn(0.2f, 1f)) }
        Box(Modifier.size(width = 196.dp, height = 112.dp).background(Lit.Ground, androidx.compose.foundation.shape.RoundedCornerShape(10.dp)), contentAlignment = Alignment.Center) {
            when {
                sending -> SendingArc(Modifier.size(44.dp))
                intended == BacklightSelection.Off -> GlyphIcon(Glyph.Moon, tint = Lit.Faint, size = 34.dp)
                level != null -> Text("$level", style = Type.Display, color = if (stale) Lit.Muted else Lit.Ink)
                else -> GlyphIcon(Glyph.Tv, tint = Lit.Faint, size = 34.dp)
            }
        }
    }
}

private fun DrawScope.drawLight(colors: List<Color>, angle: Float, strength: Float) {
    if (colors.isEmpty()) return
    val reach = size.width * 0.52f
    colors.forEachIndexed { index, color ->
        val turn = (angle + index * FULL_TURN / colors.size) * PI.toFloat() / 180f
        val drift = if (colors.size == 1) 0f else size.width * 0.14f
        val place = Offset(center.x + cos(turn) * drift, center.y + sin(turn) * drift * 0.4f)
        drawCircle(
            Brush.radialGradient(
                0f to color.copy(alpha = strength),
                0.55f to color.copy(alpha = strength * 0.75f),
                1f to Color.Transparent,
                center = place,
                radius = reach,
            ),
            radius = reach,
            center = place,
        )
    }
}

@Composable
private fun ModeDisc(label: String, glyph: Glyph, enabled: Boolean, isSelected: Boolean, onClick: () -> Unit) {
    Box(Modifier.semantics { selected = isSelected }) {
        GlyphKey(
            glyph,
            "Backlight $label",
            onClick,
            tone = if (isSelected) KeyTone.Lamp else KeyTone.Raised,
            size = 72.dp,
            enabled = enabled,
            dimmed = false,
        )
    }
}

/** Every sentence the screen doesn't say out loud, for assistive technology: nothing is claimed by silence. */
@Composable
private fun BacklightFacts(status: BacklightStatus, outdated: Boolean, old: Boolean) {
    Spoken("Intended: ${status.label(status.intended)}")
    Spoken("Last acknowledged: ${status.label(status.acknowledged)}")
    Spoken("Server command: ${status.command} · Recovery: ${status.recovery}")
    Spoken("Backlight connection: ${status.reachability}" + if (outdated) " (last fetched; refresh needed)" else "")
    Spoken("Observation: " + if (old) "Stale / unknown" else "Fresh")
    val observation = status.observed
    if (observation != null) {
        Spoken("Last observed: ${if (observation.power) "On" else "Off"} · ${observation.mode} · ${observation.brightness}%")
        Spoken("Observed at: ${observation.observed_at}")
    } else {
        Spoken("Last observed: Unknown")
    }
    Spoken("Readback cannot confirm exact RGB or effect.")
    if (status.control_source == "manual") {
        Spoken("Manual override: " + (status.override_expires_at?.let { "until $it" } ?: "until another selection"))
    } else {
        Spoken("Control: Automatic")
    }
    Spoken("Server bedtime: ${if (status.bedtime.enabled) "enabled" else "disabled"} · ${status.bedtime.start}–${status.bedtime.end} Europe/Copenhagen")
    status.next_transition?.let { Spoken("Next server transition: $it") }
    if (status.setup != "ready") Spoken("TV Sync profile incomplete. The server may require capture/recovery before selecting a mode.")
}

private fun BacklightStatus.label(selection: BacklightSelection?): String =
    when (selection) {
        null -> "None"
        BacklightSelection.Off -> "Off"
        is BacklightSelection.TvSync -> "TV Sync" + (selection.brightness?.let { " at $it%" } ?: " at saved brightness")
        is BacklightSelection.Color -> "RGB ${selection.rgb.red}, ${selection.rgb.green}, ${selection.rgb.blue} at ${selection.brightness}%"
        is BacklightSelection.Effect -> "${effects.find { it.id == selection.effect }?.name ?: selection.effect} at ${selection.brightness}%"
    }

/** On the Advanced screen the three independent facts are readable: asked, acknowledged, seen. */
@Composable
private fun VisibleFacts(status: BacklightStatus, old: Boolean, outdated: Boolean) {
    FactRow(Glyph.Arrow, status.label(status.intended))
    FactRow(Glyph.Check, status.label(status.acknowledged))
    val observation = status.observed
    FactRow(
        Glyph.Eye,
        observation?.let { "${if (it.power) "On" else "Off"} · ${it.mode} · ${it.brightness}%" } ?: "–",
        dim = old || outdated,
    )
    FactRow(Glyph.Wifi, status.reachability)
}

@Composable
private fun FactRow(glyph: Glyph, value: String, dim: Boolean = false) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.padding(start = 6.dp)) {
        GlyphIcon(glyph, tint = Lit.Muted, size = 20.dp)
        Text(value, style = Type.Body, color = if (dim) Lit.Muted else Lit.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** A draft of light: hue wheel and dimmer change nothing until their key is pressed. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BacklightInputs(status: BacklightStatus?, enabled: Boolean, select: (BacklightSelection) -> Unit) {
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
