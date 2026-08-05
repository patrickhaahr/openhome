package com.example.openhome.ui.main

import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
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

    composeTestRule.onNodeWithText("Set up OpenHome").assertIsDisplayed()
    composeTestRule.onNodeWithText("Base URL").assertIsDisplayed()
    composeTestRule.onNodeWithText("API Key").assertIsDisplayed()
    composeTestRule.onNodeWithText("Validate and continue").assertIsDisplayed()
  }

  @Test
  fun appShell_showsInitialTabs() {
    renderScreen(MainScreenUiState.App())

    composeTestRule.onNodeWithText("Home").assertIsDisplayed()
    composeTestRule.onNodeWithText("Remote").assertIsDisplayed()
    composeTestRule.onNodeWithTag("open-reconfiguration").assertIsDisplayed()
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

    composeTestRule.onNodeWithText("Update configuration").assertIsDisplayed()
    composeTestRule.onNodeWithText("Save configuration").assertIsDisplayed()
    composeTestRule.onNodeWithTag("configuration-cancel").assertIsDisplayed()
    composeTestRule.onNodeWithText("https://openhome.example").assertIsDisplayed()
  }

  @Test
  fun homeTab_showsAllEdifierControls() {
    renderScreen(
      MainScreenUiState.App(
        irState = IrState.Loaded(irStatus(edifierCommands = HOME_REMOTE_CONTROL_COMMANDS)),
      ),
    )

    composeTestRule.onNodeWithText("Edifier controls").assertIsDisplayed()
    HOME_REMOTE_CONTROL_COMMANDS.forEach { command ->
      composeTestRule.onNodeWithTag("home-remote-$command").assertIsDisplayed().assertIsEnabled()
    }
  }

  @Test
  fun homeTab_quickControlClick_callsHandler() {
    var tappedCommand: String? = null

    renderScreen(
      MainScreenUiState.App(
        irState = IrState.Loaded(irStatus(edifierCommands = setOf("bluetooth", "optical"))),
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
        irState = IrState.Loaded(irStatus(edifierCommands = setOf("bluetooth", "optical"))),
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
  fun appShell_swipeLeftMovesFromHomeToRemote() {
    renderSwipeableApp(initialTab = TopLevelTab.Home)

    composeTestRule.onNodeWithTag("top-level-tab-home").assertIsSelected()
    composeTestRule.onNodeWithTag("top-level-tab-remote").assertIsNotSelected()

    composeTestRule.onNodeWithTag("main-tabs-pager").performTouchInput { swipeLeft() }
    composeTestRule.waitForIdle()

    composeTestRule.onNodeWithTag("top-level-tab-remote").assertIsSelected()
    composeTestRule.onNodeWithTag("top-level-tab-home").assertIsNotSelected()
  }

  @Test
  fun appShell_swipeRightMovesFromRemoteToHome() {
    renderSwipeableApp(initialTab = TopLevelTab.Remote)

    composeTestRule.onNodeWithTag("top-level-tab-remote").assertIsSelected()
    composeTestRule.onNodeWithTag("top-level-tab-home").assertIsNotSelected()

    composeTestRule.onNodeWithTag("main-tabs-pager").performTouchInput { swipeRight() }
    composeTestRule.waitForIdle()

    composeTestRule.onNodeWithTag("top-level-tab-home").assertIsSelected()
    composeTestRule.onNodeWithTag("top-level-tab-remote").assertIsNotSelected()
  }

  @Test
  fun remoteTab_showsFullV1RemoteButtonSet() {
    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(irStatus(lgTvCommands = REMOTE_BUTTON_COMMANDS)),
      ),
    )

    composeTestRule.onNodeWithTag("remote-power").assertIsDisplayed()
    composeTestRule.onNodeWithTag("remote-input").assertIsDisplayed()
    composeTestRule.onNodeWithTag("remote-mute").assertIsDisplayed()
    composeTestRule.onNodeWithTag("remote-volume-up").assertIsDisplayed()
    assertEquals(31, composeTestRule.onAllNodesWithTag("remote-button").fetchSemanticsNodes().size)
    assertEquals(
      setOf(
        "power",
        "input",
        "volume-up",
        "volume-down",
        "mute",
        "home",
        "settings",
        "up",
        "down",
        "left",
        "right",
        "ok",
        "back",
        "exit",
        "channel-up",
        "channel-down",
        "guide",
        "info",
        "0",
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "9",
        "hdmi-1",
        "hdmi-2",
        "hdmi-3",
      ),
      REMOTE_BUTTON_COMMANDS,
    )
  }

  @Test
  fun remoteTab_unavailableCommandsRemainVisibleButDisabled() {
    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(irStatus(edifierCommands = setOf("bluetooth"), lgTvCommands = setOf("power", "mute"))),
      ),
    )

    composeTestRule.onNodeWithTag("remote-power").assertIsEnabled()
    composeTestRule.onNodeWithTag("remote-mute").assertIsEnabled()
    composeTestRule.onNodeWithTag("remote-input").assertIsDisplayed().assertIsNotEnabled()
    composeTestRule.onNodeWithTag("remote-volume-down").assertIsDisplayed().assertIsNotEnabled()
    composeTestRule.onNodeWithTag("remote-volume-up").assertIsDisplayed().assertIsNotEnabled()
  }

  @Test
  fun remoteTab_buttonClick_callsHandler() {
    var tappedCommand: String? = null

    renderScreen(
      MainScreenUiState.App(
        selectedTab = TopLevelTab.Remote,
        irState = IrState.Loaded(irStatus(lgTvCommands = REMOTE_BUTTON_COMMANDS)),
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
        irState = IrState.Loaded(irStatus(lgTvCommands = setOf("power", "mute"))),
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
        irState = IrState.Loaded(irStatus(lgTvCommands = setOf("power"))),
        remoteControlsState = RemoteControlsState(errorMessage = "IR bridge offline", errorCommand = "power"),
      ),
    )

    composeTestRule.onNodeWithText("Power failed: IR bridge offline").assertIsDisplayed()
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

  private fun renderSwipeableApp(initialTab: TopLevelTab) {
    composeTestRule.setContent {
      var selectedTab by remember { mutableStateOf(initialTab) }

      MainScreenContent(
        state = MainScreenUiState.App(selectedTab = selectedTab, irState = IrState.Loaded(irStatus(lgTvCommands = REMOTE_BUTTON_COMMANDS))),
        onBaseUrlChanged = {},
        onApiKeyChanged = {},
        onSubmitSetup = {},
        onOpenReconfiguration = {},
        onCancelReconfiguration = {},
        onTabSelected = { selectedTab = it },
        onRetryIrStatus = {},
        onSendHomeRemoteCommand = {},
        onSendRemoteCommand = {},
      )
    }
  }

  private fun configurationFormState(
    mode: ConfigurationFormMode = ConfigurationFormMode.Setup,
    baseUrl: String = "",
    apiKey: String = "",
  ): MainScreenUiState.ConfigurationForm =
    MainScreenUiState.ConfigurationForm(mode = mode, baseUrl = baseUrl, apiKey = apiKey)

  private fun irStatus(
    edifierCommands: Set<String> = emptySet(),
    lgTvCommands: Set<String> = emptySet(),
  ): IrStatus = IrStatus(message = "IR remote ready", edifierCommands = edifierCommands, lgTvCommands = lgTvCommands)
}
