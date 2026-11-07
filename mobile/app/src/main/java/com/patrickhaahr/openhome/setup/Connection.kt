package com.patrickhaahr.openhome.setup

import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

/** Setup input accepted for use against the Axum API. */
data class Connection(val baseUrl: HttpUrl, val apiKey: String)

sealed interface ConnectionInput {
    data class Valid(val connection: Connection) : ConnectionInput

    data class Invalid(val message: String) : ConnectionInput
}

/** Same rules as the Expo client's Setup Flow, so a Base URL that works there works here. */
fun parseConnection(baseUrlInput: String, apiKeyInput: String): ConnectionInput {
    val baseUrlText = normalizeInput(baseUrlInput).trimEnd('/')
    val apiKey = normalizeInput(apiKeyInput)
    if (baseUrlText.isEmpty()) return ConnectionInput.Invalid("Enter a Base URL.")
    if (apiKey.isEmpty()) return ConnectionInput.Invalid("Enter an API Key.")
    if (apiKey.any { it !in '!'..'~' }) {
        return ConnectionInput.Invalid("That API Key contains characters a key can't have.")
    }
    if (!baseUrlText.startsWith("http://", ignoreCase = true) &&
        !baseUrlText.startsWith("https://", ignoreCase = true)
    ) {
        return ConnectionInput.Invalid("Base URL must start with http:// or https://.")
    }
    val baseUrl =
        baseUrlText.toHttpUrlOrNull() ?: return ConnectionInput.Invalid("Base URL must include a valid host.")
    if (baseUrl.encodedQuery != null || baseUrl.encodedFragment != null) {
        return ConnectionInput.Invalid("Base URL must not include a query or fragment.")
    }
    return ConnectionInput.Valid(Connection(baseUrl, apiKey))
}

private fun normalizeInput(value: String): String {
    val trimmed = value.trim()
    return if (trimmed.length >= 2 && trimmed.startsWith('"') && trimmed.endsWith('"')) {
        trimmed.substring(1, trimmed.length - 1).trim()
    } else {
        trimmed
    }
}
