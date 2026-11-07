package com.patrickhaahr.openhome.ui.design

import android.graphics.Paint
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.content.res.ResourcesCompat
import com.patrickhaahr.openhome.R
import com.patrickhaahr.openhome.ui.theme.Lit
import kotlin.math.PI
import kotlin.math.max

private const val FULL_TURN = 360f
private const val TOP_DEGREES = -90f
private const val SEGMENT_GAP_DEGREES = 3f

/** The mark: a weight plate seen end on, whose centre hole is the door of a house. */
@Composable
fun OpenHomeMark(modifier: Modifier = Modifier, disc: Color = Lit.Lamp, door: Color = Lit.Ground) {
    Canvas(modifier) {
        drawCircle(disc)
        drawDoor(door, size.minDimension * 0.31f)
    }
}

private fun DrawScope.drawDoor(color: Color, side: Float) {
    val cx = center.x
    val cy = center.y
    val half = side / 2f
    val door =
        Path().apply {
            moveTo(cx, cy - half * 1.28f)
            lineTo(cx + half * 1.12f, cy - half * 0.28f)
            lineTo(cx + half * 1.12f, cy + half)
            lineTo(cx - half * 1.12f, cy + half)
            lineTo(cx - half * 1.12f, cy - half * 0.28f)
            close()
        }
    drawPath(door, color)
}

/**
 * The Workout screen's start key, and the thing the whole app is named after: a bumper plate, with the
 * last Workout's Exercises cast into its rim. It rolls a little when it arrives, as a plate on a bar does.
 */
@Composable
fun LitPlate(glyph: Glyph, engraved: String?, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val typeface = remember { ResourcesCompat.getFont(context, R.font.familjen_semibold) }
    val roll = remember { Animatable(-38f) }
    LaunchedEffect(Unit) { roll.animateTo(0f, tween(durationMillis = 900, easing = Motion.Ease)) }
    Box(modifier, contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val r = size.minDimension / 2f
            drawCircle(Lit.Lamp)
            drawCircle(Lit.LampDeep, radius = r * 0.885f, style = Stroke(r * 0.008f))
            drawCircle(Lit.LampDeep, radius = r * 0.635f, style = Stroke(r * 0.008f))
            drawCircle(Lit.OnLamp, radius = r * 0.285f)
            drawCircle(Lit.Lamp, radius = r * 0.245f, style = Stroke(r * 0.012f))
            if (engraved != null) {
                rotate(roll.value) { engrave(engraved, r * 0.725f, r * 0.082f, typeface) }
            }
        }
        GlyphIcon(glyph, tint = Lit.Lamp, size = 64.dp)
    }
}

/** Words as a texture: repeated round the whole rim so the casting has no start and no end. */
private fun DrawScope.engrave(text: String, radius: Float, height: Float, typeface: android.graphics.Typeface?) {
    val paint =
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            this.typeface = typeface
            textSize = height
            color = Lit.OnLamp.toArgb()
            letterSpacing = 0.14f
        }
    val circumference = (2 * PI * radius).toFloat()
    val unit = paint.measureText(text)
    val copies = max(1, (circumference / max(unit, 1f)).toInt())
    // Spreading the copies evenly closes the ring exactly, whatever the length of the names.
    paint.letterSpacing += (circumference - unit * copies) / copies / max(text.length, 1) / height
    val arc =
        android.graphics.Path().apply {
            addCircle(center.x, center.y, radius, android.graphics.Path.Direction.CW)
        }
    val line = buildString { repeat(copies) { append(text) } }
    drawContext.canvas.nativeCanvas.drawTextOnPath(line, arc, 0f, 0f, paint)
}

/** An Active Workout as a plate: dark, and lit one segment for each Set confirmed. */
@Composable
fun SegmentRing(
    done: Int,
    total: Int,
    modifier: Modifier = Modifier,
    thickness: Dp = 14.dp,
    lit: Color = Lit.Lamp,
    unlit: Color = Lit.Raised,
) {
    Canvas(modifier) {
        val stroke = thickness.toPx()
        val inset = stroke / 2f
        val arcSize = Size(size.width - stroke, size.height - stroke)
        if (total <= 0) {
            drawCircle(unlit, radius = size.minDimension / 2f - inset, style = Stroke(stroke))
            return@Canvas
        }
        val sweep = FULL_TURN / total
        val gap = if (total == 1) 0f else SEGMENT_GAP_DEGREES
        for (index in 0 until total) {
            drawArc(
                color = if (index < done) lit else unlit,
                startAngle = TOP_DEGREES + index * sweep + gap / 2f,
                sweepAngle = sweep - gap,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = arcSize,
                style = Stroke(stroke, cap = StrokeCap.Butt),
            )
        }
    }
}
