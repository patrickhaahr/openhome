package com.patrickhaahr.openhome.ui.home

import com.patrickhaahr.openhome.api.ApiFailure

/**
 * What one Home control group is doing and what it last learned, as exactly one fact.
 *
 * Each ViewModel is the sole owner of its group's transitions: a request moves [Idle], [Acknowledged] or [Failed]
 * to [Loading] or [Sending], and its single response, or its handled [com.patrickhaahr.openhome.api.ApiFailure],
 * moves it on. Cancelling the ViewModel scope cancels the in-flight call. [Acknowledged] records only that Axum
 * accepted the request; it never says anything about the device. [Failed] means the outcome may be unknown.
 */
sealed interface CommandFeedback {
    /** Nothing requested yet. */
    data object Idle : CommandFeedback

    /** Reading which commands are available. */
    data object Loading : CommandFeedback

    /** A command is in flight. */
    data class Sending(val label: String) : CommandFeedback

    data class Acknowledged(val text: String) : CommandFeedback

    data class Failed(val reason: String) : CommandFeedback

    val busy: Boolean get() = this is Loading || this is Sending

    /** The live status line, if there is one. */
    val message: String?
        get() = when (this) {
            is Sending -> "Sending $label…"
            is Acknowledged -> text
            Idle, Loading, is Failed -> null
        }

    val problem: String? get() = (this as? Failed)?.reason
}

/**
 * Runs one immediate command and settles it as [CommandFeedback.Acknowledged] or, on an [ApiFailure],
 * [CommandFeedback.Failed]. [section] names the device group in the honest "unobserved" and "unknown" wording.
 * Anything that is not an [ApiFailure] is deliberately not handled here and escapes to the caller.
 */
internal suspend fun settleCommand(section: String, label: String, send: suspend () -> Unit): CommandFeedback =
    try {
        send()
        CommandFeedback.Acknowledged("$label acknowledged by Axum. $section state is unobserved.")
    } catch (failure: ApiFailure) {
        CommandFeedback.Failed("$label failed. ${failure.message}. Nothing queued; $section outcome may be unknown.")
    }
