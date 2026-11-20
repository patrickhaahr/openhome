package com.patrickhaahr.openhome.ui.home

import com.patrickhaahr.openhome.api.ApiFailure

sealed interface CommandFeedback {
    data object Idle : CommandFeedback

    data object Loading : CommandFeedback

    data class Sending(val label: String) : CommandFeedback

    data class Acknowledged(val text: String) : CommandFeedback

    data class Failed(val reason: String) : CommandFeedback

    val busy: Boolean get() = this is Loading || this is Sending

    val message: String?
        get() = when (this) {
            is Sending -> "Sending $label…"
            is Acknowledged -> text
            Idle, Loading, is Failed -> null
        }

    val problem: String? get() = (this as? Failed)?.reason
}

internal suspend fun settleCommand(section: String, label: String, send: suspend () -> Unit): CommandFeedback =
    try {
        send()
        CommandFeedback.Acknowledged("$label acknowledged by Axum. $section state is unobserved.")
    } catch (failure: ApiFailure) {
        CommandFeedback.Failed("$label failed. ${failure.message}. Nothing queued; $section outcome may be unknown.")
    }
