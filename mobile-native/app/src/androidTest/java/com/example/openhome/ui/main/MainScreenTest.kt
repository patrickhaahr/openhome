package com.example.openhome.ui.main

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.assertExists
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import junit.framework.TestCase.assertEquals
import org.junit.Rule
import org.junit.Test

import com.example.openhome.data.IrState
import com.example.openhome.data.IrStatus

/** UI tests for [com.example.openhome.ui.main.MainScreen]. */
class MainScreenTest {

  @get:Rule val composeTestRule = createAndroidComposeRule<ComponentActivity>()

  @Test
  fun setupFlow_showsBaseUrlAndApiKeyFields() {
    renderScreen(configurationFormState())

    composeTestRule.onNodeWithText("Set up OpenHome").assertExists()
    composeTestRule.onNodeWithText("Base URL").assertExists()
    composeTestRule.onNodeWithText("API Key").assertExists()
    composeTestRule.onNodeWithText("Validate and continue").assertExists()
  }

  @Test
  fun appShell_showsInitialTabs() {
    renderScreen(MainScreenUiState.App())

    composeTestRule.onNodeWithText("Home").assertExists()
    composeTestRule.onNodeWithText("Remote").assertExists()
    composeTestRule.onNodeWithTag("open-reconfiguration").assertExists()
  }

  @Test
  fun reconfigureScreen_showsPrefilledFormAndCancelAction() {
    renderScreen(
      configurationFormState(
        mode = ConfigurationFormMode.Reconfigure,
        baseUrl = "https://openhome.example",
        apiKey = "replacement",
      ),
    )

    composeTestRule.onNodeWithText("Update configuration").assertExists()
    composeTestRule.onNodeWithText("Save configuration").assertExists()
    composeTestRule.onNodeWithTag("configuration-cancel").assertExists()
    composeTestRule.onNodeWithText("https://openhome.example").assertExists()
  }

  @Test
  fun homeTab_showsExactlyTwoQuickControls() {
    renderScreen(
      MainScreenUiState.App(
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("bluetooth", "optical"))),
      ),
    )

    composeTestRule.onNodeWithText("Quick controls").assertExists()
    composeTestRule.onNodeWithText("Bluetooth").assertExists()
    composeTestRule.onNodeWithText("Optical").assertExists()
    assertEquals(1, composeTestRule.onAllNodesWithText("Bluetooth").fetchSemanticsNodes().size)
    assertEquals(1, composeTestRule.onAllNodesWithText("Optical").fetchSemanticsNodes().size)
  }

  @Test
  fun homeTab_quickControlClick_callsHandler() {
    var tappedCommand: String? = null

    renderScreen(
      MainScreenUiState.App(
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("bluetooth", "optical"))),
      ),
      onSendHomeRemoteCommand = { tappedCommand = it },
    )

    composeTestRule.onNodeWithText("Bluetooth").performClick()

    assertEquals("bluetooth", tappedCommand)
  }

  @Test
  fun homeTab_onlyTappedQuickControlIsBlockedWhileSending() {
    renderScreen(
      MainScreenUiState.App(
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("bluetooth", "optical"))),
        homeRemoteControlsState = HomeRemoteControlsState(sendingCommands = setOf("bluetooth")),
      ),
    )

    composeTestRule.onNodeWithTag("home-remote-bluetooth").assertIsNotEnabled()
    composeTestRule.onNodeWithTag("home-remote-optical").assertIsEnabled()
    composeTestRule.onNodeWithTag("home-remote-bluetooth-label").assertTextEquals("Bluetooth")
    composeTestRule.onNodeWithTag("home-remote-optical-label").assertTextEquals("Optical")
    assertEquals(1, composeTestRule.onAllNodesWithTag("home-remote-bluetooth-progress").fetchSemanticsNodes().size)
    assertEquals(0, composeTestRule.onAllNodesWithTag("home-remote-optical-progress").fetchSemanticsNodes().size)
  }

  @Test
  fun remoteTab_showsFullV1RemoteButtonSet() {
    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = REMOTE_BUTTON_COMMANDS)),
      ),
    )

    composeTestRule.onNodeWithTag("remote-power").assertExists()
    composeTestRule.onNodeWithTag("remote-bluetooth").assertExists()
    composeTestRule.onNodeWithTag("remote-optical").assertExists()
    composeTestRule.onNodeWithTag("remote-mute").assertExists()
    composeTestRule.onNodeWithTag("remote-volume-down").assertExists()
    composeTestRule.onNodeWithTag("remote-volume-up").assertExists()
    assertEquals(6, composeTestRule.onAllNodesWithTag("remote-button").fetchSemanticsNodes().size)
  }

  @Test
  fun remoteTab_unavailableCommandsRemainVisibleButDisabled() {
    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("power", "mute"))),
      ),
    )

    composeTestRule.onNodeWithTag("remote-power").assertIsEnabled()
    composeTestRule.onNodeWithTag("remote-mute").assertIsEnabled()
    composeTestRule.onNodeWithTag("remote-bluetooth").assertExists().assertIsNotEnabled()
    composeTestRule.onNodeWithTag("remote-optical").assertExists().assertIsNotEnabled()
    composeTestRule.onNodeWithTag("remote-volume-down").assertExists().assertIsNotEnabled()
    composeTestRule.onNodeWithTag("remote-volume-up").assertExists().assertIsNotEnabled()
  }

  @Test
  fun remoteTab_buttonClick_callsHandler() {
    var tappedCommand: String? = null

    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = REMOTE_BUTTON_COMMANDS)),
      ),
      onSendRemoteCommand = { tappedCommand = it },
    )

    composeTestRule.onNodeWithTag("remote-power").performClick()

    assertEquals("power", tappedCommand)
  }

  @Test
  fun remoteTab_onlyTappedButtonIsBlockedWhileSending() {
    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("power", "mute"))),
        remoteControlsState = RemoteControlsState(sendingCommands = setOf("power")),
      ),
    )

    composeTestRule.onNodeWithTag("remote-power").assertIsNotEnabled()
    composeTestRule.onNodeWithTag("remote-mute").assertIsEnabled()
    composeTestRule.onNodeWithTag("remote-power-label").assertTextEquals("Power")
    composeTestRule.onNodeWithTag("remote-mute-label").assertTextEquals("Mute")
    assertEquals(1, composeTestRule.onAllNodesWithTag("remote-power-progress").fetchSemanticsNodes().size)
    assertEquals(0, composeTestRule.onAllNodesWithTag("remote-mute-progress").fetchSemanticsNodes().size)
  }

  @Test
  fun remoteTab_failedCommandShowsActionLocalError() {
    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(IrStatus(message = "IR remote ready", availableCommands = setOf("power"))),
        remoteControlsState = RemoteControlsState(errorMessage = "IR bridge offline", errorCommand = "power"),
      ),
    )

    composeTestRule.onNodeWithText("Power failed: IR bridge offline").assertExists()
  }

  private fun renderScreen(
    state: MainScreenUiState,
    onSendHomeRemoteCommand: (String) -> Unit = {},
    onSendRemoteCommand: (String) -> Unit = {},
  ) {
    composeTestRule.setContent {
      MainScreenContent(
        state = state,
        onBaseUrlChanged = {},
        onApiKeyChanged = {},
        onSubmitSetup = {},
        onOpenReconfiguration = {},
        onCancelReconfiguration = {},
        onTabSelected = {},
        onRetryIrStatus = {},
        onSendHomeRemoteCommand = onSendHomeRemoteCommand,
        onSendRemoteCommand = onSendRemoteCommand,
      )
    }
  }

  private fun configurationFormState(
    mode: ConfigurationFormMode = ConfigurationFormMode.Setup,
    baseUrl: String = "",
    apiKey: String = "",
  ): MainScreenUiState.ConfigurationForm =
    MainScreenUiState.ConfigurationForm(mode = mode, baseUrl = baseUrl, apiKey = apiKey)
}
