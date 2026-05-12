package com.example.openhome

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.ui.NavDisplay
import com.example.openhome.data.SetupRepository
import com.example.openhome.ui.main.MainScreen

@Composable
fun MainNavigation(setupRepository: SetupRepository) {
  val backStack = rememberNavBackStack(Main)

  NavDisplay(
    backStack = backStack,
    onBack = { backStack.removeLastOrNull() },
    entryProvider =
      entryProvider {
        entry<Main> {
          MainScreen(setupRepository = setupRepository, modifier = Modifier.safeDrawingPadding().padding(16.dp))
        }
      },
  )
}
