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

/** Available commands and command feedback are independent observations; neither implies the TV's state. */
data class TvState(val commands: Set<String> = emptySet(), val feedback: CommandFeedback = CommandFeedback.Idle)

/**
 * Immediate calls only: no database writes, network policy, background work or retry. Refresh and send share one
 * busy guard, so each transition starts only from a settled state and its response cannot be overtaken by another.
 * Exits: response or [ApiFailure] settles it; ViewModel cancellation cancels the call; the Home client's 5 s call
 * timeout (AppContainer.homeHttpClient, no connection retry) surfaces as [ApiFailure.Unreachable].
 * Only [ApiFailure] is handled: any other exception escapes uncaught (no handler is installed), ends the process, and the relaunch starts Home idle, since nothing here is persisted.
 */
class TvViewModel(private val setup: SetupService, private val client: OkHttpClient) : ViewModel() {
    private val mutableState = MutableStateFlow(TvState())
    val state = mutableState.asStateFlow()

    fun refresh() {
        if (state.value.feedback.busy) return
        mutableState.value = TvState(feedback = CommandFeedback.Loading)
        viewModelScope.launch {
            try {
                mutableState.value = TvState(commands = api().irStatus().remotes.lgtv.toSet())
            } catch (failure: ApiFailure) {
                mutableState.value = TvState(feedback = CommandFeedback.Failed("TV controls unavailable. ${failure.message}. Refresh to try again."))
            }
        }
    }

    fun send(command: String, label: String) {
        val before = state.value
        if (before.feedback.busy || command !in before.commands) return
        mutableState.value = before.copy(feedback = CommandFeedback.Sending(label))
        viewModelScope.launch {
            mutableState.value = before.copy(feedback = settleCommand("TV", label) { api().sendTvCommand(command) })
        }
    }

    private suspend fun api(): OpenHomeApi {
        val connection = setup.connection() ?: throw ApiFailure.Unauthorized()
        return OpenHomeApi(connection.baseUrl, connection.apiKey, client)
    }
}
