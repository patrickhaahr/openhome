package com.patrickhaahr.openhome.ui.setup

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.setup.SetupProgress
import com.patrickhaahr.openhome.ui.FieldShape
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.OpenHomeMark
import com.patrickhaahr.openhome.ui.design.SlabKey
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

@Composable
fun SetupRoute(interruptedBaseUrl: String?) {
    val viewModel = appViewModel { SetupViewModel(it.setup, initialBaseUrl = interruptedBaseUrl) }
    SetupScreen(viewModel, showInterruptedNotice = interruptedBaseUrl != null)
}

/** The mark, an address, a key and an arrow. Everything said here is said because something is wrong. */
@Composable
private fun SetupScreen(viewModel: SetupViewModel, showInterruptedNotice: Boolean) {
    val submission = viewModel.submission
    val progress = (submission as? SetupSubmission.Working)?.progress
    val error = (submission as? SetupSubmission.Failed)?.message
    val isWorking = submission is SetupSubmission.Working
    var apiKeyVisible by remember { mutableStateOf(false) }

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .safeDrawingPadding()
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Spacer(Modifier.height(32.dp))
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { OpenHomeMark(Modifier.size(168.dp)) }
        Spacer(Modifier.height(48.dp))
        Spoken("Connect to OpenHome", isHeading = true)
        Spoken("Your Exercises, Workouts, Profile and Body Weight download once, then stay on this device.")

        if (showInterruptedNotice && error == null && !isWorking) {
            Notice(
                text = "The last download didn't finish, so nothing was saved. Enter your API Key to try again.",
                isError = false,
            )
        }

        FormField(
            value = viewModel.baseUrl,
            onValueChange = { viewModel.baseUrl = it },
            label = "Base URL",
            glyph = Glyph.Globe,
            hint = "http://openhome.local:8000",
            enabled = !isWorking,
            capitalization = KeyboardCapitalization.None,
            autoCorrect = false,
            keyboardType = KeyboardType.Uri,
        )
        FormField(
            value = viewModel.apiKey,
            onValueChange = { viewModel.apiKey = it },
            label = "API Key",
            glyph = Glyph.Key,
            enabled = !isWorking,
            capitalization = KeyboardCapitalization.None,
            autoCorrect = false,
            keyboardType = KeyboardType.Password,
            imeAction = ImeAction.Done,
            onDone = viewModel::submit,
            visualTransformation = if (apiKeyVisible) VisualTransformation.None else PasswordVisualTransformation(),
            trailing = {
                GlyphKey(
                    if (apiKeyVisible) Glyph.EyeOff else Glyph.Eye,
                    if (apiKeyVisible) "Hide" else "Show",
                    { apiKeyVisible = !apiKeyVisible },
                    tone = KeyTone.Quiet,
                    size = 48.dp,
                )
            },
        )

        if (error != null) Notice(text = error, isError = true)
        if (progress != null) ProgressLine(progress)

        SlabKey(
            if (error == null) Glyph.Arrow else Glyph.Refresh,
            if (error == null) "Connect and download" else "Try again",
            viewModel::submit,
            modifier = Modifier.fillMaxWidth(),
            enabled = !isWorking,
        )
    }
}

@Composable
private fun Notice(text: String, isError: Boolean) {
    Text(
        text = text,
        style = Type.Body,
        color = if (isError) Lit.Alarm else Lit.Ink,
        modifier =
            Modifier
                .fillMaxWidth()
                .background(if (isError) Lit.AlarmDeep else Lit.Slab, FieldShape)
                .padding(16.dp)
                .semantics { liveRegion = LiveRegionMode.Polite },
    )
}

@Composable
private fun ProgressLine(progress: SetupProgress) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(vertical = 8.dp)) {
        val fraction =
            (progress as? SetupProgress.DownloadingWorkouts)
                ?.takeIf { it.total > 0 }
                ?.let { it.done.toFloat() / it.total }
        if (fraction != null) {
            LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth().height(8.dp), color = Lit.Lamp, trackColor = Lit.Raised)
        } else {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth().height(8.dp), color = Lit.Lamp, trackColor = Lit.Raised)
        }
        Spoken(
            when (progress) {
                SetupProgress.CheckingConnection -> "Checking your Base URL and API Key"
                SetupProgress.ListingWorkouts -> "Finding your Workouts"
                is SetupProgress.DownloadingWorkouts -> "Downloading Workouts: ${progress.done} of ${progress.total}"
                SetupProgress.DownloadingLibraryAndProfile -> "Downloading Library, Profile and Body Weight"
                SetupProgress.Saving -> "Saving on this device"
            },
            live = true,
        )
        if (progress is SetupProgress.DownloadingWorkouts) {
            Text("${progress.done}/${progress.total}", style = Type.Figure, color = Lit.Ink)
        }
    }
}
