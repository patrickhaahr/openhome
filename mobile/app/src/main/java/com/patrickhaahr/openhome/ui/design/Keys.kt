package com.patrickhaahr.openhome.ui.design

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

object Motion {
    /** One ease for the whole app: things rise and settle. */
    val Ease = CubicBezierEasing(0.2f, 0.7f, 0.1f, 1f)
    const val QUICK = 180
    const val PANEL = 320
}

enum class KeyTone(val fill: Color, val ink: Color) {
    Lamp(Lit.Lamp, Lit.OnLamp),
    Slab(Lit.Slab, Lit.Ink),
    Raised(Lit.Raised, Lit.Ink),
    Bare(Color.Transparent, Lit.Ink),
    Quiet(Color.Transparent, Lit.Muted),
    Alarm(Lit.AlarmDeep, Lit.Alarm),
}

/** Every control gives under the thumb. */
fun Modifier.pressable(source: MutableInteractionSource, depth: Float = 0.94f): Modifier =
    composed {
        val pressed by source.collectIsPressedAsState()
        val scale by animateFloatAsState(if (pressed) depth else 1f, tween(Motion.QUICK, easing = Motion.Ease), label = "press")
        graphicsLayer {
            scaleX = scale
            scaleY = scale
        }
    }

/** The one way to make a tappable shape: pressed depth, haptic tick, and the name for those who can't see it. */
fun Modifier.key(
    description: String,
    onClick: () -> Unit,
    enabled: Boolean = true,
    shape: Shape = CircleShape,
    depth: Float = 0.94f,
): Modifier =
    composed {
        val source = remember { MutableInteractionSource() }
        val haptics = LocalHapticFeedback.current
        this
            .pressable(source, depth)
            .clip(shape)
            .clickable(
                interactionSource = source,
                indication = androidx.compose.foundation.LocalIndication.current,
                enabled = enabled,
                role = Role.Button,
                onClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onClick()
                },
            ).semantics { contentDescription = description }
    }

/** A glyph on a disc: the app's button. Dims only when the thing can't be done, never while it is being done. */
@Composable
fun GlyphKey(
    glyph: Glyph,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tone: KeyTone = KeyTone.Raised,
    size: Dp = 56.dp,
    glyphSize: Dp = size * 0.43f,
    shape: Shape = CircleShape,
    enabled: Boolean = true,
    dimmed: Boolean = !enabled,
    phase: CommandPhase = CommandPhase.Idle,
    pulse: Int = 0,
) {
    val alpha = if (dimmed) 0.32f else 1f
    CommandHalo(phase, pulse, modifier.size(size)) {
        Box(
            Modifier
                .fillMaxSize()
                .key(description, onClick, enabled, shape)
                .background(tone.fill.copy(alpha = tone.fill.alpha * alpha), shape),
            contentAlignment = Alignment.Center,
        ) {
            GlyphIcon(glyph, tint = tone.ink.copy(alpha = alpha), size = glyphSize)
        }
    }
}

/** A key whose symbol is a figure or a couple of letters: 1 to 9, HDMI numbers, −15. */
@Composable
fun FigureKey(
    figure: String,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tone: KeyTone = KeyTone.Raised,
    size: Dp = 56.dp,
    enabled: Boolean = true,
    dimmed: Boolean = !enabled,
    phase: CommandPhase = CommandPhase.Idle,
    pulse: Int = 0,
) {
    val alpha = if (dimmed) 0.32f else 1f
    CommandHalo(phase, pulse, modifier.size(size)) {
        Box(
            Modifier
                .fillMaxSize()
                .key(description, onClick, enabled)
                .background(tone.fill.copy(alpha = tone.fill.alpha * alpha), CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Text(figure, style = Type.Name, color = tone.ink.copy(alpha = alpha))
        }
    }
}

/** The big one at the foot of a form or a screen: full width, one symbol, always in reach. */
@Composable
fun SlabKey(
    glyph: Glyph,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tone: KeyTone = KeyTone.Lamp,
    enabled: Boolean = true,
    height: Dp = 72.dp,
) {
    val fill by animateColorAsState(if (enabled) tone.fill else Lit.Raised, tween(Motion.PANEL), label = "slab fill")
    val shape = androidx.compose.foundation.shape.RoundedCornerShape(height / 2)
    Box(
        modifier
            .heightIn(min = height)
            .widthIn(min = height)
            .key(description, onClick, enabled, shape, depth = 0.97f)
            .background(fill, shape),
        contentAlignment = Alignment.Center,
    ) {
        GlyphIcon(glyph, tint = if (enabled) tone.ink else Lit.Muted, size = 32.dp)
    }
}

/** What a sent command is doing, as far as the key can honestly say. Never the device's state. */
enum class CommandPhase {
    Idle,
    Sending,
    Acknowledged,
    Failed,
}

/**
 * Draws a sent command on the key itself: a lit arc spinning round it while waiting, one ring rippling
 * out when acknowledged, a shake and a red ring when it failed. "Acknowledged" is the API's word, not
 * the device's: nothing here says on or off.
 */
@Composable
fun CommandHalo(phase: CommandPhase, pulse: Int, modifier: Modifier, content: @Composable () -> Unit) {
    val shake = remember { Animatable(0f) }
    val ripple = remember { Animatable(1f) }
    LaunchedEffect(phase, pulse) {
        when (phase) {
            CommandPhase.Failed ->
                shake.animateTo(
                    0f,
                    keyframes {
                        durationMillis = 360
                        -8f at 60
                        8f at 130
                        -5f at 200
                        4f at 280
                    },
                )
            CommandPhase.Acknowledged -> {
                ripple.snapTo(0f)
                ripple.animateTo(1f, tween(520, easing = Motion.Ease))
            }
            else -> Unit
        }
    }
    Box(modifier.graphicsLayer { translationX = shake.value * density }, contentAlignment = Alignment.Center) {
        content()
        if (phase == CommandPhase.Sending) SendingArc(Modifier.fillMaxSize())
        if (phase == CommandPhase.Failed) {
            Canvas(Modifier.fillMaxSize()) {
                drawCircle(Lit.Alarm, radius = size.minDimension / 2f - 1.dp.toPx(), style = Stroke(2.dp.toPx()))
            }
        }
        if (ripple.value < 1f) {
            Canvas(Modifier.fillMaxSize()) {
                drawCircle(
                    Lit.Lamp.copy(alpha = 1f - ripple.value),
                    radius = size.minDimension / 2f * (1f + ripple.value * 0.35f),
                    style = Stroke(3.dp.toPx()),
                )
            }
        }
    }
}

@Composable
fun SendingArc(modifier: Modifier = Modifier, color: Color = Lit.Lamp, stroke: Dp = 3.dp) {
    val spin = rememberInfiniteTransition(label = "sending")
    val angle by spin.animateFloat(
        0f,
        360f,
        infiniteRepeatable(tween(900, easing = LinearEasing), RepeatMode.Restart),
        label = "sending angle",
    )
    Canvas(modifier) {
        val width = stroke.toPx()
        drawArc(
            color = color,
            startAngle = angle,
            sweepAngle = 90f,
            useCenter = false,
            topLeft = Offset(width / 2f, width / 2f),
            size = androidx.compose.ui.geometry.Size(size.width - width, size.height - width),
            style = Stroke(width, cap = StrokeCap.Round),
        )
    }
}

/**
 * Follows one section's single in-flight command, so only the key that was pressed reacts to it. The
 * section's view model says busy, acknowledged or failed; this remembers whose press that was about.
 */
class CommandTracker {
    private var lastKey by mutableIntStateOf(0)
    private var presses by mutableIntStateOf(0)

    val pulse: Int
        get() = presses

    fun press(id: String) {
        lastKey = id.hashCode()
        presses += 1
    }

    fun phase(id: String, busy: Boolean, acknowledged: Boolean, failed: Boolean): CommandPhase =
        when {
            lastKey != id.hashCode() -> CommandPhase.Idle
            busy -> CommandPhase.Sending
            failed -> CommandPhase.Failed
            acknowledged -> CommandPhase.Acknowledged
            else -> CommandPhase.Idle
        }
}

/** The small status mark beside a figure or a title. */
@Composable
fun MarkDot(color: Color, modifier: Modifier = Modifier, size: Dp = 8.dp) {
    Box(modifier.size(size).background(color, CircleShape))
}

@Composable
fun Monogram(name: String, modifier: Modifier = Modifier, color: Color = Lit.Ink) {
    Text(monogramOf(name), style = Type.Small, color = color, modifier = modifier)
}

/** "Pull-Up" is PU, "Planche Hold" is PH: enough to find an Exercise among those of the day. */
fun monogramOf(name: String): String {
    val parts = name.split(' ', '-', '/').filter { it.isNotBlank() }
    return when {
        parts.size >= 2 -> "${parts[0].first()}${parts[1].first()}"
        name.length >= 2 -> name.take(2)
        else -> name
    }.uppercase()
}
