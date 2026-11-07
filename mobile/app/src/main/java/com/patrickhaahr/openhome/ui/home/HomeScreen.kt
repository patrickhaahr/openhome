package com.patrickhaahr.openhome.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.patrickhaahr.openhome.api.LightCommand
import com.patrickhaahr.openhome.ui.ProblemTag
import com.patrickhaahr.openhome.ui.ProfileKey
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.SlabShape
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.WithSpoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.CommandHalo
import com.patrickhaahr.openhome.ui.design.CommandPhase
import com.patrickhaahr.openhome.ui.design.CommandTracker
import com.patrickhaahr.openhome.ui.design.FigureKey
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.OpenHomeMark
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.theme.Lit

private val DECK_HEIGHT = 292.dp
private val SWITCH_WIDTH = 112.dp
private val SWITCH_SHAPE = RoundedCornerShape(36.dp)
private val TOP_HALF = RoundedCornerShape(topStart = 36.dp, topEnd = 36.dp)
private val BOTTOM_HALF = RoundedCornerShape(bottomStart = 36.dp, bottomEnd = 36.dp)
private val DPAD_SIZE = 288.dp

private data class TvButton(val command: String, val label: String, val glyph: Glyph? = null, val figure: String? = null)

private val POWER = TvButton("power", "Power", Glyph.Power)
private val INPUT = TvButton("input", "Input", Glyph.Input)
private val MUTE = TvButton("mute", "Mute", Glyph.Mute)
private val VOLUME_DOWN = TvButton("volume-down", "Volume −", Glyph.Minus)
private val VOLUME_UP = TvButton("volume-up", "Volume +", Glyph.Plus)
private val CHANNEL_DOWN = TvButton("channel-down", "Channel −", Glyph.Down)
private val CHANNEL_UP = TvButton("channel-up", "Channel +", Glyph.Up)
private val HDMI = (1..3).map { TvButton("hdmi-$it", "HDMI $it", figure = "H$it") }
private val DIGITS = (1..9).chunked(3).map { row -> row.map { TvButton("$it", "$it", figure = "$it") } }
private val ZERO = TvButton("0", "0", figure = "0")

@Composable
fun HomeScreen(onOpenProfile: () -> Unit, onOpenRemote: () -> Unit, onOpenBacklight: () -> Unit) {
    val tv: TvViewModel = appViewModel { TvViewModel(it.setup, it.homeHttpClient) }
    val tvState by tv.state.collectAsStateWithLifecycle()
    val household: SpeakerLightsViewModel = appViewModel { SpeakerLightsViewModel(it.setup, it.homeHttpClient) }
    val speaker by household.speaker.collectAsStateWithLifecycle()
    val lights by household.lights.collectAsStateWithLifecycle()
    LaunchedEffect(tv) { tv.refresh() }
    LaunchedEffect(household) { household.refreshSpeaker() }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Row(
            Modifier.fillMaxWidth().padding(start = ScreenMargin + 4.dp, end = ScreenMargin, top = 12.dp, bottom = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OpenHomeMark(Modifier.size(32.dp))
            Spacer(Modifier.weight(1f))
            ProfileKey(onOpenProfile)
        }
        Column(
            Modifier.padding(horizontal = ScreenMargin).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            WithSpoken(
                spoken = {
                    Spoken("Lights", isHeading = true)
                    TvNotes(tvState)
                    SectionNotes(lights, "Lights power: unobserved", "Availability is checked when you send a command.")
                },
            ) {
                Row(Modifier.fillMaxWidth().height(DECK_HEIGHT), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    WallSwitch(lights, household::sendLight, Modifier.width(SWITCH_WIDTH).fillMaxHeight())
                    TvDeck(tvState, tv, onOpenRemote, Modifier.weight(1f).fillMaxHeight())
                }
            }
            tvState.problem?.let { ProblemTag(it) }
            lights.problem?.let { ProblemTag(it) }
            SpeakerDeck(speaker, household)
            BacklightSection(onOpenAdvanced = onOpenBacklight)
        }
    }
}

/** A wall switch, because that is what it is. Both halves always look the same: a command is not a state. */
@Composable
private fun WallSwitch(state: HomeCommandState, send: (LightCommand) -> Unit, modifier: Modifier) {
    val tracker = remember { CommandTracker() }
    Column(modifier.background(Lit.Raised, SWITCH_SHAPE)) {
        for (command in LightCommand.entries) {
            val on = command == LightCommand.On
            val id = command.name
            val half = if (on) TOP_HALF else BOTTOM_HALF
            Box(
                Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .key(
                        "Lights ${command.label}",
                        {
                            tracker.press(id)
                            send(command)
                        },
                        enabled = !state.busy,
                        shape = half,
                        depth = 1f,
                    ).background(if (on) Lit.Lamp else Color.Transparent, half),
                contentAlignment = Alignment.Center,
            ) {
                CommandHalo(
                    phaseOf(tracker, id, state),
                    tracker.pulse,
                    Modifier.size(72.dp),
                ) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        GlyphIcon(
                            if (on) Glyph.BulbLit else Glyph.Bulb,
                            tint = if (on) Lit.OnLamp else Lit.Muted,
                            size = 48.dp,
                        )
                    }
                }
            }
        }
    }
}

private fun phaseOf(tracker: CommandTracker, id: String, state: HomeCommandState): CommandPhase =
    tracker.phase(id, state.busy, state.message?.contains("acknowledged") == true, state.problem != null)

private fun phaseOf(tracker: CommandTracker, id: String, state: TvState): CommandPhase =
    tracker.phase(id, state.busy, state.message?.contains("acknowledged") == true, state.problem != null)

@Composable
private fun TvKey(
    button: TvButton,
    state: TvState,
    tracker: CommandTracker,
    send: (String, String) -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 60.dp,
    tone: KeyTone = KeyTone.Raised,
) {
    val available = button.command in state.commands
    val press = {
        tracker.press(button.command)
        send(button.command, button.label)
    }
    val description = "TV ${button.label}"
    val phase = phaseOf(tracker, button.command, state)
    if (button.glyph != null) {
        GlyphKey(
            button.glyph, description, press, modifier, tone, size,
            enabled = !state.busy && available, dimmed = !available && !state.busy, phase = phase, pulse = tracker.pulse,
        )
    } else {
        FigureKey(
            button.figure.orEmpty(), description, press, modifier, tone, size,
            enabled = !state.busy && available, dimmed = !available && !state.busy, phase = phase, pulse = tracker.pulse,
        )
    }
}

@Composable
private fun TvDeck(state: TvState, model: TvViewModel, onOpenRemote: () -> Unit, modifier: Modifier) {
    val tracker = remember { CommandTracker() }
    Column(
        modifier.background(Lit.Slab, SlabShape).padding(14.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .padding(start = 6.dp)
                    .semantics {
                        contentDescription = "TV"
                        heading()
                    },
            ) { GlyphIcon(Glyph.Tv, tint = Lit.Lamp, size = 26.dp) }
            Spacer(Modifier.weight(1f))
            GlyphKey(Glyph.Refresh, "Refresh TV controls", model::refresh, tone = KeyTone.Quiet, size = 44.dp, enabled = !state.busy, dimmed = false)
            GlyphKey(Glyph.Remote, "Full TV remote", onOpenRemote, tone = KeyTone.Raised, size = 44.dp)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            TvKey(POWER, state, tracker, model::send)
            TvKey(INPUT, state, tracker, model::send)
            TvKey(MUTE, state, tracker, model::send)
        }
        Row(
            Modifier.fillMaxWidth().background(Lit.Raised, CircleShape),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            TvKey(VOLUME_DOWN, state, tracker, model::send, size = 72.dp, tone = KeyTone.Bare)
            TvKey(VOLUME_UP, state, tracker, model::send, size = 72.dp, tone = KeyTone.Bare)
        }
    }
}

/** Words only for trouble; what went well and what stays unknown are for assistive technology. */
@Composable
private fun TvNotes(state: TvState) {
    Spoken("TV power: unobserved")
    Spoken("Unavailable commands stay disabled.")
    state.message?.let { Spoken(it, live = true) }
    if (state.busy && state.message == null) Spoken("Loading TV controls…", live = true)
}

@Composable
private fun SectionNotes(state: HomeCommandState, unobserved: String, note: String) {
    Spoken(unobserved)
    Spoken(note)
    state.message?.let { Spoken(it, live = true) }
}

@Composable
private fun SpeakerDeck(state: SpeakerState, model: SpeakerLightsViewModel) {
    val tracker = remember { CommandTracker() }
    val feedback = state.feedback

    @Composable
    fun speakerKey(command: String, label: String, glyph: Glyph, tone: KeyTone = KeyTone.Raised) {
        val available = command in state.commands
        GlyphKey(
            glyph,
            "Speaker $label",
            {
                tracker.press(command)
                model.sendSpeaker(command, label)
            },
            tone = tone,
            size = 52.dp,
            enabled = !feedback.busy && available,
            dimmed = !available && !feedback.busy,
            phase = phaseOf(tracker, command, feedback),
            pulse = tracker.pulse,
        )
    }
    WithSpoken(
        spoken = {
            SectionNotes(feedback, "Speaker input and volume: unobserved", "Unavailable commands stay disabled.")
            if (feedback.busy && feedback.message == null) Spoken("Loading Speaker controls…", live = true)
        },
    ) {
    Slab(Modifier.fillMaxWidth(), padding = 14.dp, arrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .padding(start = 6.dp)
                    .semantics {
                        contentDescription = "Speaker"
                        heading()
                    },
            ) { GlyphIcon(Glyph.Speaker, tint = Lit.Lamp, size = 26.dp) }
            Spacer(Modifier.weight(1f))
            GlyphKey(Glyph.Refresh, "Refresh Speaker controls", model::refreshSpeaker, tone = KeyTone.Quiet, size = 44.dp, enabled = !feedback.busy, dimmed = false)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Row(Modifier.background(Lit.Raised, CircleShape)) {
                speakerKey("volume-down", "Volume −", Glyph.Minus, KeyTone.Bare)
                speakerKey("volume-up", "Volume +", Glyph.Plus, KeyTone.Bare)
            }
            speakerKey("mute", "Mute", Glyph.Mute)
            speakerKey("bluetooth", "Bluetooth", Glyph.Bluetooth)
            speakerKey("optical", "Optical", Glyph.Optical)
            speakerKey("power", "Power", Glyph.Power)
        }
    }
    }
    feedback.problem?.let { ProblemTag(it) }
}

/** The whole remote, as a remote: the wheel in the middle, the rockers either side, the digits below. */
@Composable
fun TvRemoteScreen() {
    val model: TvViewModel = appViewModel { TvViewModel(it.setup, it.homeHttpClient) }
    val state by model.state.collectAsStateWithLifecycle()
    val tracker = remember { CommandTracker() }
    LaunchedEffect(model) { model.refresh() }

    @Composable
    fun tvKey(button: TvButton, modifier: Modifier = Modifier, size: Dp = 64.dp, tone: KeyTone = KeyTone.Raised) {
        TvKey(button, state, tracker, model::send, modifier, size, tone)
    }
    Column(Modifier.fillMaxSize()) {
        ScreenHeader("TV remote", glyph = Glyph.Remote) {
            GlyphKey(Glyph.Refresh, "Refresh TV controls", model::refresh, tone = KeyTone.Quiet, size = 48.dp, enabled = !state.busy, dimmed = false)
        }
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = ScreenMargin).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(18.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Column { TvNotes(state) }
            state.problem?.let { ProblemTag(it) }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                tvKey(POWER, tone = KeyTone.Lamp)
                tvKey(INPUT)
                tvKey(MUTE)
                tvKey(TvButton("settings", "Settings", Glyph.Sliders))
            }
            Box(Modifier.size(DPAD_SIZE).background(Lit.Slab, CircleShape), contentAlignment = Alignment.Center) {
                tvKey(TvButton("up", "Up", Glyph.Up), Modifier.align(Alignment.TopCenter).padding(top = 8.dp), size = 72.dp, tone = KeyTone.Bare)
                tvKey(TvButton("down", "Down", Glyph.Down), Modifier.align(Alignment.BottomCenter).padding(bottom = 8.dp), size = 72.dp, tone = KeyTone.Bare)
                tvKey(TvButton("left", "Left", Glyph.Left), Modifier.align(Alignment.CenterStart).padding(start = 8.dp), size = 72.dp, tone = KeyTone.Bare)
                tvKey(TvButton("right", "Right", Glyph.Right), Modifier.align(Alignment.CenterEnd).padding(end = 8.dp), size = 72.dp, tone = KeyTone.Bare)
                tvKey(TvButton("ok", "OK", figure = "OK"), size = 104.dp, tone = KeyTone.Lamp)
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Rocker { tvKey(VOLUME_UP, tone = KeyTone.Bare); tvKey(VOLUME_DOWN, tone = KeyTone.Bare) }
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        tvKey(TvButton("back", "Back", Glyph.Back))
                        tvKey(TvButton("home", "TV Home", Glyph.House))
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        tvKey(TvButton("guide", "Guide", Glyph.Guide))
                        tvKey(TvButton("info", "Info", Glyph.Info))
                    }
                }
                Rocker { tvKey(CHANNEL_UP, tone = KeyTone.Bare); tvKey(CHANNEL_DOWN, tone = KeyTone.Bare) }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                HDMI.forEach { tvKey(it, size = 72.dp) }
                tvKey(TvButton("exit", "Exit", Glyph.Exit), size = 72.dp)
            }
            Slab(Modifier.fillMaxWidth(), arrangement = Arrangement.spacedBy(10.dp)) {
                DIGITS.forEach { row ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly) {
                        row.forEach { tvKey(it, size = 72.dp) }
                    }
                }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) { tvKey(ZERO, size = 72.dp) }
            }
        }
    }
}

/** Two keys in one capsule, + on top. */
@Composable
private fun Rocker(content: @Composable () -> Unit) {
    Column(
        Modifier.background(Lit.Raised, CircleShape),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) { content() }
}
