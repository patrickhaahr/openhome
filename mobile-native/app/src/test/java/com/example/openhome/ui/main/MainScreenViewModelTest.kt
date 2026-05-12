package com.example.openhome.ui.main

import com.example.openhome.data.SetupRepository
import com.example.openhome.data.StoredConfiguration
import junit.framework.TestCase.assertEquals
import junit.framework.TestCase.assertTrue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
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
    val viewModel = MainScreenViewModel(FakeSetupRepository())

    assertEquals(MainScreenUiState.Setup(), viewModel.awaitState<MainScreenUiState.Setup>())
  }

  @Test
  fun uiState_withStoredConfiguration_showsHomeTab() = runTest {
    val viewModel = MainScreenViewModel(FakeSetupRepository(initialConfiguration = VALID_CONFIGURATION))

    assertEquals(MainScreenUiState.App(), viewModel.awaitState<MainScreenUiState.App>())
  }

  @Test
  fun submitSetup_withValidConfiguration_savesAndShowsApp() = runTest {
    val repository = FakeSetupRepository()
    val viewModel = MainScreenViewModel(repository)

    viewModel.onBaseUrlChanged(VALID_CONFIGURATION.baseUrl)
    viewModel.onApiKeyChanged(VALID_CONFIGURATION.apiKey)
    viewModel.submitSetup()
    advanceUntilIdle()

    assertEquals(listOf(VALID_CONFIGURATION), repository.savedConfigurations)
    assertEquals(MainScreenUiState.App(), viewModel.awaitState<MainScreenUiState.App>())
  }

  @Test
  fun submitSetup_withInvalidConfiguration_keepsSetupVisible() = runTest {
    val repository = FakeSetupRepository(saveResult = Result.failure(IllegalStateException("OpenHome rejected that Base URL or API Key.")))
    val viewModel = MainScreenViewModel(repository)

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
    val viewModel = MainScreenViewModel(FakeSetupRepository(saveException = IllegalStateException("Couldn't persist configuration.")))

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
    val viewModel = MainScreenViewModel(repository)

    assertEquals(MainScreenUiState.App(), viewModel.awaitState<MainScreenUiState.App>())
    repository.updateConfiguration(null)

    assertEquals(MainScreenUiState.Setup(), viewModel.awaitState<MainScreenUiState.Setup>())
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

private val VALID_CONFIGURATION = StoredConfiguration(baseUrl = "http://192.168.1.20:8000", apiKey = "secret")
