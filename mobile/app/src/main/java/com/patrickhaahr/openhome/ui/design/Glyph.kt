package com.patrickhaahr.openhome.ui.design

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.ui.theme.Lit

private const val GRID = 24f
private const val STROKE = 2.1f

private const val DISC_8 = "M12 15a3 3 0 1 0 0-6a3 3 0 1 0 0 6z"
private const val CLOUD = "M7 18.5a4.5 4.5 0 0 1-.5-9A6 6 0 0 1 18 9a4.8 4.8 0 0 1-.5 9.5z"
private const val RAYS =
    "M12 2.5v2 M12 19.5v2 M2.5 12h2 M19.5 12h2 M5.3 5.3l1.4 1.4 M17.3 17.3l1.4 1.4 M5.3 18.7l1.4-1.4 M17.3 6.7l1.4-1.4"

/** The app's own symbols: one 24-unit grid, one stroke, round ends. They are its only labels. */
enum class Glyph(val path: String, val filled: Boolean = false) {
    Barbell("M3 9.5v5 M21 9.5v5 M6.5 6v12 M17.5 6v12 M6.5 12h11"),
    Rings("M8 3v7 M16 3v7 M8 10a3.4 3.4 0 1 0 0 6.8a3.4 3.4 0 1 0 0-6.8z M16 10a3.4 3.4 0 1 0 0 6.8a3.4 3.4 0 1 0 0-6.8z"),
    House("M3.5 11.5L12 4l8.5 7.5 M6 9.5V20h12V9.5 M10 20v-5.5h4V20"),
    Person("M12 12a4 4 0 1 0 0-8a4 4 0 1 0 0 8z M4.5 20.5c1-4.2 4-6 7.5-6s6.5 1.8 7.5 6"),
    Repeat("M17 2.5l4 4-4 4 M3 11.5V9.5a3 3 0 0 1 3-3h15 M7 21.5l-4-4 4-4 M21 12.5v2a3 3 0 0 1-3 3H3"),
    Plus("M12 5v14 M5 12h14"),
    Minus("M5 12h14"),
    Check("M4.5 12.5l5 5L19.5 7"),
    CheckDouble("M2 12.5l5 5L17 7 M11 17l.5.5L22 7"),
    Close("M6 6l12 12 M18 6L6 18"),
    Trash("M4 7h16 M9 7V4h6v3 M6 7l1 13.5h10L18 7 M10 11.5v5 M14 11.5v5"),
    Flag("M5.5 21.5V3.5 M5.5 4h13l-3 4.5 3 4.5h-13"),
    Pencil("M4 20l1-4.5L16 4.5l3.5 3.5L8.5 19z M13.5 7l3.5 3.5"),
    Calendar("M4 6h16v14.5H4z M4 10.5h16 M8.5 3v4.5 M15.5 3v4.5"),
    Search("M10.5 18a7.5 7.5 0 1 0 0-15a7.5 7.5 0 1 0 0 15z M16 16l5 5"),
    Rewind("M3.5 12a8.5 8.5 0 1 0 2.8-6.3 M3.5 3.5v5h5 M12 7.5V12l3.5 2"),
    Stack("M5 4v16.5 M10 4v16.5 M14.5 4.5l4.8 15.8"),
    CloudDone("$CLOUD M9 13.5l2.2 2.2 4-4.2"),
    CloudUp("$CLOUD M12 16v-5 M9.5 13l2.5-2.5 2.5 2.5"),
    CloudWait("$CLOUD M9 14h.01 M12 14h.01 M15 14h.01"),
    Wifi("M2.5 9.5a14 14 0 0 1 19 0 M5.5 13a9.5 9.5 0 0 1 13 0 M8.5 16.5a5 5 0 0 1 7 0 M12 20h.01"),
    Warn("M12 3.5l10 17H2z M12 10v4.5 M12 17.5h.01"),
    Key("M8 21a5 5 0 1 0 0-10a5 5 0 1 0 0 10z M11.5 12.5L20.5 3.5 M16 8l3.5 3.5"),
    Globe("M12 21a9 9 0 1 0 0-18a9 9 0 1 0 0 18z M3 12h18 M12 3c3.5 3 3.5 15 0 18 M12 3c-3.5 3-3.5 15 0 18"),
    Power("M12 3v9 M6.5 6.5a8 8 0 1 0 11 0"),
    Input("M3.5 8V5h17v14h-17v-3 M2 12h11 M10 8.5l3.5 3.5-3.5 3.5"),
    Mute("M3.5 9.5v5H7l5 4.5v-14L7 9.5z M16 9.5l5 5 M21 9.5l-5 5"),
    Speaker("M6.5 3h11v18h-11z M12 18a3.2 3.2 0 1 0 0-6.4a3.2 3.2 0 1 0 0 6.4z M12 7h.01"),
    Tv("M3 5h18v12.5H3z M8 21h8 M12 17.5V21"),
    Bulb("M9.5 18h5 M10.5 21.5h3 M9.5 18c0-2.5-3.5-3.8-3.5-8a6 6 0 0 1 12 0c0 4.2-3.5 5.5-3.5 8"),
    BulbLit("M9.8 18.5h4.4 M10.6 21.5h2.8 M9.8 18.5c0-2-2.8-3-2.8-6.2a5 5 0 0 1 10 0c0 3.2-2.8 4.2-2.8 6.2 M12 2.5v2 M3.5 12.5h1.7 M18.8 12.5h1.7 M5.8 5.8l1.2 1.2 M18.2 5.8L17 7"),
    Bluetooth("M6.5 7.5l11 9-5.5 5V2.5l5.5 5-11 9"),
    Optical("M12 16.5a4.5 4.5 0 1 0 0-9a4.5 4.5 0 1 0 0 9z M12 12h.01 M2.5 12h3 M18.5 12h3"),
    Up("M5.5 15l6.5-6.5 6.5 6.5"),
    Down("M5.5 9l6.5 6.5L18.5 9"),
    Left("M15 5.5L8.5 12l6.5 6.5"),
    Right("M9 5.5l6.5 6.5L9 18.5"),
    Back("M9 4.5l-5 5 5 5 M4 9.5h10.5a5 5 0 0 1 0 10H11"),
    Exit("M10 4H4v16h6 M21 12H9.5 M17 8l4 4-4 4"),
    Sliders("M4 7h8.5 M17 7h3 M4 17h3 M11.5 17H20 M14.7 4.5v5 M9.2 14.5v5"),
    Guide("M4 5.5h16 M4 12h16 M4 18.5h10"),
    Info("M12 21a9 9 0 1 0 0-18a9 9 0 1 0 0 18z M12 11v5.5 M12 7.5h.01"),
    Remote("M7.5 2.5h9v19h-9z M12 6.5h.01 M10.2 11h3.6 M10.2 14.2h3.6 M10.2 17.4h3.6"),
    Auto("M3.5 20.5L9 6h1.5L16 20.5 M5.5 15.5h8.5 M19.5 3v5 M17 5.5h5"),
    Moon("M20.5 14.5A8.8 8.8 0 1 1 9.5 3.5a7 7 0 0 0 11 11z"),
    Sun("M12 16.5a4.5 4.5 0 1 0 0-9a4.5 4.5 0 1 0 0 9z $RAYS"),
    Capture("M3.5 8V3.5H8 M16 3.5h4.5V8 M20.5 16v4.5H16 M8 20.5H3.5V16 $DISC_8"),
    Restore("M3.5 4h17v5h-17z M5.5 9v11.5h13V9 M12 18v-5.5 M9.2 15L12 12.2l2.8 2.8"),
    Refresh("M20 11.5a8 8 0 1 0-2.2 6 M20.5 4v7.5H13"),
    Timer("M12 21.5a8 8 0 1 0 0-16a8 8 0 1 0 0 16z M12 9.5v4l2.5 1.5 M9 2.5h6"),
    TimerOff("M12 21.5a8 8 0 1 0 0-16a8 8 0 1 0 0 16z M9 2.5h6 M4 4l16 18"),
    Copy("M8.5 8.5h12v12h-12z M15.5 8.5v-5h-12v12h5"),
    Gauge("M3.8 18.5a9.5 9.5 0 1 1 16.4 0 M12 14.5l4-5.5 M12 14.5h.01"),
    Kettlebell("M8.5 9a3.5 3.5 0 1 1 7 0 M7 9.5h10l3 11H4z"),
    Ruler("M2.5 8h19v8h-19z M6.5 8v3 M10.5 8v4.5 M14.5 8v3 M18.5 8v4.5"),
    Heart("M12 20.5s-8.5-5.2-8.5-11.2A4.7 4.7 0 0 1 12 6.5a4.7 4.7 0 0 1 8.5 2.8c0 6-8.5 11.2-8.5 11.2z"),
    Cake("M3.5 20.5h17 M5 20.5v-8.5h14v8.5 M5 16c2.3 2 3.5 2 5 .5s3.5-1.5 5 0 2.7 1.5 4-.5 M12 12V8 M12 4.5h.01"),
    Male("M10 20.5a6.5 6.5 0 1 0 0-13a6.5 6.5 0 1 0 0 13z M14.7 9L20.5 3.5 M15 3.5h5.5V9"),
    Female("M12 14.5a5.5 5.5 0 1 0 0-11a5.5 5.5 0 1 0 0 11z M12 14.5v7 M8.5 18.2h7"),
    Scale("M4 3.5h16v17H4z M8 7.5h8v4.5H8z"),
    Note("M5 3.5h14v17H5z M8.5 8.5h7 M8.5 12.5h7 M8.5 16.5h4"),
    ArrowUp("M12 20V4.5 M5.5 11l6.5-6.5 6.5 6.5"),
    ArrowDown("M12 4v15.5 M5.5 13l6.5 6.5 6.5-6.5"),
    Arrow("M4 12h15.5 M13 5.5l6.5 6.5-6.5 6.5"),
    Play("M7 4l13 8-13 8z", filled = true),
    Skip("M5 5l10.5 7L5 19z M19.5 5v14"),
    Eye("M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z $DISC_8"),
    EyeOff("M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z $DISC_8 M4 4l16 16"),
    Download("M12 3v12.5 M6.5 10.5l5.5 5.5 5.5-5.5 M4 20.5h16"),
    ;

    /** Parsed once: glyphs are drawn on every frame of a pressed key. */
    val shape: Path by lazy { PathParser().parsePathString(path).toPath() }
}

/** Decorative by itself: the control it sits on carries the name. */
@Composable
fun GlyphIcon(glyph: Glyph, modifier: Modifier = Modifier, tint: Color = Lit.Ink, size: Dp = 24.dp) {
    Canvas(modifier.size(size)) {
        scale(this.size.minDimension / GRID, pivot = Offset.Zero) {
            drawPath(
                path = glyph.shape,
                color = tint,
                style =
                    if (glyph.filled) {
                        Fill
                    } else {
                        Stroke(width = STROKE, cap = StrokeCap.Round, join = StrokeJoin.Round)
                    },
            )
        }
    }
}
