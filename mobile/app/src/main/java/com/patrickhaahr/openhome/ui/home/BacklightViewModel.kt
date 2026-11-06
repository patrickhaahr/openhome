package com.patrickhaahr.openhome.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.api.BacklightSelection
import com.patrickhaahr.openhome.api.BacklightStatus
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.setup.SetupService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

data class BacklightState(
    val status: BacklightStatus? = null,
    val pending: String? = null,
    val problem: String? = null,
    val outdated: Boolean = false,
)

/** Ephemeral, explicit requests only: no scheduler, Room writes or retry path. */
class BacklightViewModel(private val setup: SetupService, private val client: OkHttpClient) : ViewModel() {
    private val mutableState = MutableStateFlow(BacklightState())
    val state = mutableState.asStateFlow()

    fun load() = request("Loading Backlight") { backlightStatus() }
    fun refresh() = request("Refreshing Backlight") { refreshBacklight() }
    fun select(selection: BacklightSelection) = request("Sending Backlight selection") { selectBacklight(selection) }
    fun automatic() = request("Returning to Automatic") { automaticBacklight() }

    private fun request(label: String, call: suspend OpenHomeApi.() -> BacklightStatus) {
        val before = state.value
        if (before.pending != null) return
        mutableState.value = before.copy(pending = label, problem = null)
        viewModelScope.launch {
            mutableState.value = try {
                val connection = setup.connection() ?: throw ApiFailure.Unauthorized()
                BacklightState(status = OpenHomeApi(connection.baseUrl, connection.apiKey, client).call())
            } catch (failure: ApiFailure) {
                val detail = if (failure is ApiFailure.Rejected && failure.status == 409) {
                    "TV Sync profile required; use the server's capture/recovery tools."
                } else "${failure.message}."
                before.copy(
                    pending = null,
                    outdated = true,
                    problem = "$detail Outcome may be unknown. Nothing queued or retried by Android. Refresh Backlight status before choosing again.",
                )
            }
        }
    }
}
