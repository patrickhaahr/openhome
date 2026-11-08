package com.patrickhaahr.openhome.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.api.LightCommand
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.setup.SetupService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

/** Available commands and command feedback are independent observations; neither implies the speaker's state. */
data class SpeakerState(val commands: Set<String> = emptySet(), val feedback: CommandFeedback = CommandFeedback.Idle)

/**
 * Immediate Home calls share neither fitness's Wi-Fi policy nor its persistent outbox. Speaker (refresh and send)
 * and Lights are two independent groups, each with one busy guard and one transition owner here. Exits: response
 * or [ApiFailure] settles; ViewModel cancellation cancels the call; the Home client's 5 s call timeout
 * (AppContainer.homeHttpClient, no connection retry) surfaces as [ApiFailure.Unreachable]. Nothing is queued or retried.
 * Only [ApiFailure] is handled: any other exception escapes uncaught (no handler is installed), ends the process, and the relaunch starts Home idle, since nothing here is persisted.
 */
class SpeakerLightsViewModel(private val setup: SetupService, private val client: OkHttpClient) : ViewModel() {
    private val mutableSpeaker = MutableStateFlow(SpeakerState())
    val speaker = mutableSpeaker.asStateFlow()
    private val mutableLights = MutableStateFlow<CommandFeedback>(CommandFeedback.Idle)
    val lights = mutableLights.asStateFlow()

    fun refreshSpeaker() {
        if (speaker.value.feedback.busy) return
        mutableSpeaker.value = SpeakerState(feedback = CommandFeedback.Loading)
        viewModelScope.launch {
            mutableSpeaker.value = try {
                SpeakerState(commands = api().irStatus().remotes.edifier.toSet())
            } catch (failure: ApiFailure) {
                SpeakerState(feedback = CommandFeedback.Failed("Speaker controls unavailable. ${failure.message}. Refresh to try again."))
            }
        }
    }

    fun sendSpeaker(command: String, label: String) {
        val before = speaker.value
        if (before.feedback.busy || command !in before.commands) return
        mutableSpeaker.value = before.copy(feedback = CommandFeedback.Sending(label))
        viewModelScope.launch {
            mutableSpeaker.value = before.copy(feedback = settleCommand("Speaker", label) { api().sendSpeakerCommand(command) })
        }
    }

    fun sendLight(command: LightCommand) {
        if (lights.value.busy) return
        mutableLights.value = CommandFeedback.Sending(command.label)
        viewModelScope.launch {
            mutableLights.value = settleCommand("Lights", command.label) { api().sendLightCommand(command) }
        }
    }

    private suspend fun api(): OpenHomeApi {
        val connection = setup.connection() ?: throw ApiFailure.Unauthorized()
        return OpenHomeApi(connection.baseUrl, connection.apiKey, client)
    }
}
