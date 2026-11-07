package com.patrickhaahr.openhome.ui.setup

import android.database.SQLException
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.setup.ConnectionInput
import com.patrickhaahr.openhome.setup.SetupProgress
import com.patrickhaahr.openhome.setup.SetupService
import com.patrickhaahr.openhome.setup.parseConnection
import java.security.GeneralSecurityException
import java.security.ProviderException
import kotlinx.coroutines.launch

private const val API_KEY_PROTECTION_ERROR = "Couldn't protect your API Key on this device. Nothing was saved. Try again."

class SetupViewModel(private val setup: SetupService, initialBaseUrl: String?) : ViewModel() {
    var baseUrl by mutableStateOf(initialBaseUrl.orEmpty())
    var apiKey by mutableStateOf("")
    var progress by mutableStateOf<SetupProgress?>(null)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    fun submit() {
        if (progress != null) return
        val connection =
            when (val input = parseConnection(baseUrl, apiKey)) {
                is ConnectionInput.Invalid -> {
                    error = input.message
                    return
                }
                is ConnectionInput.Valid -> input.connection
            }
        error = null
        progress = SetupProgress.CheckingConnection
        viewModelScope.launch {
            var reachedDownload = false
            try {
                // On success the gate replaces this screen, so the final progress can stay showing until then.
                setup.run(connection) { step ->
                    if (step !is SetupProgress.CheckingConnection) reachedDownload = true
                    progress = step
                }
            } catch (failure: ApiFailure) {
                fail(failure.toMessage(baseUrl = connection.baseUrl.toString(), reachedDownload))
            } catch (_: SQLException) {
                fail("Couldn't save your data on this device. Nothing was saved. Try again.")
            } catch (_: GeneralSecurityException) {
                fail(API_KEY_PROTECTION_ERROR)
            } catch (_: ProviderException) {
                fail(API_KEY_PROTECTION_ERROR)
            }
        }
    }

    private fun fail(message: String) {
        error = message
        progress = null
    }
}

private fun ApiFailure.toMessage(baseUrl: String, reachedDownload: Boolean): String {
    val reason =
        when (this) {
            is ApiFailure.Unreachable -> "Couldn't reach OpenHome at $baseUrl. Check the address and your connection."
            is ApiFailure.Unauthorized -> "OpenHome rejected that API Key."
            is ApiFailure.Rejected ->
                if (status in 500..599) {
                    "OpenHome couldn't answer right now (HTTP $status)."
                } else {
                    "OpenHome refused the request (HTTP $status)."
                }
            is ApiFailure.UnreadableResponse -> "That address didn't answer like an OpenHome API. Check the Base URL."
            is ApiFailure.HistoryCannotBePaged ->
                "OpenHome has too many Workouts on $date to download them all."
        }
    return if (reachedDownload) "Download stopped, so nothing was saved. $reason" else reason
}
