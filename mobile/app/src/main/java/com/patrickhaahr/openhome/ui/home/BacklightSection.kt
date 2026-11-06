package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.patrickhaahr.openhome.api.BacklightSelection
import com.patrickhaahr.openhome.api.BacklightRgb
import com.patrickhaahr.openhome.api.BacklightStatus
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.appViewModel
import java.time.Instant
import kotlinx.coroutines.delay

@Composable
fun BacklightSection() {
    val model: BacklightViewModel = appViewModel { BacklightViewModel(it.setup, it.homeHttpClient) }
    val state by model.state.collectAsStateWithLifecycle()
    LaunchedEffect(model) { model.load() }
    var now by remember { mutableStateOf(Instant.now()) }
    LaunchedEffect(Unit) { while (true) { delay(5000); now = Instant.now() } }
    val status = state.status
    val enabled = state.pending == null
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Backlight", style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
        if (status != null) {
            BacklightFacts(status, state.outdated, now)
        } else Text("Backlight status unavailable. Refresh to try again.")
        Column(Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
            state.pending?.let { Text("$it…") }
            state.problem?.let { ProblemText(it) }
        }
        TextButton(onClick = model::refresh, enabled = enabled) { Text("Refresh Backlight status") }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ModeButton("Off", enabled, status?.intended == BacklightSelection.Off) { model.select(BacklightSelection.Off) }
            ModeButton("TV Sync", enabled, status?.intended is BacklightSelection.TvSync) { model.select(BacklightSelection.TvSync()) }
        }
        ModeButton("Automatic", enabled, status?.control_source == "automatic") { model.automatic() }
        Text("Automatic returns to the server routine. TV Sync restores its saved brightness.", style = MaterialTheme.typography.bodySmall)
        BacklightInputs(status, enabled, model::select)
    }
}

@Composable
private fun ModeButton(label: String, enabled: Boolean, isSelected: Boolean, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.heightIn(min = 48.dp).semantics {
            contentDescription = "Backlight $label"
            selected = isSelected
        },
    ) { Text(if (isSelected) "$label · selected" else label) }
}

@Composable
private fun BacklightFacts(status: BacklightStatus, outdated: Boolean, now: Instant) {
    fun label(selection: BacklightSelection?): String = when (selection) {
        null -> "None"
        BacklightSelection.Off -> "Off"
        is BacklightSelection.TvSync -> "TV Sync" + (selection.brightness?.let { " at $it%" } ?: " at saved brightness")
        is BacklightSelection.Color -> "RGB ${selection.rgb.red}, ${selection.rgb.green}, ${selection.rgb.blue} at ${selection.brightness}%"
        is BacklightSelection.Effect -> "${status.effects.find { it.id == selection.effect }?.name ?: selection.effect} at ${selection.brightness}%"
    }
    Text("Intended: ${label(status.intended)}")
    Text("Last acknowledged: ${label(status.acknowledged)}")
    Text("Server command: ${status.command} · Recovery: ${status.recovery}")
    Text("Backlight connection: ${status.reachability}" + if (outdated) " (last fetched; refresh needed)" else "")
    val observation = status.observed
    val old = observation == null || runCatching { Instant.parse(observation.observed_at).plusSeconds(30).isBefore(now) }.getOrDefault(true)
    Text("Observation: " + if (outdated || status.stale || old) "Stale / unknown" else "Fresh")
    if (observation != null) {
        Text("Last observed: ${if (observation.power) "On" else "Off"} · ${observation.mode} · ${observation.brightness}%")
        Text("Observed at: ${observation.observed_at}", style = MaterialTheme.typography.bodySmall)
    } else Text("Last observed: Unknown")
    Text("Readback cannot confirm exact RGB or effect.", style = MaterialTheme.typography.bodySmall)
    if (status.control_source == "manual") {
        Text("Manual override: " + (status.override_expires_at?.let { "until $it" } ?: "until another selection"))
    } else Text("Control: Automatic")
    Text("Server bedtime: ${if (status.bedtime.enabled) "enabled" else "disabled"} · ${status.bedtime.start}–${status.bedtime.end} Europe/Copenhagen")
    status.next_transition?.let { Text("Next server transition: $it", style = MaterialTheme.typography.bodySmall) }
    if (status.setup != "ready") Text("TV Sync profile incomplete. The server may require capture/recovery before selecting a mode.")
}

@Composable
private fun BacklightInputs(status: BacklightStatus?, enabled: Boolean, select: (BacklightSelection) -> Unit) {
    // Drafts are deliberately separate from server intent; editing never sends a command.
    var red by remember { mutableStateOf("255") }
    var green by remember { mutableStateOf("0") }
    var blue by remember { mutableStateOf("0") }
    var brightness by remember { mutableStateOf("50") }
    Text("RGB color")
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for ((label, value, change) in listOf(
            Triple("Red", red, { next: String -> red = next }),
            Triple("Green", green, { next: String -> green = next }),
            Triple("Blue", blue, { next: String -> blue = next }),
        )) OutlinedTextField(
            value = value, onValueChange = change, label = { Text(label) }, singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.weight(1f), enabled = enabled,
        )
    }
    OutlinedTextField(
        value = brightness, onValueChange = { brightness = it }, label = { Text("Backlight brightness (0–100)") },
        singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        modifier = Modifier.fillMaxWidth(), enabled = enabled,
    )
    val level = brightness.toIntOrNull()?.takeIf { it in 0..100 }
    val rgb = listOf(red, green, blue).map { it.toIntOrNull()?.takeIf { channel -> channel in 0..255 } }
    // Enabled controls guarantee parsed channel and brightness ranges before these assertions.
    Button(onClick = {
        select(BacklightSelection.Color(BacklightRgb(rgb[0]!!, rgb[1]!!, rgb[2]!!), level!!))
    }, enabled = enabled && level != null && rgb.all { it != null }) { Text("Apply RGB") }
    Text("RGB channels: 0–255. Brightness: 0–100%. Edits apply only when you choose a control.", style = MaterialTheme.typography.bodySmall)
    val target = status?.intended
    Button(onClick = {
        val updated = when (target) {
            is BacklightSelection.TvSync -> target.copy(brightness = level!!)
            is BacklightSelection.Color -> target.copy(brightness = level!!)
            is BacklightSelection.Effect -> target.copy(brightness = level!!)
            else -> return@Button
        }
        select(updated)
    }, enabled = enabled && level != null && target != null && target != BacklightSelection.Off) { Text("Apply brightness to intended mode") }
    Text("Available effects", style = MaterialTheme.typography.titleMedium)
    if (status?.effects.isNullOrEmpty()) Text("No effects reported. Refresh to check availability.")
    for (effect in status?.effects.orEmpty()) {
        ModeButton(effect.name, enabled && level != null, (target as? BacklightSelection.Effect)?.effect == effect.id) {
            select(BacklightSelection.Effect(effect.id, level!!))
        }
        Text(effect.description, style = MaterialTheme.typography.bodySmall)
    }
}
