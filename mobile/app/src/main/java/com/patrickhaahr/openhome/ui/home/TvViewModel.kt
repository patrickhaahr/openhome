package com.patrickhaahr.openhome.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.setup.SetupService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

data class TvState(
    val commands: Set<String> = emptySet(),
    val busy: Boolean = false,
    val message: String? = null,
    val problem: String? = null,
)

/** Immediate calls only: no database writes, network policy, background work or retry. */
class TvViewModel(private val setup: SetupService, private val client: OkHttpClient) : ViewModel() {
    private val mutableState = MutableStateFlow(TvState())
    val state = mutableState.asStateFlow()

    fun refresh() {
        if (state.value.busy) return
        mutableState.value = TvState(busy = true)
        viewModelScope.launch {
            try {
                mutableState.value = TvState(commands = api().irStatus().remotes.lgtv.toSet())
            } catch (failure: ApiFailure) {
                mutableState.value = TvState(problem = "TV controls unavailable. ${failure.message}. Refresh to try again.")
            }
        }
    }

    fun send(command: String, label: String) {
        val before = state.value
        if (before.busy || command !in before.commands) return
        mutableState.value = before.copy(busy = true, message = "Sending $label…", problem = null)
        viewModelScope.launch {
            try {
                api().sendTvCommand(command)
                mutableState.value = before.copy(message = "$label acknowledged by Axum. TV state is unobserved.", problem = null)
            } catch (failure: ApiFailure) {
                mutableState.value = before.copy(
                    message = null,
                    problem = "$label failed. ${failure.message}. Nothing queued; TV outcome may be unknown.",
                )
            }
        }
    }

    private suspend fun api(): OpenHomeApi {
        val connection = setup.connection() ?: throw ApiFailure.Unauthorized()
        return OpenHomeApi(connection.baseUrl, connection.apiKey, client)
    }
}
