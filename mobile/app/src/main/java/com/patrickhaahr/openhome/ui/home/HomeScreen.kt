package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel

private data class TvButton(val command: String, val label: String)
private fun key(command: String, label: String) = TvButton(command, label)
private val frequent = listOf(
    listOf(key("power", "Power"), key("input", "Input")),
    listOf(key("volume-down", "Volume −"), key("mute", "Mute"), key("volume-up", "Volume +")),
)
private val directions = listOf(
    listOf(null, key("up", "Up"), null),
    listOf(key("left", "Left"), key("ok", "OK"), key("right", "Right")),
    listOf(null, key("down", "Down"), null),
)
private val more = listOf(
    listOf(key("back", "Back"), key("home", "TV Home"), key("exit", "Exit")),
    listOf(key("settings", "Settings"), key("guide", "Guide"), key("info", "Info")),
    listOf(key("channel-down", "Channel −"), key("channel-up", "Channel +")),
    listOf(key("hdmi-1", "HDMI 1"), key("hdmi-2", "HDMI 2"), key("hdmi-3", "HDMI 3")),
) + (1..9).chunked(3).map { row -> row.map { key(it.toString(), it.toString()) } } + listOf(listOf(null, key("0", "0"), null))

@Composable
fun HomeScreen(onOpenProfile: () -> Unit, onOpenRemote: () -> Unit) {
    TvContent(fullRemote = false, onOpenProfile = onOpenProfile, onOpenRemote = onOpenRemote)
}

@Composable
fun TvRemoteScreen() {
    TvContent(fullRemote = true)
}

@Composable
private fun TvContent(fullRemote: Boolean, onOpenProfile: (() -> Unit)? = null, onOpenRemote: () -> Unit = {}) {
    val model: TvViewModel = appViewModel { TvViewModel(it.setup, it.homeHttpClient) }
    val state by model.state.collectAsStateWithLifecycle()
    LaunchedEffect(model) { model.refresh() }
    Column(Modifier.fillMaxSize()) {
        ScreenHeader(if (fullRemote) "TV remote" else "Home", onOpenProfile = onOpenProfile)
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (!fullRemote) SectionTitle("TV")
            Text("TV power: unobserved", style = MaterialTheme.typography.bodyMedium)
            Column(Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
                state.message?.let { Text(it) }
                state.problem?.let { ProblemText(it) }
                if (state.busy && state.message == null) Text("Loading TV controls…")
            }
            TextButton(onClick = model::refresh, enabled = !state.busy) { Text("Refresh TV controls") }
            Text("Unavailable commands stay disabled.", style = MaterialTheme.typography.bodySmall)
            TvRows(frequent, state, model::send)
            if (fullRemote) {
                TvRows(directions, state, model::send, directional = true)
                TvRows(more, state, model::send)
            } else {
                Button(onClick = onOpenRemote, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) { Text("Full TV remote") }
                for (section in listOf("Speaker", "Lights", "Backlight")) {
                    Column(Modifier.padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        SectionTitle(section)
                        Text("Controls aren't available in this version.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}

@Composable
private fun SectionTitle(title: String) {
    Text(title, style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
}

@Composable
private fun TvRows(rows: List<List<TvButton?>>, state: TvState, send: (String, String) -> Unit, directional: Boolean = false) {
    for (row in rows) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (button in row) {
                val modifier = Modifier.weight(1f).heightIn(min = if (directional) 64.dp else 56.dp)
                if (button == null) Spacer(modifier) else Button(
                    onClick = { send(button.command, button.label) },
                    enabled = !state.busy && button.command in state.commands,
                    modifier = modifier.semantics { contentDescription = "TV ${button.label}" },
                ) { Text(button.label) }
            }
        }
    }
}
