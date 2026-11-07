package com.patrickhaahr.openhome.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.patrickhaahr.openhome.R

/**
 * A dark house with one lamp: everything is black until it is lit, and only light is yellow. Surfaces
 * step up in lightness and never carry a line or a shadow.
 */
object Lit {
    val Ground = Color(0xFF000000)
    val Slab = Color(0xFF121210)
    val Raised = Color(0xFF1F1F1C)
    val Lifted = Color(0xFF2E2E2A)
    val Ink = Color(0xFFF8F7EF)
    val Muted = Color(0xFFA6A498)
    val Faint = Color(0xFF6E6C62)
    val Lamp = Color(0xFFFFD60A)
    val LampDeep = Color(0xFFE5BC00)
    val OnLamp = Color(0xFF0C0B00)
    val Glow = Color(0xFF2B2500)
    val Alarm = Color(0xFFFF6B57)
    val AlarmDeep = Color(0xFF3A120C)
}

val Familjen =
    FontFamily(
        Font(R.font.familjen_regular, FontWeight.Normal),
        Font(R.font.familjen_semibold, FontWeight.SemiBold),
    )

/** Everything that counts is a number, and numbers that change must not jitter. */
private const val TABULAR = "tnum"

private fun style(size: Int, line: Int, weight: FontWeight = FontWeight.Normal, tracking: Double = 0.0) =
    TextStyle(
        fontFamily = Familjen,
        fontSize = size.sp,
        lineHeight = line.sp,
        fontWeight = weight,
        letterSpacing = tracking.em,
        fontFeatureSettings = TABULAR,
    )

object Type {
    /** The one enormous thing on a screen: a weight, a countdown, a date. */
    val Giant = style(84, 84, FontWeight.SemiBold, -0.05)
    val Display = style(40, 42, FontWeight.SemiBold, -0.035)
    val Title = style(28, 31, FontWeight.SemiBold, -0.025)
    val Figure = style(24, 28, FontWeight.SemiBold, -0.02)
    val Name = style(17, 22, FontWeight.SemiBold, -0.01)
    val Body = style(15, 21)
    val Small = style(12, 16, FontWeight.SemiBold)
}

private val OpenHomeColors =
    darkColorScheme(
        primary = Lit.Lamp,
        onPrimary = Lit.OnLamp,
        primaryContainer = Lit.Glow,
        onPrimaryContainer = Lit.Lamp,
        secondary = Lit.Lamp,
        onSecondary = Lit.OnLamp,
        secondaryContainer = Lit.Raised,
        onSecondaryContainer = Lit.Ink,
        tertiary = Lit.Lamp,
        onTertiary = Lit.OnLamp,
        background = Lit.Ground,
        onBackground = Lit.Ink,
        surface = Lit.Ground,
        onSurface = Lit.Ink,
        surfaceVariant = Lit.Raised,
        onSurfaceVariant = Lit.Muted,
        surfaceContainerLowest = Lit.Ground,
        surfaceContainerLow = Lit.Slab,
        surfaceContainer = Lit.Slab,
        surfaceContainerHigh = Lit.Slab,
        surfaceContainerHighest = Lit.Raised,
        outline = Lit.Faint,
        outlineVariant = Lit.Raised,
        error = Lit.Alarm,
        onError = Lit.OnLamp,
        errorContainer = Lit.AlarmDeep,
        onErrorContainer = Lit.Alarm,
    )

private val OpenHomeTypography =
    Typography(
        displaySmall = Type.Display,
        headlineLarge = Type.Display,
        headlineSmall = Type.Title,
        titleLarge = Type.Figure,
        titleMedium = Type.Name,
        titleSmall = Type.Name,
        bodyLarge = Type.Body,
        bodyMedium = Type.Body,
        bodySmall = Type.Small,
        labelLarge = Type.Name,
        labelMedium = Type.Small,
        labelSmall = Type.Small,
    )

private val OpenHomeShapes =
    Shapes(
        extraSmall = RoundedCornerShape(10.dp),
        small = RoundedCornerShape(14.dp),
        medium = RoundedCornerShape(20.dp),
        large = RoundedCornerShape(28.dp),
        extraLarge = RoundedCornerShape(36.dp),
    )

/** The only appearance: the app follows no system light theme and no wallpaper colors. */
@Composable
fun OpenHomeTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = OpenHomeColors, typography = OpenHomeTypography, shapes = OpenHomeShapes, content = content)
}
