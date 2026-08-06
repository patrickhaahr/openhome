package com.example.openhome.data

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.io.IOException

enum class LightCommand(val path: String) {
  On("/api/lights/on"),
  Off("/api/lights/off"),
}

fun interface LightRepository {
  suspend fun sendCommand(command: LightCommand): Result<Unit>
}

class DefaultLightRepository(private val openHomeClient: OpenHomeClient) : LightRepository {
  override suspend fun sendCommand(command: LightCommand): Result<Unit> =
    openHomeClient
      .execute(OpenHomeRequest(path = command.path, method = "POST"))
      .mapCatching { response ->
        if (response.statusCode !in SUCCESS_RESPONSE_CODES) {
          throw IOException(response.body.readErrorMessage() ?: DEFAULT_SEND_ERROR)
        }
      }

  private fun ByteArray.readErrorMessage(): String? =
    runCatching {
      jsonParser.parseToJsonElement(decodeToString()).jsonObject[ERROR_KEY]?.jsonPrimitive?.content?.trim()?.takeIf { it.isNotEmpty() }
    }.getOrNull()

  private companion object {
    val jsonParser = Json { ignoreUnknownKeys = true }
    val SUCCESS_RESPONSE_CODES = 200..299
    const val ERROR_KEY = "error"
    const val DEFAULT_SEND_ERROR = "Couldn't switch the light."
  }
}
