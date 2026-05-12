package com.example.openhome.ui.main

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.openhome.data.IrRepository
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
  private val setupForm = MutableStateFlow(SetupForm())
  private val isSaving = MutableStateFlow(false)
  private val setupErrorMessage = MutableStateFlow<String?>(null)
  private val selectedTab = MutableStateFlow(TopLevelTab.Home)
  private val homeRemoteControlsState = MutableStateFlow(HomeRemoteControlsState())
  private val homeRemoteControlsGeneration = AtomicLong(0)
  private var activeConfiguration: StoredConfiguration? = null

  init {
    observeConfiguration()
  }

  private val baseUiState: StateFlow<MainScreenUiState> =
    combine(setupRepository.configuration, setupForm, isSaving, setupErrorMessage, selectedTab) { configuration, form, saving, errorMessage, currentTab ->
        if (configuration == null) {
          MainScreenUiState.Setup(baseUrl = form.baseUrl, apiKey = form.apiKey, isSaving = saving, errorMessage = errorMessage)
        } else {
          MainScreenUiState.App(selectedTab = currentTab)
        }
      }
      .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS), MainScreenUiState.Loading)

  val uiState: StateFlow<MainScreenUiState> =
    combine(baseUiState, irRepository.state, homeRemoteControlsState) { state, irState, currentHomeRemoteControlsState ->
        when (state) {
          MainScreenUiState.Loading -> MainScreenUiState.Loading
          is MainScreenUiState.Setup -> state
          is MainScreenUiState.App -> state.copy(irState = irState, homeRemoteControlsState = currentHomeRemoteControlsState)
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
    if (isSaving.value) {
      return
    }

    viewModelScope.launch {
      try {
        isSaving.value = true
        setupErrorMessage.value = null
        val form = setupForm.value

        setupRepository
          .validateAndSave(baseUrl = form.baseUrl, apiKey = form.apiKey)
          .onSuccess { selectedTab.value = TopLevelTab.Home }
          .onFailure(::showSetupError)
      } catch (exception: CancellationException) {
        throw exception
      } catch (throwable: Throwable) {
        showSetupError(throwable)
      } finally {
        isSaving.value = false
      }
    }
  }

  fun onTabSelected(tab: TopLevelTab) {
    selectedTab.value = tab
    if (tab == TopLevelTab.Remote && irRepository.state.value is IrState.Error) {
      refreshIrStatus()
    }
  }

  fun retryIrStatus() {
    refreshIrStatus()
  }

  fun sendHomeRemoteCommand(command: String) {
    if (command !in HOME_REMOTE_CONTROL_COMMANDS || activeConfiguration == null) {
      return
    }

    val irStatus = (irRepository.state.value as? IrState.Loaded)?.status ?: return
    if (command !in irStatus.availableCommands || command in homeRemoteControlsState.value.sendingCommands) {
      return
    }

    val generation = homeRemoteControlsGeneration.get()
    startSendingHomeRemoteCommand(command)

    viewModelScope.launch {
      val result =
        try {
          irRepository.sendCommand(command)
        } catch (exception: CancellationException) {
          throw exception
        } catch (throwable: Throwable) {
          Result.failure(throwable)
        }

      result
        .onSuccess { finishSendingHomeRemoteCommand(generation, command) }
        .onFailure { throwable -> showHomeRemoteSendError(generation, command, throwable) }
    }
  }

  private fun updateSetupForm(transform: SetupForm.() -> SetupForm) {
    setupForm.value = setupForm.value.transform()
    setupErrorMessage.value = null
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

  private fun showSetupError(throwable: Throwable) {
    setupErrorMessage.value = throwable.message ?: DEFAULT_VALIDATION_ERROR
  }

  private fun startSendingHomeRemoteCommand(command: String) {
    homeRemoteControlsState.update { currentState ->
      currentState.copy(sendingCommands = currentState.sendingCommands + command, errorMessage = null, errorCommand = null)
    }
  }

  private fun finishSendingHomeRemoteCommand(generation: Long, command: String) {
    updateHomeRemoteControlsStateIfCurrent(generation) { currentState ->
      currentState.copy(sendingCommands = currentState.sendingCommands - command)
    }
  }

  private fun showHomeRemoteSendError(generation: Long, command: String, throwable: Throwable) {
    updateHomeRemoteControlsStateIfCurrent(generation) { currentState ->
      currentState.copy(
        sendingCommands = currentState.sendingCommands - command,
        errorMessage = throwable.message ?: DEFAULT_SEND_ERROR,
        errorCommand = command,
      )
    }
  }

  private fun resetHomeRemoteControlsState() {
    homeRemoteControlsGeneration.incrementAndGet()
    homeRemoteControlsState.value = HomeRemoteControlsState()
  }

  private fun updateHomeRemoteControlsStateIfCurrent(generation: Long, transform: (HomeRemoteControlsState) -> HomeRemoteControlsState) {
    if (homeRemoteControlsGeneration.get() != generation) {
      return
    }

    homeRemoteControlsState.update(transform)
  }

  private companion object {
    const val STOP_TIMEOUT_MILLIS = 5_000L
    const val DEFAULT_VALIDATION_ERROR = "Couldn't validate that configuration."
    const val DEFAULT_SEND_ERROR = "Couldn't send that IR command."
  }
}

sealed interface MainScreenUiState {
  object Loading : MainScreenUiState

  data class Setup(
    val baseUrl: String = "",
    val apiKey: String = "",
    val isSaving: Boolean = false,
    val errorMessage: String? = null,
  ) : MainScreenUiState

  data class App(
    val selectedTab: TopLevelTab = TopLevelTab.Home,
    val irState: IrState = IrState.Idle,
    val homeRemoteControlsState: HomeRemoteControlsState = HomeRemoteControlsState(),
  ) : MainScreenUiState
}

data class HomeRemoteControlsState(
  val sendingCommands: Set<String> = emptySet(),
  val errorMessage: String? = null,
  val errorCommand: String? = null,
)

enum class TopLevelTab {
  Home,
  Remote,
}

private data class SetupForm(val baseUrl: String = "", val apiKey: String = "")
