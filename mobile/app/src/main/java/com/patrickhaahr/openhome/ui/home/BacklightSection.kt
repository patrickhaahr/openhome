package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import java.time.Instant
import kotlinx.coroutines.delay

private const val STALE_AFTER_SECONDS = 30L

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

@Composable
private fun StatusMarks(status: BacklightStatus, outdated: Boolean, stale: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        MarkDot(if (status.reachability == "reachable") Lit.Lamp else Lit.Alarm)
        if (stale || outdated) GlyphIcon(Glyph.Rewind, tint = Lit.Muted, size = 18.dp)
        if (status.setup != "ready") GlyphIcon(Glyph.Warn, tint = Lit.Alarm, size = 18.dp)
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
