package com.example.openhome.ui.main

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.openhome.data.IrRepository
import com.example.openhome.data.IrRemote
import com.example.openhome.data.IrState
import com.example.openhome.data.SetupRepository
import com.example.openhome.data.StoredConfiguration
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicLong

class MainScreenViewModel(
  private val setupRepository: SetupRepository,
  private val irRepository: IrRepository,
) : ViewModel() {
  private val localUiState = MutableStateFlow(LocalUiState())
  private val homeRemoteControlsState = MutableStateFlow(HomeRemoteControlsState())
  private val remoteControlsState = MutableStateFlow(RemoteControlsState())
  private val homeRemoteControlsGeneration = AtomicLong(0)
  private val remoteControlsGeneration = AtomicLong(0)
  private var activeConfiguration: StoredConfiguration? = null

  init {
    observeConfiguration()
  }

  private val baseUiState: StateFlow<MainScreenUiState> =
    combine(setupRepository.configuration, localUiState) { configuration, state ->
        if (configuration == null) {
          state.form.toUiState(ConfigurationFormMode.Setup)
        } else if (state.isReconfiguring) {
          state.form.toUiState(ConfigurationFormMode.Reconfigure)
        } else {
          MainScreenUiState.App(selectedTab = state.selectedTab)
        }
      }
      .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), MainScreenUiState.Loading)

  val uiState: StateFlow<MainScreenUiState> =
    combine(baseUiState, irRepository.state, homeRemoteControlsState, remoteControlsState) { state, irState, currentHomeRemoteControlsState, currentRemoteControlsState ->
        when (state) {
          MainScreenUiState.Loading -> MainScreenUiState.Loading
          is MainScreenUiState.ConfigurationForm -> state
          is MainScreenUiState.App ->
            state.copy(
              irState = irState,
              homeRemoteControlsState = currentHomeRemoteControlsState,
              remoteControlsState = currentRemoteControlsState,
            )
        }
      }
      .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), MainScreenUiState.Loading)

  fun onBaseUrlChanged(baseUrl: String) {
    updateSetupForm { copy(baseUrl = baseUrl) }
  }

  fun onApiKeyChanged(apiKey: String) {
    updateSetupForm { copy(apiKey = apiKey) }
  }

  fun submitSetup() {
    if (localUiState.value.form.isSaving) {
      return
    }

    viewModelScope.launch {
      try {
        val shouldSelectHome = activeConfiguration == null
        localUiState.update { currentState -> currentState.copy(form = currentState.form.copy(isSaving = true, errorMessage = null)) }
        val form = localUiState.value.form

        setupRepository
          .validateAndSave(baseUrl = form.baseUrl, apiKey = form.apiKey)
          .onSuccess {
            localUiState.update { currentState ->
              currentState.copy(
                isReconfiguring = false,
                selectedTab = if (shouldSelectHome) TopLevelTab.Home else currentState.selectedTab,
              )
            }
          }
          .onFailure(::showSetupError)
      } catch (exception: CancellationException) {
        throw exception
      } catch (throwable: Throwable) {
        showSetupError(throwable)
      } finally {
        localUiState.update { currentState -> currentState.copy(form = currentState.form.copy(isSaving = false)) }
      }
    }
  }

  fun onTabSelected(tab: TopLevelTab) {
    localUiState.update { currentState -> currentState.copy(selectedTab = tab) }
    if (tab == TopLevelTab.Remote && irRepository.state.value is IrState.Error) {
      refreshIrStatus()
    }
  }

  fun openReconfiguration() {
    enterReconfiguration(isReconfiguring = true)
  }

  fun cancelReconfiguration() {
    enterReconfiguration(isReconfiguring = false)
  }

  fun retryIrStatus() {
    refreshIrStatus()
  }

  fun sendHomeRemoteCommand(command: String) {
    if (command !in HOME_REMOTE_CONTROL_COMMANDS || activeConfiguration == null) {
      return
    }

    if (!canSendCommand(IrRemote.Edifier, command)) {
      return
    }

    sendCommand(
      remote = IrRemote.Edifier,
      command = command,
      controlsState = homeRemoteControlsState,
      controlsGeneration = homeRemoteControlsGeneration,
    )
  }

  fun sendRemoteCommand(command: String) {
    if (command !in REMOTE_BUTTON_COMMANDS || activeConfiguration == null) {
      return
    }

    if (!canSendCommand(IrRemote.LgTv, command)) {
      return
    }

    sendCommand(
      remote = IrRemote.LgTv,
      command = command,
      controlsState = remoteControlsState,
      controlsGeneration = remoteControlsGeneration,
    )
  }

  private fun updateSetupForm(transform: ConfigurationFormState.() -> ConfigurationFormState) {
    localUiState.update { currentState ->
      currentState.copy(form = currentState.form.transform().copy(errorMessage = null))
    }
  }

  private fun observeConfiguration() {
    viewModelScope.launch {
      setupRepository.configuration.collect { configuration ->
        val previousConfiguration = activeConfiguration
        if (configuration == previousConfiguration) {
          return@collect
        }

        activeConfiguration = configuration
        resetHomeRemoteControlsState()
        resetRemoteControlsState()

        if (configuration == null) {
          localUiState.value = LocalUiState()
        }

        if (previousConfiguration != null || configuration == null) {
          irRepository.reset()
        }

        if (configuration != null) {
          refreshIrStatus()
        }
      }
    }
  }

  private fun refreshIrStatus() {
    if (activeConfiguration == null || irRepository.state.value is IrState.Loading) {
      return
    }

    viewModelScope.launch {
      irRepository.refresh()
    }
  }

  private fun enterReconfiguration(isReconfiguring: Boolean) {
    val configuration = activeConfiguration ?: return
    if (localUiState.value.form.isSaving) {
      return
    }

    localUiState.update { currentState ->
      currentState.copy(
        form = ConfigurationFormState(baseUrl = configuration.baseUrl, apiKey = configuration.apiKey),
        isReconfiguring = isReconfiguring,
      )
    }
  }

  private fun sendCommand(
    remote: IrRemote,
    command: String,
    controlsState: MutableStateFlow<CommandControlsState>,
    controlsGeneration: AtomicLong,
  ) {
    val generation = controlsGeneration.get()
    controlsState.update { currentState ->
      currentState.copy(sendingCommands = currentState.sendingCommands + command, errorMessage = null, errorCommand = null)
    }

    viewModelScope.launch {
      val result = runSendCommand(remote, command)
      if (controlsGeneration.get() != generation) {
        return@launch
      }

      result
        .onSuccess {
          controlsState.update { currentState ->
            currentState.copy(sendingCommands = currentState.sendingCommands - command)
          }
        }
        .onFailure { throwable ->
          controlsState.update { currentState ->
            currentState.copy(
              sendingCommands = currentState.sendingCommands - command,
              errorMessage = throwable.message ?: DEFAULT_SEND_ERROR,
              errorCommand = command,
            )
          }
        }
    }
  }

  private suspend fun runSendCommand(remote: IrRemote, command: String): Result<Unit> =
    try {
      irRepository.sendCommand(remote, command)
    } catch (exception: CancellationException) {
      throw exception
    } catch (throwable: Throwable) {
      Result.failure(throwable)
    }

  private fun canSendCommand(remote: IrRemote, command: String): Boolean {
    val irStatus = (irRepository.state.value as? IrState.Loaded)?.status ?: return false
    val availableCommands =
      when (remote) {
        IrRemote.Edifier -> irStatus.edifierCommands
        IrRemote.LgTv -> irStatus.lgTvCommands
      }
    return command in availableCommands && !isCommandInFlight(remote, command)
  }

  private fun isCommandInFlight(remote: IrRemote, command: String): Boolean =
    command in
      when (remote) {
        IrRemote.Edifier -> homeRemoteControlsState.value.sendingCommands
        IrRemote.LgTv -> remoteControlsState.value.sendingCommands
      }

  private fun showSetupError(throwable: Throwable) {
    localUiState.update { currentState -> currentState.copy(form = currentState.form.copy(errorMessage = throwable.message ?: DEFAULT_VALIDATION_ERROR)) }
  }

  private fun resetHomeRemoteControlsState() {
    homeRemoteControlsGeneration.incrementAndGet()
    homeRemoteControlsState.value = HomeRemoteControlsState()
  }

  private fun resetRemoteControlsState() {
    remoteControlsGeneration.incrementAndGet()
    remoteControlsState.value = RemoteControlsState()
  }

  private companion object {
    const val STOP_TIMEOUT_MILLIS = 5_000L
    const val DEFAULT_VALIDATION_ERROR = "Couldn't validate that configuration."
    const val DEFAULT_SEND_ERROR = "Couldn't send that IR command."
  }
}

sealed interface MainScreenUiState {
  object Loading : MainScreenUiState

  data class ConfigurationForm(
    val mode: ConfigurationFormMode,
    val baseUrl: String = "",
    val apiKey: String = "",
    val isSaving: Boolean = false,
    val errorMessage: String? = null,
  ) : MainScreenUiState

  data class App(
    val selectedTab: TopLevelTab = TopLevelTab.Home,
    val irState: IrState = IrState.Idle,
    val homeRemoteControlsState: HomeRemoteControlsState = HomeRemoteControlsState(),
    val remoteControlsState: RemoteControlsState = RemoteControlsState(),
  ) : MainScreenUiState
}

data class CommandControlsState(
  val sendingCommands: Set<String> = emptySet(),
  val errorMessage: String? = null,
  val errorCommand: String? = null,
)

typealias HomeRemoteControlsState = CommandControlsState

typealias RemoteControlsState = CommandControlsState

enum class TopLevelTab {
  Home,
  Remote,
}

enum class ConfigurationFormMode {
  Setup,
  Reconfigure,
}

private data class LocalUiState(
  val form: ConfigurationFormState = ConfigurationFormState(),
  val isReconfiguring: Boolean = false,
  val selectedTab: TopLevelTab = TopLevelTab.Home,
)

private fun ConfigurationFormState.toUiState(mode: ConfigurationFormMode): MainScreenUiState.ConfigurationForm =
  MainScreenUiState.ConfigurationForm(
    mode = mode,
    baseUrl = baseUrl,
    apiKey = apiKey,
    isSaving = isSaving,
    errorMessage = errorMessage,
  )

private data class ConfigurationFormState(
  val baseUrl: String = "",
  val apiKey: String = "",
  val isSaving: Boolean = false,
  val errorMessage: String? = null,
)
