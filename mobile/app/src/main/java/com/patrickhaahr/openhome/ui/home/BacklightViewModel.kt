package com.patrickhaahr.openhome.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.api.BacklightBedtimeUpdate
import com.patrickhaahr.openhome.api.BacklightProfileSource
import com.patrickhaahr.openhome.api.BacklightSelection
import com.patrickhaahr.openhome.api.BacklightStatus
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.setup.SetupService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

sealed interface BacklightOperation {
    data object Idle : BacklightOperation

    data class Pending(val label: String) : BacklightOperation

    data class Saved(val message: String) : BacklightOperation

    data class Failed(val problem: String) : BacklightOperation
}

data class BacklightState(
    val status: BacklightStatus? = null,
    val outdated: Boolean = false,
    val operation: BacklightOperation = BacklightOperation.Idle,
) {
    val pending: String? get() = (operation as? BacklightOperation.Pending)?.label
    val message: String? get() = (operation as? BacklightOperation.Saved)?.message
    val problem: String? get() = (operation as? BacklightOperation.Failed)?.problem
}

class BacklightViewModel(private val setup: SetupService, private val client: OkHttpClient) : ViewModel() {
    private val mutableState = MutableStateFlow(BacklightState())
    val state = mutableState.asStateFlow()

    fun load() = request("Loading Backlight") { backlightStatus() }
    fun refresh() = request("Refreshing Backlight") { refreshBacklight() }
    fun select(selection: BacklightSelection) = request("Sending Backlight selection") { selectBacklight(selection) }
    fun automatic() = request("Returning to Automatic") { automaticBacklight() }

    fun saveBedtime(settings: BacklightBedtimeUpdate) = request(
        "Saving bedtime routine",
        success = "Bedtime routine saved by OpenHome.",
        hint = "Check HH:MM times (00:00–23:59), different Start and End, and red brightness 0–100. Refresh and try again.",
    ) { updateBacklightBedtime(settings) }

    fun saveProfile(source: BacklightProfileSource) = request(
        if (source == BacklightProfileSource.Capture) "Capturing TV Sync profile" else "Recovering TV Sync backup",
        success = if (source == BacklightProfileSource.Capture) "TV Sync profile captured by OpenHome." else "TV Sync backup recovered by OpenHome.",
        hint = if (source == BacklightProfileSource.Capture)
            "Activate TV Sync on the controller before capture and check its connection. Refresh and try again."
        else "Ask the server administrator to check the private backup is readable, valid and belongs to the configured controller. Refresh and try again.",
    ) { saveBacklightProfile(source) }

    private fun request(label: String, success: String? = null, hint: String? = null, call: suspend OpenHomeApi.() -> BacklightStatus) {
        val before = state.value
        if (before.operation is BacklightOperation.Pending) return
        mutableState.value = before.copy(operation = BacklightOperation.Pending(label))
        viewModelScope.launch {
            mutableState.value = try {
                val connection = setup.connection() ?: throw ApiFailure.Unauthorized()
                BacklightState(
                    status = OpenHomeApi(connection.baseUrl, connection.apiKey, client).call(),
                    operation = success?.let(BacklightOperation::Saved) ?: BacklightOperation.Idle,
                )
            } catch (failure: ApiFailure) {
                val detail = if (hint != null) {
                    "${failure.message}. $hint"
                } else if (failure is ApiFailure.Rejected && failure.status == 409) {
                    "TV Sync profile required; open Advanced Backlight for capture/recovery."
                } else "${failure.message}."
                before.copy(
                    outdated = true,
                    operation = BacklightOperation.Failed(
                        "$detail Outcome may be unknown. Nothing queued or retried by Android. Refresh Backlight status before choosing again.",
                    ),
                )
            }
        }
    }
}
