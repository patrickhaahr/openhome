package com.patrickhaahr.openhome.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

private val OpenHomeColors =
    darkColorScheme(
        primary = Color(0xFFC6EE6E),
        onPrimary = Color(0xFF172100),
        primaryContainer = Color(0xFF2B3A0E),
        onPrimaryContainer = Color(0xFFDDF5A2),
        secondary = Color(0xFFB9C3CF),
        onSecondary = Color(0xFF1B2128),
        secondaryContainer = Color(0xFF2A3038),
        onSecondaryContainer = Color(0xFFE3E8EE),
        background = Color(0xFF0C0E11),
        onBackground = Color(0xFFF2F4F7),
        surface = Color(0xFF0C0E11),
        onSurface = Color(0xFFF2F4F7),
        surfaceVariant = Color(0xFF1A1E23),
        onSurfaceVariant = Color(0xFFA6AEB8),
        surfaceContainerLowest = Color(0xFF08090B),
        surfaceContainerLow = Color(0xFF12151A),
        surfaceContainer = Color(0xFF161A1F),
        surfaceContainerHigh = Color(0xFF1D2127),
        surfaceContainerHighest = Color(0xFF262B32),
        outline = Color(0xFF3B424B),
        outlineVariant = Color(0xFF2A2F36),
        error = Color(0xFFFF9D8F),
        onError = Color(0xFF3A0B05),
        errorContainer = Color(0xFF3C1712),
        onErrorContainer = Color(0xFFFFD9D3),
    )

private val OpenHomeTypography =
    Typography(
        displaySmall =
            TextStyle(fontSize = 34.sp, lineHeight = 40.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.02).em),
        headlineSmall = TextStyle(fontSize = 24.sp, lineHeight = 30.sp, fontWeight = FontWeight.SemiBold),
        titleMedium = TextStyle(fontSize = 17.sp, lineHeight = 24.sp, fontWeight = FontWeight.SemiBold),
        titleSmall = TextStyle(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.SemiBold),
        bodyLarge = TextStyle(fontSize = 16.sp, lineHeight = 24.sp),
        bodyMedium = TextStyle(fontSize = 14.sp, lineHeight = 20.sp),
        labelMedium = TextStyle(fontSize = 13.sp, lineHeight = 18.sp, fontWeight = FontWeight.Medium),
    )

/** The only appearance: the app follows no system light theme and no wallpaper colors. */
@Composable
fun OpenHomeTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = OpenHomeColors, typography = OpenHomeTypography, content = content)
}
