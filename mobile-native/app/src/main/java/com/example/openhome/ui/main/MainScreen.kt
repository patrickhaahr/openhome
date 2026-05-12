package com.example.openhome.ui.main

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
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
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

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
    onOpenReconfiguration = viewModel::openReconfiguration,
    onCancelReconfiguration = viewModel::cancelReconfiguration,
    onTabSelected = viewModel::onTabSelected,
    onRetryIrStatus = viewModel::retryIrStatus,
    onSendHomeRemoteCommand = viewModel::sendHomeRemoteCommand,
    onSendRemoteCommand = viewModel::sendRemoteCommand,
    modifier = modifier,
  )
}

@Composable
internal fun MainScreenContent(
  state: MainScreenUiState,
  onBaseUrlChanged: (String) -> Unit,
  onApiKeyChanged: (String) -> Unit,
  onSubmitSetup: () -> Unit,
  onOpenReconfiguration: () -> Unit,
  onCancelReconfiguration: () -> Unit,
  onTabSelected: (TopLevelTab) -> Unit,
  onRetryIrStatus: () -> Unit,
  onSendHomeRemoteCommand: (String) -> Unit,
  onSendRemoteCommand: (String) -> Unit,
  modifier: Modifier = Modifier,
) {
  when (state) {
    MainScreenUiState.Loading -> LoadingScreen(modifier)
    is MainScreenUiState.ConfigurationForm ->
      ConfigurationFormScreen(state, onBaseUrlChanged, onApiKeyChanged, onSubmitSetup, onCancelReconfiguration, modifier)
    is MainScreenUiState.App -> AppShell(state, onOpenReconfiguration, onTabSelected, onRetryIrStatus, onSendHomeRemoteCommand, onSendRemoteCommand, modifier)
  }
}

@Composable
private fun LoadingScreen(modifier: Modifier = Modifier) {
  Column(modifier = modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
    CircularProgressIndicator()
  }
}

@Composable
private fun ConfigurationFormScreen(
  state: MainScreenUiState.ConfigurationForm,
  onBaseUrlChanged: (String) -> Unit,
  onApiKeyChanged: (String) -> Unit,
  onSubmitSetup: () -> Unit,
  onCancelReconfiguration: () -> Unit,
  modifier: Modifier = Modifier,
) {
  val screenConfig = state.mode.screenConfig()

  SetupForm(
    title = screenConfig.title,
    description = screenConfig.description,
    baseUrl = state.baseUrl,
    apiKey = state.apiKey,
    isSaving = state.isSaving,
    errorMessage = state.errorMessage,
    submitLabel = screenConfig.submitLabel,
    onBaseUrlChanged = onBaseUrlChanged,
    onApiKeyChanged = onApiKeyChanged,
    onSubmit = onSubmitSetup,
    secondaryActionLabel = screenConfig.secondaryActionLabel,
    onSecondaryAction = screenConfig.secondaryAction(onCancelReconfiguration),
    modifier = modifier,
  )
}

@Composable
private fun SetupForm(
  title: String,
  description: String,
  baseUrl: String,
  apiKey: String,
  isSaving: Boolean,
  errorMessage: String?,
  submitLabel: String,
  onBaseUrlChanged: (String) -> Unit,
  onApiKeyChanged: (String) -> Unit,
  onSubmit: () -> Unit,
  modifier: Modifier = Modifier,
  secondaryActionLabel: String? = null,
  onSecondaryAction: (() -> Unit)? = null,
) {
  val isEditingEnabled = !isSaving

  Column(
    modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()),
    verticalArrangement = Arrangement.Center,
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
    Column(modifier = Modifier.fillMaxWidth().widthIn(max = 480.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
      Text(text = title, style = MaterialTheme.typography.headlineMedium)
      Text(text = description, style = MaterialTheme.typography.bodyMedium)
      OutlinedTextField(
        value = baseUrl,
        onValueChange = onBaseUrlChanged,
        modifier = Modifier.fillMaxWidth(),
        label = { Text("Base URL") },
        singleLine = true,
        enabled = isEditingEnabled,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
      )
      OutlinedTextField(
        value = apiKey,
        onValueChange = onApiKeyChanged,
        modifier = Modifier.fillMaxWidth(),
        label = { Text("API Key") },
        singleLine = true,
        enabled = isEditingEnabled,
        visualTransformation = PasswordVisualTransformation(),
      )
      if (errorMessage != null) {
        Text(text = errorMessage, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
      }
      if (secondaryActionLabel != null && onSecondaryAction != null) {
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
          OutlinedButton(onClick = onSecondaryAction, enabled = isEditingEnabled, modifier = Modifier.weight(1f).testTag("configuration-cancel")) {
            Text(secondaryActionLabel)
          }
          SubmitSetupButton(
            label = submitLabel,
            isSaving = isSaving,
            enabled = isEditingEnabled,
            onSubmit = onSubmit,
            modifier = Modifier.weight(1f),
          )
        }
      } else {
        SubmitSetupButton(label = submitLabel, isSaving = isSaving, enabled = isEditingEnabled, onSubmit = onSubmit, modifier = Modifier.fillMaxWidth())
      }
    }
  }
}

@Composable
private fun SubmitSetupButton(label: String, isSaving: Boolean, enabled: Boolean, onSubmit: () -> Unit, modifier: Modifier = Modifier) {
  Button(onClick = onSubmit, enabled = enabled, modifier = modifier) {
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
      if (isSaving) {
        CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
      }
      Text(if (isSaving) "Validating..." else label)
    }
  }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AppShell(
  state: MainScreenUiState.App,
  onOpenReconfiguration: () -> Unit,
  onTabSelected: (TopLevelTab) -> Unit,
  onRetryIrStatus: () -> Unit,
  onSendHomeRemoteCommand: (String) -> Unit,
  onSendRemoteCommand: (String) -> Unit,
  modifier: Modifier = Modifier,
) {
  val tabs = TopLevelTab.entries
  val selectedPage = state.selectedTab.ordinal
  val pagerState = rememberPagerState(initialPage = selectedPage) { tabs.size }
  val coroutineScope = rememberCoroutineScope()
  val currentSelectedTab by rememberUpdatedState(state.selectedTab)
  val currentOnTabSelected by rememberUpdatedState(onTabSelected)

  LaunchedEffect(selectedPage) {
    if (pagerState.currentPage != selectedPage && pagerState.targetPage != selectedPage) {
      pagerState.animateScrollToPage(selectedPage)
    }
  }

  LaunchedEffect(pagerState) {
    snapshotFlow { pagerState.settledPage }.distinctUntilChanged().collect { settledPage ->
      val settledTab = tabs[settledPage]
      if (settledTab != currentSelectedTab) {
        currentOnTabSelected(settledTab)
      }
    }
  }

  Scaffold(
    modifier = modifier.fillMaxSize(),
    topBar = {
      TopAppBar(
        title = { Text("OpenHome") },
        actions = {
          TextButton(onClick = onOpenReconfiguration, modifier = Modifier.testTag("open-reconfiguration")) {
            Text("Reconfigure")
          }
        },
      )
    },
    bottomBar = {
      NavigationBar {
        tabs.forEach { tab ->
          NavigationBarItem(
            selected = pagerState.currentPage == tab.ordinal,
            onClick = {
              coroutineScope.launch {
                pagerState.animateScrollToPage(tab.ordinal)
              }
            },
            icon = {},
            label = { Text(tab.label) },
            modifier = Modifier.testTag("top-level-tab-${tab.name.lowercase()}"),
          )
        }
      }
    },
  ) { innerPadding ->
    HorizontalPager(
      state = pagerState,
      modifier = Modifier.fillMaxSize().padding(innerPadding).testTag("main-tabs-pager"),
    ) { page ->
      val tab = tabs[page]
      Column(
        modifier =
          Modifier
            .fillMaxSize()
            .padding(24.dp)
            .verticalScroll(rememberScrollState())
            .testTag(if (tab == TopLevelTab.Home) "home-tab-page" else "remote-tab-page"),
        verticalArrangement = Arrangement.spacedBy(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
      ) {
        when (tab) {
          TopLevelTab.Home -> {
            HomeTab(
              irState = state.irState,
              homeRemoteControlsState = state.homeRemoteControlsState,
              onRetryIrStatus = onRetryIrStatus,
              onSendHomeRemoteCommand = onSendHomeRemoteCommand,
            )
          }
          TopLevelTab.Remote -> {
            RemoteTab(
              irState = state.irState,
              remoteControlsState = state.remoteControlsState,
              onRetryIrStatus = onRetryIrStatus,
              onSendRemoteCommand = onSendRemoteCommand,
            )
          }
        }
      }
    }
  }
}

@Composable
private fun HomeTab(
  irState: IrState,
  homeRemoteControlsState: HomeRemoteControlsState,
  onRetryIrStatus: () -> Unit,
  onSendHomeRemoteCommand: (String) -> Unit,
  modifier: Modifier = Modifier,
) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    TabHeader(title = TopLevelTab.Home.label, description = "OpenHome is configured and ready.")
    when (irState) {
      IrState.Idle, IrState.Loading -> IrLoadingState(message = "Loading shared IR status for Home and Remote.")
      is IrState.Error -> IrErrorState(message = irState.message, onRetryIrStatus = onRetryIrStatus)
      is IrState.Loaded -> {
        IrLoadedState(status = irState.status)
        HomeRemoteControls(
          status = irState.status,
          controlsState = homeRemoteControlsState,
          onSendHomeRemoteCommand = onSendHomeRemoteCommand,
        )
      }
    }
  }
}

@Composable
private fun RemoteTab(
  irState: IrState,
  remoteControlsState: RemoteControlsState,
  onRetryIrStatus: () -> Unit,
  onSendRemoteCommand: (String) -> Unit,
  modifier: Modifier = Modifier,
) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    TabHeader(title = TopLevelTab.Remote.label, description = "Use the full v1 IR Remote from the shared IR status.")

    when (irState) {
      IrState.Idle, IrState.Loading -> IrLoadingState(message = "Loading the IR remote state.")
      is IrState.Error -> IrErrorState(message = irState.message, onRetryIrStatus = onRetryIrStatus)
      is IrState.Loaded -> IrLoadedState(status = irState.status)
    }

    RemoteButtonLayout(
      modifier = Modifier.fillMaxWidth(),
      irState = irState,
      controlsState = remoteControlsState,
      onSendRemoteCommand = onSendRemoteCommand,
    )

    if (remoteControlsState.errorMessage != null) {
      Text(
        text = remoteErrorText(remoteControlsState, REMOTE_BUTTONS),
        color = MaterialTheme.colorScheme.error,
        style = MaterialTheme.typography.bodyMedium,
        textAlign = TextAlign.Center,
      )
    }
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
private fun HomeRemoteControls(
  status: IrStatus,
  controlsState: HomeRemoteControlsState,
  onSendHomeRemoteCommand: (String) -> Unit,
  modifier: Modifier = Modifier,
) {
  Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    Text(text = "Quick controls", style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
      HOME_REMOTE_CONTROLS.forEach { control ->
        val isSending = control.command in controlsState.sendingCommands
        val isAvailable = control.command in status.availableCommands
        OutlinedButton(
          onClick = { onSendHomeRemoteCommand(control.command) },
          enabled = isAvailable && !isSending,
          modifier = Modifier.weight(1f).testTag("home-remote-${control.command}"),
        ) {
          Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (isSending) {
              CircularProgressIndicator(modifier = Modifier.size(18.dp).testTag("home-remote-${control.command}-progress"), strokeWidth = 2.dp)
            }
            Text(
              text = remoteButtonLabel(control.command, control.label, status.availableCommands, true),
              textAlign = TextAlign.Center,
              modifier = Modifier.testTag("home-remote-${control.command}-label"),
            )
          }
        }
      }
    }

    if (controlsState.errorMessage != null) {
      Text(
        text = homeRemoteErrorText(controlsState, HOME_REMOTE_CONTROLS),
        color = MaterialTheme.colorScheme.error,
        style = MaterialTheme.typography.bodyMedium,
        textAlign = TextAlign.Center,
      )
    }
  }
}

@Composable
private fun RemoteButtonLayout(
  irState: IrState,
  controlsState: RemoteControlsState,
  onSendRemoteCommand: (String) -> Unit,
  modifier: Modifier = Modifier,
) {
  val loadedState = irState as? IrState.Loaded
  val isLoaded = loadedState != null
  val availableCommands = loadedState?.status?.availableCommands.orEmpty()

  Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
    REMOTE_BUTTON_ROWS.forEach { buttonRow ->
      Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        buttonRow.forEach { button ->
          val isSending = button.command in controlsState.sendingCommands
          val isAvailable = button.command in availableCommands
          Box(modifier = Modifier.weight(1f).testTag("remote-button")) {
            OutlinedButton(
              onClick = { onSendRemoteCommand(button.command) },
              enabled = isLoaded && isAvailable && !isSending,
              modifier = Modifier.fillMaxWidth().testTag("remote-${button.command}"),
            ) {
              Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (isSending) {
                  CircularProgressIndicator(modifier = Modifier.size(18.dp).testTag("remote-${button.command}-progress"), strokeWidth = 2.dp)
                }
                Text(
                  text = remoteButtonLabel(button.command, button.label, availableCommands, isLoaded),
                  textAlign = TextAlign.Center,
                  modifier = Modifier.testTag("remote-${button.command}-label"),
                )
              }
            }
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

private fun remoteButtonLabel(command: String, label: String, availableCommands: Set<String>, isLoaded: Boolean): String =
  if (isLoaded && command !in availableCommands) {
    "$label\nUnavailable"
  } else {
    label
  }

private fun homeRemoteErrorText(state: HomeRemoteControlsState, controls: List<HomeRemoteControlDefinition>): String {
  val errorMessage = state.errorMessage.orEmpty()
  val errorLabel = controls.firstOrNull { it.command == state.errorCommand }?.label ?: return errorMessage
  return "$errorLabel failed: $errorMessage"
}

private fun remoteErrorText(state: RemoteControlsState, controls: List<RemoteButtonDefinition>): String {
  val errorMessage = state.errorMessage.orEmpty()
  val errorLabel = controls.firstOrNull { it.command == state.errorCommand }?.label ?: return errorMessage
  return "$errorLabel failed: $errorMessage"
}

private fun ConfigurationFormMode.screenConfig(): ConfigurationScreenConfig =
  when (this) {
    ConfigurationFormMode.Setup ->
      ConfigurationScreenConfig(
        title = "Set up OpenHome",
        description = "Enter the Axum API Base URL and API Key. The app validates them with /api/health before saving.",
        submitLabel = "Validate and continue",
      )
    ConfigurationFormMode.Reconfigure ->
      ConfigurationScreenConfig(
        title = "Update configuration",
        description = "Replace the stored Axum API Base URL and API Key. The current configuration stays active until /api/health succeeds.",
        submitLabel = "Save configuration",
        secondaryActionLabel = "Cancel",
      )
  }

private data class ConfigurationScreenConfig(
  val title: String,
  val description: String,
  val submitLabel: String,
  val secondaryActionLabel: String? = null,
) {
  fun secondaryAction(onCancelReconfiguration: () -> Unit): (() -> Unit)? =
    if (secondaryActionLabel == null) {
      null
    } else {
      onCancelReconfiguration
    }
}

@Preview(showBackground = true)
@Composable
fun SetupScreenPreview() {
  OpenhomeTheme {
    MainScreenContent(
      state = MainScreenUiState.ConfigurationForm(mode = ConfigurationFormMode.Setup, baseUrl = "http://192.168.1.20:8000", apiKey = "secret"),
      onBaseUrlChanged = {},
      onApiKeyChanged = {},
      onSubmitSetup = {},
      onOpenReconfiguration = {},
      onCancelReconfiguration = {},
      onTabSelected = {},
      onRetryIrStatus = {},
      onSendHomeRemoteCommand = {},
      onSendRemoteCommand = {},
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
      onOpenReconfiguration = {},
      onCancelReconfiguration = {},
      onTabSelected = {},
      onRetryIrStatus = {},
      onSendHomeRemoteCommand = {},
      onSendRemoteCommand = {},
    )
  }
}
