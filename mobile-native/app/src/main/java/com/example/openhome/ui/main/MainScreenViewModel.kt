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
import kotlinx.coroutines.launch

class MainScreenViewModel(
  private val setupRepository: SetupRepository,
  private val irRepository: IrRepository,
) : ViewModel() {
  private val setupForm = MutableStateFlow(SetupForm())
  private val isSaving = MutableStateFlow(false)
  private val setupErrorMessage = MutableStateFlow<String?>(null)
  private val selectedTab = MutableStateFlow(TopLevelTab.Home)
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
    combine(baseUiState, irRepository.state) { state, irState ->
        when (state) {
          MainScreenUiState.Loading -> MainScreenUiState.Loading
          is MainScreenUiState.Setup -> state
          is MainScreenUiState.App -> state.copy(irState = irState)
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

  private companion object {
    const val STOP_TIMEOUT_MILLIS = 5_000L
    const val DEFAULT_VALIDATION_ERROR = "Couldn't validate that configuration."
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
  ) : MainScreenUiState
}

enum class TopLevelTab {
  Home,
  Remote,
}

private data class SetupForm(val baseUrl: String = "", val apiKey: String = "")
