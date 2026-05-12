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

    assertEquals(MainScreenUiState.Setup(), viewModel.awaitState<MainScreenUiState.Setup>())
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

    val setupState = viewModel.awaitState<MainScreenUiState.Setup>()
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
      MainScreenUiState.Setup(
        baseUrl = VALID_CONFIGURATION.baseUrl,
        apiKey = VALID_CONFIGURATION.apiKey,
        isSaving = false,
        errorMessage = "Couldn't persist configuration.",
      ),
      viewModel.awaitState<MainScreenUiState.Setup>(),
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

    assertEquals(MainScreenUiState.Setup(), viewModel.awaitState<MainScreenUiState.Setup>())
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
) : IrRepository {
  private val stateFlow = MutableStateFlow(initialState)

  var refreshCallCount = 0
    private set

  var resetCallCount = 0
    private set

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

  override fun reset() {
    resetCallCount += 1
    stateFlow.value = IrState.Idle
  }
}

private fun appState(
  selectedTab: TopLevelTab = TopLevelTab.Home,
  irState: IrState = IrState.Loaded(DEFAULT_IR_STATUS),
): MainScreenUiState.App = MainScreenUiState.App(selectedTab = selectedTab, irState = irState)

private val VALID_CONFIGURATION = StoredConfiguration(baseUrl = "http://192.168.1.20:8000", apiKey = "secret")

private val UPDATED_CONFIGURATION = StoredConfiguration(baseUrl = "http://192.168.1.21:8000", apiKey = "secret-2")

private val DEFAULT_IR_STATUS = IrStatus(message = "IR remote ready", availableCommands = emptySet())
