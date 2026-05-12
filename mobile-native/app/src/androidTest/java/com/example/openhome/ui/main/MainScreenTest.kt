package com.example.openhome.ui.main

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test

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

  private fun renderScreen(state: MainScreenUiState) {
    composeTestRule.setContent {
      MainScreenContent(state = state, onBaseUrlChanged = {}, onApiKeyChanged = {}, onSubmitSetup = {}, onTabSelected = {})
    }
  }
}
