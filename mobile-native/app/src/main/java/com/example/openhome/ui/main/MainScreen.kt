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
import com.example.openhome.data.IrRepository
import com.example.openhome.data.IrState
import com.example.openhome.data.IrStatus
import com.example.openhome.data.SetupRepository
import com.example.openhome.theme.OpenhomeTheme

@Composable
fun MainScreen(
  setupRepository: SetupRepository,
  irRepository: IrRepository,
  modifier: Modifier = Modifier,
  viewModel: MainScreenViewModel = viewModel(factory = mainScreenViewModelFactory(setupRepository, irRepository)),
) {
  val state by viewModel.uiState.collectAsStateWithLifecycle()
  MainScreenContent(
    state = state,
    onBaseUrlChanged = viewModel::onBaseUrlChanged,
    onApiKeyChanged = viewModel::onApiKeyChanged,
    onSubmitSetup = viewModel::submitSetup,
    onTabSelected = viewModel::onTabSelected,
    onRetryIrStatus = viewModel::retryIrStatus,
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
  onRetryIrStatus: () -> Unit,
  modifier: Modifier = Modifier,
) {
  when (state) {
    MainScreenUiState.Loading -> LoadingScreen(modifier)
    is MainScreenUiState.Setup -> SetupScreen(state, onBaseUrlChanged, onApiKeyChanged, onSubmitSetup, modifier)
    is MainScreenUiState.App -> AppShell(state, onTabSelected, onRetryIrStatus, modifier)
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
private fun AppShell(
  state: MainScreenUiState.App,
  onTabSelected: (TopLevelTab) -> Unit,
  onRetryIrStatus: () -> Unit,
  modifier: Modifier = Modifier,
) {
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
      modifier = Modifier.fillMaxSize().padding(innerPadding).padding(24.dp).verticalScroll(rememberScrollState()),
      verticalArrangement = Arrangement.spacedBy(24.dp),
      horizontalAlignment = Alignment.CenterHorizontally,
    ) {
      when (state.selectedTab) {
        TopLevelTab.Home -> HomeTab(irState = state.irState, onRetryIrStatus = onRetryIrStatus)
        TopLevelTab.Remote -> RemoteTab(irState = state.irState, onRetryIrStatus = onRetryIrStatus)
      }
    }
  }
}

@Composable
private fun HomeTab(irState: IrState, onRetryIrStatus: () -> Unit, modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    TabHeader(title = TopLevelTab.Home.label, description = "OpenHome is configured and ready.")
    when (irState) {
      IrState.Idle, IrState.Loading -> IrLoadingState(message = "Loading shared IR status for Home and Remote.")
      is IrState.Error -> IrErrorState(message = irState.message, onRetryIrStatus = onRetryIrStatus)
      is IrState.Loaded -> IrLoadedState(status = irState.status)
    }
  }
}

@Composable
private fun RemoteTab(irState: IrState, onRetryIrStatus: () -> Unit, modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    TabHeader(
      title = TopLevelTab.Remote.label,
      description = "This tab reflects the shared IR state preload before command sending is added.",
    )

    when (irState) {
      IrState.Idle, IrState.Loading -> IrLoadingState(message = "Loading the IR remote state.")
      is IrState.Error -> IrErrorState(message = irState.message, onRetryIrStatus = onRetryIrStatus)
      is IrState.Loaded -> {
        IrLoadedState(status = irState.status)
        Text(
          text = "Command sending is added in the next slice, so the full remote stays visible while using the shared availability state.",
          style = MaterialTheme.typography.bodyMedium,
          textAlign = TextAlign.Center,
        )
      }
    }

    RemoteButtonLayout(modifier = Modifier.fillMaxWidth(), irState = irState)
  }
}

@Composable
private fun TabHeader(title: String, description: String, modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    Text(text = title, style = MaterialTheme.typography.headlineMedium, textAlign = TextAlign.Center)
    Text(text = description, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
  }
}

@Composable
private fun IrLoadingState(message: String, modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    CircularProgressIndicator()
    Text(text = message, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
  }
}

@Composable
private fun IrErrorState(message: String, onRetryIrStatus: () -> Unit, modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    Text(text = message, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
    OutlinedButton(onClick = onRetryIrStatus) {
      Text("Retry IR status")
    }
  }
}

@Composable
private fun IrLoadedState(status: IrStatus, modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    Text(text = status.message, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center)
    Text(text = availableCommandsText(status.availableCommands), style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.Center)
  }
}

@Composable
private fun RemoteButtonLayout(irState: IrState, modifier: Modifier = Modifier) {
  val isLoaded = irState is IrState.Loaded
  val availableCommands = (irState as? IrState.Loaded)?.status?.availableCommands.orEmpty()

  Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
    REMOTE_BUTTON_ROWS.forEach { buttonRow ->
      Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        buttonRow.forEach { button ->
          OutlinedButton(onClick = {}, enabled = false, modifier = Modifier.weight(1f)) {
            Text(text = remoteButtonLabel(button, availableCommands, isLoaded), textAlign = TextAlign.Center)
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

private fun mainScreenViewModelFactory(setupRepository: SetupRepository, irRepository: IrRepository) =
  viewModelFactory {
    initializer {
      MainScreenViewModel(setupRepository = setupRepository, irRepository = irRepository)
    }
  }

private fun availableCommandsText(availableCommands: Set<String>): String =
  if (availableCommands.isEmpty()) {
    "The Axum API did not report any available IR commands."
  } else {
    "Available commands: ${availableCommands.joinToString(", ")}."
  }

private fun remoteButtonLabel(button: RemoteButtonDefinition, availableCommands: Set<String>, isLoaded: Boolean): String =
  if (isLoaded && button.command !in availableCommands) {
    "${button.label}\nUnavailable"
  } else {
    button.label
  }

private val REMOTE_BUTTON_ROWS =
  listOf(
    listOf(
      RemoteButtonDefinition(command = "power", label = "Power"),
      RemoteButtonDefinition(command = "bluetooth", label = "Bluetooth"),
      RemoteButtonDefinition(command = "optical", label = "Optical"),
    ),
    listOf(
      RemoteButtonDefinition(command = "mute", label = "Mute"),
      RemoteButtonDefinition(command = "volume-down", label = "Volume -"),
      RemoteButtonDefinition(command = "volume-up", label = "Volume +"),
    ),
  )

private data class RemoteButtonDefinition(val command: String, val label: String)

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
      onRetryIrStatus = {},
    )
  }
}

@Preview(showBackground = true)
@Composable
fun AppShellPreview() {
  OpenhomeTheme {
    MainScreenContent(
      state = MainScreenUiState.App(irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("bluetooth", "optical")))),
      onBaseUrlChanged = {},
      onApiKeyChanged = {},
      onSubmitSetup = {},
      onTabSelected = {},
      onRetryIrStatus = {},
    )
  }
}
