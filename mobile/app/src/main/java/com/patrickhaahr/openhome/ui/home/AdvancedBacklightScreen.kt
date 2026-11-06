package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.api.BacklightBedtimeUpdate
import com.patrickhaahr.openhome.api.BacklightProfileSource
import com.patrickhaahr.openhome.api.BacklightStatus
import com.patrickhaahr.openhome.ui.ScreenHeader

@Composable
fun AdvancedBacklightScreen() {
    Column(Modifier.fillMaxSize().imePadding()) {
        ScreenHeader("Advanced Backlight")
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) { BacklightSection(advanced = true) }
    }
}

@Composable
internal fun AdvancedBacklightInputs(
    status: BacklightStatus?,
    enabled: Boolean,
    saveBedtime: (BacklightBedtimeUpdate) -> Unit,
    saveProfile: (BacklightProfileSource) -> Unit,
) {
    Text("Bedtime routine", style = MaterialTheme.typography.titleMedium, modifier = Modifier.semantics { heading() })
    Text("Europe/Copenhagen time, including overnight windows. OpenHome selects red during the enabled window and TV Sync outside it. Scheduling and manual override expiry stay on the server.")
    val bedtime = status?.bedtime
    if (bedtime != null) {
        var active by remember(bedtime) { mutableStateOf(bedtime.enabled) }
        var start by remember(bedtime) { mutableStateOf(bedtime.start) }
        var end by remember(bedtime) { mutableStateOf(bedtime.end) }
        var brightness by remember(bedtime) { mutableStateOf(bedtime.brightness.toString()) }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("Enable bedtime window")
            Switch(active, { active = it }, enabled = enabled, modifier = Modifier.semantics { contentDescription = "Enable bedtime window" })
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(start, { start = it }, label = { Text("Start (HH:MM)") }, singleLine = true, enabled = enabled, modifier = Modifier.weight(1f))
            OutlinedTextField(end, { end = it }, label = { Text("End (HH:MM)") }, singleLine = true, enabled = enabled, modifier = Modifier.weight(1f))
        }
        OutlinedTextField(
            brightness, { brightness = it }, label = { Text("Bedtime red brightness (0–100)") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true,
            enabled = enabled, modifier = Modifier.fillMaxWidth(),
        )
        val level = brightness.toIntOrNull()?.takeIf { it in 0..100 }
        Text("Use different Start and End times in HH:MM format. Edits send nothing until Save; OpenHome validates the window.", style = MaterialTheme.typography.bodySmall)
        Button(onClick = {
            // Submission is enabled only for a parsed brightness in the server's range.
            saveBedtime(BacklightBedtimeUpdate(active.takeIf { it != bedtime.enabled }, start, end, level!!))
        }, enabled = enabled && level != null && start.isNotBlank() && end.isNotBlank()) { Text("Save bedtime routine") }
    }
    Text("TV Sync setup", style = MaterialTheme.typography.titleMedium, modifier = Modifier.semantics { heading() })
    Text("Profile: " + when (status?.setup) {
        "ready" -> "Ready"
        "incomplete" -> "Setup required"
        else -> "Unknown — refresh status"
    })
    Text("Capture saves the controller's current TV Sync settings. Activate TV Sync on the controller first. Off remains available in everyday controls during setup.")
    Button(onClick = { saveProfile(BacklightProfileSource.Capture) }, enabled = enabled) { Text("Capture TV Sync profile") }
    Text("Recovery uses the server's private backup. OpenHome checks that it is valid and belongs to the configured controller. Saving a profile does not confirm light output; restoration remains server-owned.")
    Button(onClick = { saveProfile(BacklightProfileSource.Backup) }, enabled = enabled) { Text("Recover TV Sync backup") }
}
