package com.example.openhome.ui.main

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.viewModelFactory
import com.example.openhome.data.SetupRepository
import com.example.openhome.theme.OpenhomeTheme

@Composable
fun MainScreen(
  setupRepository: SetupRepository,
  modifier: Modifier = Modifier,
  viewModel: MainScreenViewModel = viewModel(factory = mainScreenViewModelFactory(setupRepository)),
) {
  val state by viewModel.uiState.collectAsStateWithLifecycle()
  MainScreenContent(
    state = state,
    onBaseUrlChanged = viewModel::onBaseUrlChanged,
    onApiKeyChanged = viewModel::onApiKeyChanged,
    onSubmitSetup = viewModel::submitSetup,
    onTabSelected = viewModel::onTabSelected,
    modifier = modifier,
  )
}

@Composable
internal fun MainScreenContent(
  state: MainScreenUiState,
  onBaseUrlChanged: (String) -> Unit,
  onApiKeyChanged: (String) -> Unit,
  onSubmitSetup: () -> Unit,
  onTabSelected: (TopLevelTab) -> Unit,
  modifier: Modifier = Modifier,
) {
  when (state) {
    MainScreenUiState.Loading -> LoadingScreen(modifier)
    is MainScreenUiState.Setup -> SetupScreen(state, onBaseUrlChanged, onApiKeyChanged, onSubmitSetup, modifier)
    is MainScreenUiState.App -> AppShell(state, onTabSelected, modifier)
  }
}

@Composable
private fun LoadingScreen(modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
    CircularProgressIndicator()
  }
}

@Composable
private fun SetupScreen(
  state: MainScreenUiState.Setup,
  onBaseUrlChanged: (String) -> Unit,
  onApiKeyChanged: (String) -> Unit,
  onSubmitSetup: () -> Unit,
  modifier: Modifier = Modifier,
) {
  val isEditingEnabled = !state.isSaving

  Column(
    modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()),
    verticalArrangement = Arrangement.Center,
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
    Column(modifier = Modifier.fillMaxWidth().widthIn(max = 480.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
      Text(text = "Set up OpenHome", style = MaterialTheme.typography.headlineMedium)
      Text(
        text = "Enter the Axum API Base URL and API Key. The app validates them with /api/health before saving.",
        style = MaterialTheme.typography.bodyMedium,
      )
      OutlinedTextField(
        value = state.baseUrl,
        onValueChange = onBaseUrlChanged,
        modifier = Modifier.fillMaxWidth(),
        label = { Text("Base URL") },
        singleLine = true,
        enabled = isEditingEnabled,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
      )
      OutlinedTextField(
        value = state.apiKey,
        onValueChange = onApiKeyChanged,
        modifier = Modifier.fillMaxWidth(),
        label = { Text("API Key") },
        singleLine = true,
        enabled = isEditingEnabled,
        visualTransformation = PasswordVisualTransformation(),
      )
      if (state.errorMessage != null) {
        Text(text = state.errorMessage, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
      }
      Button(onClick = onSubmitSetup, enabled = isEditingEnabled, modifier = Modifier.fillMaxWidth()) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
          if (state.isSaving) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
          }
          Text(if (state.isSaving) "Validating..." else "Validate and continue")
        }
      }
    }
  }
}

@Composable
private fun AppShell(state: MainScreenUiState.App, onTabSelected: (TopLevelTab) -> Unit, modifier: Modifier = Modifier) {
  Scaffold(
    modifier = modifier.fillMaxSize(),
    bottomBar = {
      NavigationBar {
        TopLevelTab.entries.forEach { tab ->
          NavigationBarItem(selected = state.selectedTab == tab, onClick = { onTabSelected(tab) }, icon = {}, label = { Text(tab.label) })
        }
      }
    },
  ) { innerPadding ->
    Column(
      modifier = Modifier.fillMaxSize().padding(innerPadding).padding(24.dp),
      verticalArrangement = Arrangement.Center,
      horizontalAlignment = Alignment.CenterHorizontally,
    ) {
      Text(text = state.selectedTab.label, style = MaterialTheme.typography.headlineMedium, textAlign = TextAlign.Center)
      Text(text = state.selectedTab.description, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
      if (state.selectedTab == TopLevelTab.Remote) {
        RemoteButtonLayout(modifier = Modifier.fillMaxWidth().padding(top = 24.dp))
      }
    }
  }
}

@Composable
private fun RemoteButtonLayout(modifier: Modifier = Modifier) {
  Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
    REMOTE_BUTTON_ROWS.forEach { buttonRow ->
      Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        buttonRow.forEach { label ->
          OutlinedButton(onClick = {}, enabled = false, modifier = Modifier.weight(1f)) {
            Text(text = label, textAlign = TextAlign.Center)
          }
        }
      }
    }
  }
}

private val TopLevelTab.label: String
  get() =
    when (this) {
      TopLevelTab.Home -> "Home"
      TopLevelTab.Remote -> "Remote"
    }

private val TopLevelTab.description: String
  get() =
    when (this) {
      TopLevelTab.Home -> "OpenHome is configured and ready."
      TopLevelTab.Remote -> "Infrared commands stay visible and disabled until the app loads remote state."
    }

private fun mainScreenViewModelFactory(setupRepository: SetupRepository) =
  viewModelFactory {
    initializer {
      MainScreenViewModel(setupRepository = setupRepository)
    }
  }

private val REMOTE_BUTTON_ROWS =
  listOf(
    listOf("Power", "Bluetooth", "Optical"),
    listOf("Mute", "Volume -", "Volume +"),
  )

@Preview(showBackground = true)
@Composable
fun SetupScreenPreview() {
  OpenhomeTheme {
    MainScreenContent(
      state = MainScreenUiState.Setup(baseUrl = "http://192.168.1.20:8000", apiKey = "secret"),
      onBaseUrlChanged = {},
      onApiKeyChanged = {},
      onSubmitSetup = {},
      onTabSelected = {},
    )
  }
}

@Preview(showBackground = true)
@Composable
fun AppShellPreview() {
  OpenhomeTheme {
    MainScreenContent(state = MainScreenUiState.App(), onBaseUrlChanged = {}, onApiKeyChanged = {}, onSubmitSetup = {}, onTabSelected = {})
  }
}
