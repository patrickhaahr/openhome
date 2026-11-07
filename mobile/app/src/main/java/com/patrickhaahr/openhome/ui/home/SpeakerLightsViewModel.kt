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

data class HomeCommandState(
    val busy: Boolean = false,
    val message: String? = null,
    val problem: String? = null,
)

data class SpeakerState(val commands: Set<String> = emptySet(), val feedback: HomeCommandState = HomeCommandState())

/** Immediate Home calls share neither fitness's Wi-Fi policy nor its persistent outbox. */
class SpeakerLightsViewModel(private val setup: SetupService, private val client: OkHttpClient) : ViewModel() {
    private val mutableSpeaker = MutableStateFlow(SpeakerState())
    val speaker = mutableSpeaker.asStateFlow()
    private val mutableLights = MutableStateFlow(HomeCommandState())
    val lights = mutableLights.asStateFlow()

    fun refreshSpeaker() {
        if (speaker.value.feedback.busy) return
        mutableSpeaker.value = SpeakerState(feedback = HomeCommandState(busy = true))
        viewModelScope.launch {
            mutableSpeaker.value = try {
                SpeakerState(commands = api().irStatus().remotes.edifier.toSet())
            } catch (failure: ApiFailure) {
                SpeakerState(feedback = HomeCommandState(problem = "Speaker controls unavailable. ${failure.message}. Refresh to try again."))
            }
        }
    }

    fun sendSpeaker(command: String, label: String) {
        val before = speaker.value
        if (before.feedback.busy || command !in before.commands) return
        mutableSpeaker.value = before.copy(feedback = HomeCommandState(busy = true, message = "Sending $label…"))
        viewModelScope.launch {
            mutableSpeaker.value = before.copy(feedback = commandFeedback("Speaker", label) { api().sendSpeakerCommand(command) })
        }
    }

    fun sendLight(command: LightCommand) {
        if (lights.value.busy) return
        mutableLights.value = HomeCommandState(busy = true, message = "Sending ${command.label}…")
        viewModelScope.launch {
            mutableLights.value = commandFeedback("Lights", command.label) { api().sendLightCommand(command) }
        }
    }

    private suspend fun commandFeedback(section: String, label: String, send: suspend () -> Unit): HomeCommandState =
        try {
            send()
            HomeCommandState(message = "$label acknowledged by Axum. $section state is unobserved.")
        } catch (failure: ApiFailure) {
            HomeCommandState(problem = "$label failed. ${failure.message}. Nothing queued; $section outcome may be unknown.")
        }

    private suspend fun api(): OpenHomeApi {
        val connection = setup.connection() ?: throw ApiFailure.Unauthorized()
        return OpenHomeApi(connection.baseUrl, connection.apiKey, client)
    }
}
