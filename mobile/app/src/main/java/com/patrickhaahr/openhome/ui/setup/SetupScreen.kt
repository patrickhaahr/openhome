package com.patrickhaahr.openhome.ui.setup

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.setup.SetupProgress
import com.patrickhaahr.openhome.ui.appViewModel

@Composable
fun SetupRoute(interruptedBaseUrl: String?) {
    val viewModel = appViewModel { SetupViewModel(it.setup, initialBaseUrl = interruptedBaseUrl) }
    SetupScreen(viewModel, showInterruptedNotice = interruptedBaseUrl != null)
}

@Composable
private fun SetupScreen(viewModel: SetupViewModel, showInterruptedNotice: Boolean) {
    val progress = viewModel.progress
    val error = viewModel.error
    val isWorking = progress != null
    var apiKeyVisible by remember { mutableStateOf(false) }

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .safeDrawingPadding()
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 32.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            text = "Connect to OpenHome",
            style = MaterialTheme.typography.displaySmall,
            modifier = Modifier.semantics { heading() },
        )
        Text(
            text = "Your Exercises, Workouts, Profile and Body Weight download once, then stay on this device.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        if (showInterruptedNotice && error == null && !isWorking) {
            Notice(
                text = "The last download didn't finish, so nothing was saved. Enter your API Key to try again.",
                isError = false,
            )
        }

        OutlinedTextField(
            value = viewModel.baseUrl,
            onValueChange = { viewModel.baseUrl = it },
            label = { Text("Base URL") },
            placeholder = { Text("http://openhome.local:8000") },
            singleLine = true,
            enabled = !isWorking,
            keyboardOptions =
                KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    autoCorrectEnabled = false,
                    keyboardType = KeyboardType.Uri,
                    imeAction = ImeAction.Next,
                ),
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = viewModel.apiKey,
            onValueChange = { viewModel.apiKey = it },
            label = { Text("API Key") },
            singleLine = true,
            enabled = !isWorking,
            visualTransformation =
                if (apiKeyVisible) VisualTransformation.None else PasswordVisualTransformation(),
            trailingIcon = {
                TextButton(onClick = { apiKeyVisible = !apiKeyVisible }) {
                    Text(if (apiKeyVisible) "Hide" else "Show")
                }
            },
            keyboardOptions =
                KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    autoCorrectEnabled = false,
                    keyboardType = KeyboardType.Password,
                    imeAction = ImeAction.Done,
                ),
            keyboardActions = KeyboardActions(onDone = { viewModel.submit() }),
            modifier = Modifier.fillMaxWidth(),
        )

        if (error != null) Notice(text = error, isError = true)
        if (progress != null) ProgressLine(progress)

        Button(
            onClick = viewModel::submit,
            enabled = !isWorking,
            modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
        ) {
            Text(if (error == null) "Connect and download" else "Try again")
        }
    }
}

@Composable
private fun Notice(text: String, isError: Boolean) {
    Surface(
        color =
            if (isError) {
                MaterialTheme.colorScheme.errorContainer
            } else {
                MaterialTheme.colorScheme.surfaceContainerHigh
            },
        contentColor =
            if (isError) {
                MaterialTheme.colorScheme.onErrorContainer
            } else {
                MaterialTheme.colorScheme.onSurface
            },
        shape = MaterialTheme.shapes.medium,
        modifier = Modifier.fillMaxWidth().semantics { liveRegion = LiveRegionMode.Polite },
    ) {
        Text(text = text, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(16.dp))
    }
}

@Composable
private fun ProgressLine(progress: SetupProgress) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        val fraction =
            (progress as? SetupProgress.DownloadingWorkouts)
                ?.takeIf { it.total > 0 }
                ?.let { it.done.toFloat() / it.total }
        if (fraction != null) {
            LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth())
        } else {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }
        Text(
            text =
                when (progress) {
                    SetupProgress.CheckingConnection -> "Checking your Base URL and API Key"
                    SetupProgress.ListingWorkouts -> "Finding your Workouts"
                    is SetupProgress.DownloadingWorkouts -> "Downloading Workouts: ${progress.done} of ${progress.total}"
                    SetupProgress.DownloadingLibraryAndProfile -> "Downloading Library, Profile and Body Weight"
                    SetupProgress.Saving -> "Saving on this device"
                },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
