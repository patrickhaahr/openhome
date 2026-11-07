package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.api.BacklightBedtimeUpdate
import com.patrickhaahr.openhome.api.BacklightProfileSource
import com.patrickhaahr.openhome.api.BacklightStatus
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.SlabKey
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.workout.LampSwitch
import java.time.LocalTime
import java.time.ZoneId

private val BEDTIME_RED = Color(0xFFFF3B30)
private val HOUSEHOLD_ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")
private const val MINUTES_PER_DAY = 1440f
private const val FULL_TURN = 360f
private const val MIDNIGHT_DEGREES = -90f

@Composable
fun AdvancedBacklightScreen() {
    Column(Modifier.fillMaxSize().imePadding()) {
        ScreenHeader("Advanced Backlight", glyph = Glyph.Sliders)
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = ScreenMargin).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) { BacklightSection(advanced = true) }
    }
}

private fun minutesOf(text: String): Int? =
    runCatching {
        val (hours, minutes) = text.split(":").map { it.trim().toInt() }
        hours * 60 + minutes
    }.getOrNull()

/** The day as a ring with midnight on top: the bedtime window in red, and this minute as the lit dot. */
@Composable
private fun BedtimeDial(start: String, end: String, active: Boolean, modifier: Modifier, center: @Composable () -> Unit) {
    val now = remember { LocalTime.now(HOUSEHOLD_ZONE).let { it.hour * 60 + it.minute } }
    Box(modifier, contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val stroke = 18.dp.toPx()
            val inset = stroke / 2f
            val arcSize = Size(size.width - stroke, size.height - stroke)
            drawCircle(Lit.Raised, radius = size.minDimension / 2f - inset, style = Stroke(stroke))
            val from = minutesOf(start)
            val to = minutesOf(end)
            if (from != null && to != null) {
                val sweep = ((to - from + MINUTES_PER_DAY.toInt()) % MINUTES_PER_DAY.toInt()) / MINUTES_PER_DAY * FULL_TURN
                drawArc(
                    color = if (active) BEDTIME_RED else Lit.Faint,
                    startAngle = MIDNIGHT_DEGREES + from / MINUTES_PER_DAY * FULL_TURN,
                    sweepAngle = sweep,
                    useCenter = false,
                    topLeft = Offset(inset, inset),
                    size = arcSize,
                    style = Stroke(stroke, cap = StrokeCap.Round),
                )
            }
            val turn = Math.toRadians((MIDNIGHT_DEGREES + now / MINUTES_PER_DAY * FULL_TURN).toDouble())
            val radius = size.minDimension / 2f - inset
            drawCircle(
                Lit.Lamp,
                radius = 7.dp.toPx(),
                center = Offset(this.center.x + (radius * Math.cos(turn)).toFloat(), this.center.y + (radius * Math.sin(turn)).toFloat()),
            )
        }
        center()
    }
}

@Composable
internal fun AdvancedBacklightInputs(
    status: BacklightStatus?,
    enabled: Boolean,
    saveBedtime: (BacklightBedtimeUpdate) -> Unit,
    saveProfile: (BacklightProfileSource) -> Unit,
) {
    val bedtime = status?.bedtime
    Column {
    Spoken("Bedtime routine", isHeading = true)
    Spoken("Europe/Copenhagen time, including overnight windows. OpenHome selects red during the enabled window and TV Sync outside it. Scheduling and manual override expiry stay on the server.")
    Spoken("TV Sync setup", isHeading = true)
    Spoken(
        "Profile: " +
            when (status?.setup) {
                "ready" -> "Ready"
                "incomplete" -> "Setup required"
                else -> "Unknown — refresh status"
            },
    )
    Spoken("Capture saves the controller's current TV Sync settings. Activate TV Sync on the controller first. Off remains available in everyday controls during setup.")
    Spoken("Recovery uses the server's private backup. OpenHome checks that it is valid and belongs to the configured controller. Saving a profile does not confirm light output; restoration remains server-owned.")
    }
    if (bedtime != null) {
        var active by remember(bedtime) { mutableStateOf(bedtime.enabled) }
        var start by remember(bedtime) { mutableStateOf(bedtime.start) }
        var end by remember(bedtime) { mutableStateOf(bedtime.end) }
        var brightness by remember(bedtime) { mutableStateOf(bedtime.brightness.toString()) }
        val level = brightness.toIntOrNull()?.takeIf { it in 0..100 }
        Slab(Modifier.fillMaxWidth()) {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                BedtimeDial(start, end, active, Modifier.size(232.dp)) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        GlyphIcon(Glyph.Moon, tint = if (active) BEDTIME_RED else Lit.Faint, size = 32.dp)
                        LampSwitch(active, { active = it }, "Enable bedtime window", enabled = enabled)
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FormField(
                    start, { start = it }, "Start (HH:MM)", glyph = Glyph.Moon,
                    modifier = Modifier.weight(1f), enabled = enabled, keyboardType = KeyboardType.Ascii, well = Lit.Raised,
                )
                FormField(
                    end, { end = it }, "End (HH:MM)", glyph = Glyph.Sun,
                    modifier = Modifier.weight(1f), enabled = enabled, keyboardType = KeyboardType.Ascii, well = Lit.Raised,
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Dimmer(
                    level = level ?: 0,
                    onLevel = { brightness = it.toString() },
                    fill = BEDTIME_RED,
                    modifier = Modifier.weight(1f).height(72.dp),
                )
                FormField(
                    brightness, { brightness = it }, "Bedtime red brightness (0–100)",
                    modifier = Modifier.width(92.dp), enabled = enabled, keyboardType = KeyboardType.Number, well = Lit.Raised,
                )
            }
            SlabKey(
                Glyph.Check,
                "Save bedtime routine",
                {
                    // Submission is enabled only for a parsed brightness in the server's range.
                    saveBedtime(BacklightBedtimeUpdate(active.takeIf { it != bedtime.enabled }, start, end, level!!))
                },
                modifier = Modifier.fillMaxWidth(),
                enabled = enabled && level != null && start.isNotBlank() && end.isNotBlank(),
            )
        }
    }
    Slab(Modifier.fillMaxWidth(), padding = 14.dp) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            GlyphIcon(Glyph.Tv, tint = Lit.Lamp, size = 26.dp, modifier = Modifier.padding(start = 6.dp))
            GlyphIcon(
                when (status?.setup) {
                    "ready" -> Glyph.Check
                    "incomplete" -> Glyph.Warn
                    else -> Glyph.Info
                },
                tint = if (status?.setup == "ready") Lit.Lamp else Lit.Alarm,
                size = 22.dp,
            )
            Box(Modifier.weight(1f))
            GlyphKey(Glyph.Capture, "Capture TV Sync profile", { saveProfile(BacklightProfileSource.Capture) }, tone = KeyTone.Raised, size = 64.dp, enabled = enabled, dimmed = false)
            GlyphKey(Glyph.Restore, "Recover TV Sync backup", { saveProfile(BacklightProfileSource.Backup) }, tone = KeyTone.Raised, size = 64.dp, enabled = enabled, dimmed = false)
        }
    }
}
