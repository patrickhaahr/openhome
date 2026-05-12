package com.example.openhome.ui.main

import com.example.openhome.data.IrRepository
import com.example.openhome.data.IrState
import com.example.openhome.data.IrStatus
import com.example.openhome.data.SetupRepository
import com.example.openhome.data.StoredConfiguration
import junit.framework.TestCase.assertEquals
import junit.framework.TestCase.assertTrue
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class MainScreenViewModelTest {
  @Before
  fun setUp() {
    Dispatchers.setMain(StandardTestDispatcher())
  }

  @After
  fun tearDown() {
    Dispatchers.resetMain()
  }

  @Test
  fun uiState_withoutStoredConfiguration_showsSetupFlow() = runTest {
    val viewModel = MainScreenViewModel(FakeSetupRepository(), FakeIrRepository())

    assertEquals(configurationFormState(), viewModel.awaitState<MainScreenUiState.ConfigurationForm>())
  }

  @Test
  fun uiState_withStoredConfiguration_showsHomeTabAndStartsIrPreload() = runTest {
    val irRepository = FakeIrRepository()
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()

    assertEquals(appState(), viewModel.awaitState<MainScreenUiState.App>())
    assertEquals(1, irRepository.refreshCallCount)
  }

  @Test
  fun submitSetup_withValidConfiguration_savesAndShowsApp() = runTest {
    val repository = FakeSetupRepository()
    val irRepository = FakeIrRepository()
    val viewModel = MainScreenViewModel(repository, irRepository)

    viewModel.onBaseUrlChanged(VALID_CONFIGURATION.baseUrl)
    viewModel.onApiKeyChanged(VALID_CONFIGURATION.apiKey)
    viewModel.submitSetup()
    advanceUntilIdle()

    assertEquals(listOf(VALID_CONFIGURATION), repository.savedConfigurations)
    assertEquals(appState(), viewModel.awaitState<MainScreenUiState.App>())
    assertEquals(1, irRepository.refreshCallCount)
  }

  @Test
  fun submitSetup_withInvalidConfiguration_keepsSetupVisible() = runTest {
    val repository = FakeSetupRepository(saveResult = Result.failure(IllegalStateException("OpenHome rejected that Base URL or API Key.")))
    val viewModel = MainScreenViewModel(repository, FakeIrRepository())

    viewModel.onBaseUrlChanged("http://192.168.1.20:8000")
    viewModel.onApiKeyChanged("wrong")
    viewModel.submitSetup()
    advanceUntilIdle()

    val setupState = viewModel.awaitState<MainScreenUiState.ConfigurationForm>()
    assertEquals("OpenHome rejected that Base URL or API Key.", setupState.errorMessage)
    assertTrue(repository.savedConfigurations.isEmpty())
  }

  @Test
  fun submitSetup_whenRepositoryThrows_showsErrorAndStopsSaving() = runTest {
    val viewModel = MainScreenViewModel(FakeSetupRepository(saveException = IllegalStateException("Couldn't persist configuration.")), FakeIrRepository())

    viewModel.onBaseUrlChanged(VALID_CONFIGURATION.baseUrl)
    viewModel.onApiKeyChanged(VALID_CONFIGURATION.apiKey)
    viewModel.submitSetup()
    advanceUntilIdle()

    assertEquals(
      configurationFormState(
        baseUrl = VALID_CONFIGURATION.baseUrl,
        apiKey = VALID_CONFIGURATION.apiKey,
        isSaving = false,
        errorMessage = "Couldn't persist configuration.",
      ),
      viewModel.awaitState<MainScreenUiState.ConfigurationForm>(),
    )
  }

  @Test
  fun uiState_whenStoredConfigurationIsRemoved_returnsToSetupFlow() = runTest {
    val repository = FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION)
    val irRepository = FakeIrRepository()
    val viewModel = MainScreenViewModel(repository, irRepository)

    advanceUntilIdle()

    assertEquals(appState(), viewModel.awaitState<MainScreenUiState.App>())
    repository.updateConfiguration(null)
    advanceUntilIdle()

    assertEquals(configurationFormState(), viewModel.awaitState<MainScreenUiState.ConfigurationForm>())
    assertEquals(1, irRepository.resetCallCount)
  }

  @Test
  fun uiState_whenConfigurationChangesWhileIrLoads_startsFreshIrPreload() = runTest {
    val initialRefresh = CompletableDeferred<Result<IrStatus>>()
    val updatedRefresh = CompletableDeferred<Result<IrStatus>>()
    val repository = FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION)
    val irRepository = FakeIrRepository(pendingRefreshes = mutableListOf(initialRefresh, updatedRefresh))
    val viewModel = MainScreenViewModel(repository, irRepository)

    advanceUntilIdle()
    repository.updateConfiguration(UPDATED_CONFIGURATION)
    advanceUntilIdle()

    assertEquals(2, irRepository.refreshCallCount)

    initialRefresh.complete(Result.success(IrStatus(message = "Old server ready", availableCommands = setOf("mute"))))
    updatedRefresh.complete(Result.success(IrStatus(message = "New server ready", availableCommands = setOf("bluetooth"))))
    advanceUntilIdle()

    assertEquals(
      appState(
        irState = IrState.Loaded(IrStatus(message = "New server ready", availableCommands = setOf("bluetooth"))),
      ),
      viewModel.awaitState<MainScreenUiState.App>(),
    )
  }

  @Test
  fun openReconfiguration_withStoredConfiguration_showsPrefilledForm() = runTest {
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), FakeIrRepository())

    advanceUntilIdle()
    viewModel.openReconfiguration()
    advanceUntilIdle()

    assertEquals(
      configurationFormState(mode = ConfigurationFormMode.Reconfigure, baseUrl = VALID_CONFIGURATION.baseUrl, apiKey = VALID_CONFIGURATION.apiKey),
      viewModel.awaitState<MainScreenUiState.ConfigurationForm>(),
    )
  }

  @Test
  fun cancelReconfiguration_returnsToPreviouslySelectedTab() = runTest {
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), FakeIrRepository())

    advanceUntilIdle()
    viewModel.onTabSelected(TopLevelTab.Remote)
    viewModel.openReconfiguration()
    advanceUntilIdle()
    viewModel.onBaseUrlChanged(UPDATED_CONFIGURATION.baseUrl)
    viewModel.onApiKeyChanged(UPDATED_CONFIGURATION.apiKey)
    viewModel.cancelReconfiguration()
    advanceUntilIdle()

    assertEquals(appState(selectedTab = TopLevelTab.Remote), viewModel.awaitState<MainScreenUiState.App>())
  }

  @Test
  fun submitReconfiguration_withInvalidConfiguration_keepsPreviousConfigurationActive() = runTest {
    val repository =
      FakeSetupRepository(
        initialConfiguration = VALID_CONFIGURATION,
        saveResult = Result.failure(IllegalStateException("OpenHome rejected that Base URL or API Key.")),
      )
    val irRepository = FakeIrRepository()
    val viewModel = MainScreenViewModel(repository, irRepository)

    advanceUntilIdle()
    viewModel.openReconfiguration()
    advanceUntilIdle()
    viewModel.onBaseUrlChanged(UPDATED_CONFIGURATION.baseUrl)
    viewModel.onApiKeyChanged("wrong")
    viewModel.submitSetup()
    advanceUntilIdle()

    assertEquals(
      configurationFormState(
        mode = ConfigurationFormMode.Reconfigure,
        baseUrl = UPDATED_CONFIGURATION.baseUrl,
        apiKey = "wrong",
        errorMessage = "OpenHome rejected that Base URL or API Key.",
      ),
      viewModel.awaitState<MainScreenUiState.ConfigurationForm>(),
    )
    assertEquals(VALID_CONFIGURATION, repository.configuration.first())
    assertEquals(1, irRepository.refreshCallCount)
  }

  @Test
  fun submitReconfiguration_withValidConfiguration_updatesConfigAndReturnsToApp() = runTest {
    val repository = FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION)
    val irRepository =
      FakeIrRepository(
        refreshResults =
          mutableListOf(
            Result.success(DEFAULT_IR_STATUS),
            Result.success(IrStatus(message = "Office ready", availableCommands = setOf("power", "mute"))),
          ),
      )
    val viewModel = MainScreenViewModel(repository, irRepository)

    advanceUntilIdle()
    viewModel.onTabSelected(TopLevelTab.Remote)
    viewModel.openReconfiguration()
    advanceUntilIdle()
    viewModel.onBaseUrlChanged(UPDATED_CONFIGURATION.baseUrl)
    viewModel.onApiKeyChanged(UPDATED_CONFIGURATION.apiKey)
    viewModel.submitSetup()
    advanceUntilIdle()

    assertEquals(listOf(UPDATED_CONFIGURATION), repository.savedConfigurations)
    assertEquals(UPDATED_CONFIGURATION, repository.configuration.first())
    assertEquals(2, irRepository.refreshCallCount)
    assertEquals(
      appState(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(IrStatus(message = "Office ready", availableCommands = setOf("power", "mute"))),
      ),
      viewModel.awaitState<MainScreenUiState.App>(),
    )
  }

  @Test
  fun onTabSelected_remoteAfterFailedPreload_retriesIrStatus() = runTest {
    val irRepository =
      FakeIrRepository(
        refreshResults =
          mutableListOf(
            Result.failure(IllegalStateException("Couldn't load IR status from the Axum API.")),
            Result.success(IrStatus(message = "IR remote ready", availableCommands = setOf("bluetooth"))),
          ),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.onTabSelected(TopLevelTab.Remote)
    advanceUntilIdle()

    assertEquals(2, irRepository.refreshCallCount)
    assertEquals(
      appState(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("bluetooth"))),
      ),
      viewModel.awaitState<MainScreenUiState.App>(),
    )
  }

  @Test
  fun retryIrStatus_whenPreloadFailed_runsManualRetry() = runTest {
    val irRepository =
      FakeIrRepository(
        refreshResults =
          mutableListOf(
            Result.failure(IllegalStateException("Couldn't load IR status from the Axum API.")),
            Result.success(IrStatus(message = "Living room ready", availableCommands = setOf("optical", "bluetooth"))),
          ),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.retryIrStatus()
    advanceUntilIdle()

    assertEquals(2, irRepository.refreshCallCount)
    assertEquals(
      appState(
        irState = IrState.Loaded(IrStatus(message = "Living room ready", availableCommands = setOf("optical", "bluetooth"))),
      ),
      viewModel.awaitState<MainScreenUiState.App>(),
    )
  }

  @Test
  fun sendHomeRemoteCommand_withAvailableCommand_sendsWithoutRefreshingIrStatus() = runTest {
    val sendResult = CompletableDeferred<Result<Unit>>()
    val irRepository =
      FakeIrRepository(
        refreshResults = mutableListOf(Result.success(IrStatus(message = "Living room ready", availableCommands = setOf("bluetooth", "optical")))),
        pendingSendResults = mutableMapOf("bluetooth" to sendResult),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendHomeRemoteCommand("bluetooth")
    advanceUntilIdle()

    assertEquals(setOf("bluetooth"), viewModel.awaitState<MainScreenUiState.App>().homeRemoteControlsState.sendingCommands)
    assertEquals(listOf("bluetooth"), irRepository.sentCommands)
    assertEquals(1, irRepository.refreshCallCount)

    sendResult.complete(Result.success(Unit))
    advanceUntilIdle()

    assertEquals(HomeRemoteControlsState(), viewModel.awaitState<MainScreenUiState.App>().homeRemoteControlsState)
    assertEquals(1, irRepository.refreshCallCount)
  }

  @Test
  fun sendHomeRemoteCommand_withFailedCommand_showsActionErrorWithoutChangingIrState() = runTest {
    val initialStatus = IrStatus(message = "Living room ready", availableCommands = setOf("bluetooth", "optical"))
    val irRepository =
      FakeIrRepository(
        refreshResults = mutableListOf(Result.success(initialStatus)),
        sendResults = mutableMapOf("optical" to Result.failure(IllegalStateException("IR bridge offline"))),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendHomeRemoteCommand("optical")
    advanceUntilIdle()

    assertEquals(
      HomeRemoteControlsState(errorMessage = "IR bridge offline", errorCommand = "optical"),
      viewModel.awaitState<MainScreenUiState.App>().homeRemoteControlsState,
    )
    assertEquals(IrState.Loaded(initialStatus), viewModel.awaitState<MainScreenUiState.App>().irState)
  }

  @Test
  fun sendHomeRemoteCommand_withUnavailableCommand_doesNothing() = runTest {
    val irRepository =
      FakeIrRepository(
        initialState = IrState.Loaded(IrStatus(message = "Living room ready", availableCommands = setOf("bluetooth"))),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendHomeRemoteCommand("optical")
    advanceUntilIdle()

    assertTrue(irRepository.sentCommands.isEmpty())
    assertEquals(HomeRemoteControlsState(), viewModel.awaitState<MainScreenUiState.App>().homeRemoteControlsState)
  }

  @Test
  fun sendHomeRemoteCommand_whenConfigurationChanges_ignoresStaleCompletion() = runTest {
    val pendingSend = CompletableDeferred<Result<Unit>>()
    val repository = FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION)
    val irRepository =
      FakeIrRepository(
        refreshResults =
          mutableListOf(
            Result.success(IrStatus(message = "Living room ready", availableCommands = setOf("bluetooth", "optical"))),
            Result.success(IrStatus(message = "Office ready", availableCommands = setOf("bluetooth", "optical"))),
          ),
        pendingSendResults = mutableMapOf("bluetooth" to pendingSend),
      )
    val viewModel = MainScreenViewModel(repository, irRepository)

    advanceUntilIdle()
    viewModel.sendHomeRemoteCommand("bluetooth")
    advanceUntilIdle()

    repository.updateConfiguration(UPDATED_CONFIGURATION)
    advanceUntilIdle()
    pendingSend.complete(Result.failure(IllegalStateException("Old server offline")))
    advanceUntilIdle()

    assertEquals(
      appState(
        irState = IrState.Loaded(IrStatus(message = "Office ready", availableCommands = setOf("bluetooth", "optical"))),
      ),
      viewModel.awaitState<MainScreenUiState.App>(),
    )
  }

  @Test
  fun sendRemoteCommand_whenSameCommandIsAlreadySendingFromHome_doesNothing() = runTest {
    val pendingSend = CompletableDeferred<Result<Unit>>()
    val irRepository =
      FakeIrRepository(
        refreshResults = mutableListOf(Result.success(IrStatus(message = "Living room ready", availableCommands = setOf("bluetooth", "optical")))),
        pendingSendResults = mutableMapOf("bluetooth" to pendingSend),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendHomeRemoteCommand("bluetooth")
    advanceUntilIdle()
    viewModel.sendRemoteCommand("bluetooth")
    advanceUntilIdle()

    assertEquals(listOf("bluetooth"), irRepository.sentCommands)
    assertEquals(setOf("bluetooth"), viewModel.awaitState<MainScreenUiState.App>().homeRemoteControlsState.sendingCommands)
    assertEquals(RemoteControlsState(), viewModel.awaitState<MainScreenUiState.App>().remoteControlsState)

    pendingSend.complete(Result.success(Unit))
    advanceUntilIdle()
  }

  @Test
  fun sendRemoteCommand_withAvailableCommand_sendsWithoutRefreshingIrStatus() = runTest {
    val sendResult = CompletableDeferred<Result<Unit>>()
    val irRepository =
      FakeIrRepository(
        refreshResults = mutableListOf(Result.success(IrStatus(message = "Living room ready", availableCommands = setOf("power", "mute")))),
        pendingSendResults = mutableMapOf("power" to sendResult),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendRemoteCommand("power")
    advanceUntilIdle()

    assertEquals(setOf("power"), viewModel.awaitState<MainScreenUiState.App>().remoteControlsState.sendingCommands)
    assertEquals(listOf("power"), irRepository.sentCommands)
    assertEquals(1, irRepository.refreshCallCount)

    sendResult.complete(Result.success(Unit))
    advanceUntilIdle()

    assertEquals(RemoteControlsState(), viewModel.awaitState<MainScreenUiState.App>().remoteControlsState)
    assertEquals(1, irRepository.refreshCallCount)
  }

  @Test
  fun sendRemoteCommand_withFailedCommand_showsActionErrorWithoutChangingIrState() = runTest {
    val initialStatus = IrStatus(message = "Living room ready", availableCommands = setOf("mute", "power"))
    val irRepository =
      FakeIrRepository(
        refreshResults = mutableListOf(Result.success(initialStatus)),
        sendResults = mutableMapOf("mute" to Result.failure(IllegalStateException("IR bridge offline"))),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendRemoteCommand("mute")
    advanceUntilIdle()

    assertEquals(
      RemoteControlsState(errorMessage = "IR bridge offline", errorCommand = "mute"),
      viewModel.awaitState<MainScreenUiState.App>().remoteControlsState,
    )
    assertEquals(IrState.Loaded(initialStatus), viewModel.awaitState<MainScreenUiState.App>().irState)
  }

  @Test
  fun sendRemoteCommand_withUnavailableCommand_doesNothing() = runTest {
    val irRepository =
      FakeIrRepository(
        initialState = IrState.Loaded(IrStatus(message = "Living room ready", availableCommands = setOf("power"))),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendRemoteCommand("mute")
    advanceUntilIdle()

    assertTrue(irRepository.sentCommands.isEmpty())
    assertEquals(RemoteControlsState(), viewModel.awaitState<MainScreenUiState.App>().remoteControlsState)
  }

  @Test
  fun sendRemoteCommand_whenConfigurationChanges_ignoresStaleCompletion() = runTest {
    val pendingSend = CompletableDeferred<Result<Unit>>()
    val repository = FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION)
    val irRepository =
      FakeIrRepository(
        refreshResults =
          mutableListOf(
            Result.success(IrStatus(message = "Living room ready", availableCommands = setOf("power", "mute"))),
            Result.success(IrStatus(message = "Office ready", availableCommands = setOf("power", "mute"))),
          ),
        pendingSendResults = mutableMapOf("power" to pendingSend),
      )
    val viewModel = MainScreenViewModel(repository, irRepository)

    advanceUntilIdle()
    viewModel.sendRemoteCommand("power")
    advanceUntilIdle()

    repository.updateConfiguration(UPDATED_CONFIGURATION)
    advanceUntilIdle()
    pendingSend.complete(Result.failure(IllegalStateException("Old server offline")))
    advanceUntilIdle()

    assertEquals(
      appState(
        irState = IrState.Loaded(IrStatus(message = "Office ready", availableCommands = setOf("power", "mute"))),
      ),
      viewModel.awaitState<MainScreenUiState.App>(),
    )
  }

  @Test
  fun sendHomeRemoteCommand_whenSameCommandIsAlreadySendingFromRemote_doesNothing() = runTest {
    val pendingSend = CompletableDeferred<Result<Unit>>()
    val irRepository =
      FakeIrRepository(
        refreshResults = mutableListOf(Result.success(IrStatus(message = "Living room ready", availableCommands = setOf("bluetooth", "optical")))),
        pendingSendResults = mutableMapOf("bluetooth" to pendingSend),
      )
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION), irRepository)

    advanceUntilIdle()
    viewModel.sendRemoteCommand("bluetooth")
    advanceUntilIdle()
    viewModel.sendHomeRemoteCommand("bluetooth")
    advanceUntilIdle()

    assertEquals(listOf("bluetooth"), irRepository.sentCommands)
    assertEquals(HomeRemoteControlsState(), viewModel.awaitState<MainScreenUiState.App>().homeRemoteControlsState)
    assertEquals(setOf("bluetooth"), viewModel.awaitState<MainScreenUiState.App>().remoteControlsState.sendingCommands)

    pendingSend.complete(Result.success(Unit))
    advanceUntilIdle()
  }
}

private suspend inline fun <reified T : MainScreenUiState> MainScreenViewModel.awaitState(): T = uiState.first { it is T } as T

private class FakeSetupRepository(
  initialConfiguration: StoredConfiguration? = null,
  private val saveResult: Result<StoredConfiguration> = Result.success(VALID_CONFIGURATION),
  private val saveException: Throwable? = null,
) : SetupRepository {
  private val configurationState = MutableStateFlow(initialConfiguration)

  val savedConfigurations = mutableListOf<StoredConfiguration>()

  override val configuration: Flow<StoredConfiguration?> = configurationState

  override suspend fun validateAndSave(baseUrl: String, apiKey: String): Result<StoredConfiguration> {
    saveException?.let { throw it }
    val attemptedConfiguration = StoredConfiguration(baseUrl = baseUrl, apiKey = apiKey)
    return saveResult.onSuccess {
      savedConfigurations += attemptedConfiguration
      configurationState.value = attemptedConfiguration
    }
  }

  fun updateConfiguration(configuration: StoredConfiguration?) {
    configurationState.value = configuration
  }
}

private class FakeIrRepository(
  initialState: IrState = IrState.Idle,
  private val refreshResults: MutableList<Result<IrStatus>> = mutableListOf(Result.success(DEFAULT_IR_STATUS)),
  private val pendingRefreshes: MutableList<CompletableDeferred<Result<IrStatus>>> = mutableListOf(),
  private val sendResults: MutableMap<String, Result<Unit>> = mutableMapOf(),
  private val pendingSendResults: MutableMap<String, CompletableDeferred<Result<Unit>>> = mutableMapOf(),
) : IrRepository {
  private val stateFlow = MutableStateFlow(initialState)

  var refreshCallCount = 0
    private set

  var resetCallCount = 0
    private set

  val sentCommands = mutableListOf<String>()

  override val state: StateFlow<IrState> = stateFlow.asStateFlow()

  override suspend fun refresh(): Result<IrStatus> {
    refreshCallCount += 1
    stateFlow.value = IrState.Loading
    val result = pendingRefreshes.removeFirstOrNull()?.await() ?: refreshResults.removeFirstOrNull() ?: Result.success(DEFAULT_IR_STATUS)
    result
      .onSuccess { stateFlow.value = IrState.Loaded(it) }
      .onFailure { throwable -> stateFlow.value = IrState.Error(throwable.message ?: "Couldn't load IR status from the Axum API.") }
    return result
  }

  override suspend fun sendCommand(command: String): Result<Unit> {
    sentCommands += command
    return pendingSendResults.remove(command)?.await() ?: sendResults.remove(command) ?: Result.success(Unit)
  }

  override fun reset() {
    resetCallCount += 1
    stateFlow.value = IrState.Idle
  }
}

private fun appState(
  selectedTab: TopLevelTab = TopLevelTab.Home,
  irState: IrState = IrState.Loaded(DEFAULT_IR_STATUS),
  homeRemoteControlsState: HomeRemoteControlsState = HomeRemoteControlsState(),
  remoteControlsState: RemoteControlsState = RemoteControlsState(),
): MainScreenUiState.App =
  MainScreenUiState.App(
    selectedTab = selectedTab,
    irState = irState,
    homeRemoteControlsState = homeRemoteControlsState,
    remoteControlsState = remoteControlsState,
  )

private fun configurationFormState(
  mode: ConfigurationFormMode = ConfigurationFormMode.Setup,
  baseUrl: String = "",
  apiKey: String = "",
  isSaving: Boolean = false,
  errorMessage: String? = null,
): MainScreenUiState.ConfigurationForm =
  MainScreenUiState.ConfigurationForm(
    mode = mode,
    baseUrl = baseUrl,
    apiKey = apiKey,
    isSaving = isSaving,
    errorMessage = errorMessage,
  )

private val VALID_CONFIGURATION = StoredConfiguration(baseUrl = "http://192.168.1.20:8000", apiKey = "secret")

private val UPDATED_CONFIGURATION = StoredConfiguration(baseUrl = "http://192.168.1.21:8000", apiKey = "secret-2")

private val DEFAULT_IR_STATUS = IrStatus(message = "IR remote ready", availableCommands = emptySet())
