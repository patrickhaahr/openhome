package com.patrickhaahr.openhome.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.setup.SetupService
import com.patrickhaahr.openhome.setup.SetupState
import com.patrickhaahr.openhome.ui.setup.SetupRoute
import com.patrickhaahr.openhome.ui.theme.OpenHomeTheme
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn

class AppGateViewModel(setup: SetupService) : ViewModel() {
    /** Null until storage answers, so a set-up app never flashes the Setup Flow. */
    val state: StateFlow<SetupState?> =
        setup.observeState().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
}

/** The Setup Flow gates the app; once complete, the navigation hierarchy never includes it. */
@Composable
fun OpenHomeApp() {
    OpenHomeTheme {
        val gate = appViewModel { AppGateViewModel(it.setup) }
        val state by gate.state.collectAsStateWithLifecycle()
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            when (val current = state) {
                null -> Box(Modifier.fillMaxSize())
                SetupState.Complete -> MainNavigation()
                is SetupState.Interrupted -> SetupRoute(interruptedBaseUrl = current.baseUrl)
                SetupState.NotStarted -> SetupRoute(interruptedBaseUrl = null)
            }
        }
    }
}
