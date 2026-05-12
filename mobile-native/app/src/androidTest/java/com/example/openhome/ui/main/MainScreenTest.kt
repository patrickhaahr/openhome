package com.example.openhome.ui.main

import androidx.activity.ComponentActivity
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
    renderScreen(MainScreenUiState.Setup())

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

  private fun renderScreen(state: MainScreenUiState, onSendHomeRemoteCommand: (String) -> Unit = {}) {
    composeTestRule.setContent {
      MainScreenContent(
        state = state,
        onBaseUrlChanged = {},
        onApiKeyChanged = {},
        onSubmitSetup = {},
        onTabSelected = {},
        onRetryIrStatus = {},
        onSendHomeRemoteCommand = onSendHomeRemoteCommand,
      )
    }
  }
}
