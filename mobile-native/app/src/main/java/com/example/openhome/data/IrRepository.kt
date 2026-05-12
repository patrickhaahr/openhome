package com.example.openhome.data

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.io.IOException
import java.util.concurrent.atomic.AtomicLong

data class IrStatus(val message: String, val availableCommands: Set<String>)

sealed interface IrState {
  data object Idle : IrState

  data object Loading : IrState

  data class Loaded(val status: IrStatus) : IrState

  data class Error(val message: String) : IrState
}

interface IrRepository {
  val state: StateFlow<IrState>

  suspend fun refresh(): Result<IrStatus>

  suspend fun sendCommand(command: String): Result<Unit>

  fun reset()
}

class DefaultIrRepository(private val openHomeClient: OpenHomeClient) : IrRepository {
  private val stateFlow = MutableStateFlow<IrState>(IrState.Idle)
  private val refreshGeneration = AtomicLong(0)

  override val state: StateFlow<IrState> = stateFlow.asStateFlow()

  override suspend fun refresh(): Result<IrStatus> {
    val generation = refreshGeneration.incrementAndGet()
    updateStateIfCurrent(generation, IrState.Loading)

    val result =
      openHomeClient.execute(OpenHomeRequest(path = IR_STATUS_PATH)).mapCatching { response ->
        response.requireSuccess(DEFAULT_LOAD_ERROR)
        response.toIrStatus()
      }

    result
      .onSuccess { status -> updateStateIfCurrent(generation, IrState.Loaded(status)) }
      .onFailure { throwable ->
        updateStateIfCurrent(generation, IrState.Error(throwable.message ?: DEFAULT_LOAD_ERROR))
      }

    return result
  }

  override suspend fun sendCommand(command: String): Result<Unit> =
    openHomeClient.execute(sendCommandRequest(command))
      .mapCatching { response ->
        response.requireSuccess(DEFAULT_SEND_ERROR)
      }

  override fun reset() {
    refreshGeneration.incrementAndGet()
    stateFlow.value = IrState.Idle
  }

  private fun updateStateIfCurrent(generation: Long, state: IrState) {
    if (refreshGeneration.get() == generation) {
      stateFlow.value = state
    }
  }

  private fun OpenHomeResponse.requireSuccess(defaultErrorMessage: String) {
    if (statusCode !in SUCCESS_RESPONSE_CODES) {
      throw IOException(body.readErrorMessage() ?: defaultErrorMessage)
    }
  }

  private fun OpenHomeResponse.toIrStatus(): IrStatus =
    runCatching {
      val responseJson = jsonParser.parseToJsonElement(body.decodeToString()).jsonObject
      IrStatus(
        message = responseJson[MESSAGE_KEY]?.jsonPrimitive?.content?.trim().orEmpty().ifBlank { DEFAULT_READY_MESSAGE },
        availableCommands = responseJson.readAvailableCommands(),
      )
    }.getOrElse { throwable ->
      throw IOException(DEFAULT_PARSE_ERROR, throwable)
    }

  private fun JsonObject.readAvailableCommands(): Set<String> =
    (this[AVAILABLE_COMMANDS_KEY]?.jsonArray ?: JsonArray(emptyList()))
      .mapNotNull { element -> element.jsonPrimitive.content.trim().takeIf { it.isNotEmpty() } }
      .toSet()

  private fun ByteArray.readErrorMessage(): String? =
    runCatching {
      jsonParser.parseToJsonElement(decodeToString()).jsonObject[ERROR_KEY]?.jsonPrimitive?.content?.trim()?.takeIf { it.isNotEmpty() }
    }.getOrNull()

  private fun sendCommandRequest(command: String) =
    OpenHomeRequest(
      path = IR_SEND_PATH,
      method = "POST",
      body = jsonParser.encodeToString(SendCommandRequest(command = command)).encodeToByteArray(),
      contentType = JSON_CONTENT_TYPE,
    )

  @Serializable
  private data class SendCommandRequest(val command: String)

  private companion object {
    val jsonParser = Json { ignoreUnknownKeys = true }
    val SUCCESS_RESPONSE_CODES = 200..299
    const val IR_STATUS_PATH = "/api/ir"
    const val IR_SEND_PATH = "/api/ir/send"
    const val AVAILABLE_COMMANDS_KEY = "available_commands"
    const val MESSAGE_KEY = "message"
    const val ERROR_KEY = "error"
    const val JSON_CONTENT_TYPE = "application/json"
    const val DEFAULT_READY_MESSAGE = "IR remote ready"
    const val DEFAULT_LOAD_ERROR = "Couldn't load IR status from the Axum API."
    const val DEFAULT_PARSE_ERROR = "Couldn't read IR status from the Axum API."
    const val DEFAULT_SEND_ERROR = "Couldn't send that IR command to the Axum API."
  }
}
