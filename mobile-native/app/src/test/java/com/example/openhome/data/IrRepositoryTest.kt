package com.example.openhome.data

import junit.framework.TestCase.assertEquals
import junit.framework.TestCase.assertTrue
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class IrRepositoryTest {
  @Test
  fun refresh_withSuccessfulResponse_loadsDeviceCommandSets() = runTest {
    val client =
      FakeOpenHomeClient(
        irStatusResponse(
          message = "Living room remote ready",
          edifierCommands = listOf("bluetooth", "optical"),
          lgTvCommands = listOf("power", "mute"),
        ),
      )
    val repository = DefaultIrRepository(openHomeClient = client)

    val result = repository.refresh()

    assertTrue(result.isSuccess)
    assertEquals(OpenHomeRequest(path = "/api/ir"), client.requests.single())
    assertEquals(
      IrState.Loaded(
        IrStatus(
          message = "Living room remote ready",
          edifierCommands = setOf("bluetooth", "optical"),
          lgTvCommands = setOf("power", "mute"),
        ),
      ),
      repository.state.value,
    )
  }

  @Test
  fun refresh_withApiError_usesReturnedErrorMessage() = runTest {
    val client = FakeOpenHomeClient(errorResponse(message = "IR service unavailable", statusCode = 503))
    val repository = DefaultIrRepository(openHomeClient = client)

    val result = repository.refresh()

    assertTrue(result.isFailure)
    assertEquals(IrState.Error("IR service unavailable"), repository.state.value)
  }

  @Test
  fun reset_whileRefreshIsInFlight_discardsTheStaleResponse() = runTest {
    val response = CompletableDeferred<Result<OpenHomeResponse>>()
    val client = BlockingOpenHomeClient(response)
    val repository = DefaultIrRepository(openHomeClient = client)

    val refreshJob = launch { repository.refresh() }
    advanceUntilIdle()

    assertEquals(IrState.Loading, repository.state.value)

    repository.reset()
    assertEquals(IrState.Idle, repository.state.value)

    response.complete(irStatusResponse(message = "Living room remote ready", edifierCommands = listOf("bluetooth")))
    advanceUntilIdle()

    assertEquals(IrState.Idle, repository.state.value)
    refreshJob.join()
  }

  @Test
  fun sendCommand_toEdifier_postsJsonToDevicePathWithoutChangingSharedState() = runTest {
    val status =
      IrStatus(
        message = "Living room remote ready",
        edifierCommands = setOf("bluetooth", "optical"),
        lgTvCommands = setOf("power"),
      )
    val client = QueueingOpenHomeClient(mutableListOf(irStatusResponse(status), successResponse()))
    val repository = DefaultIrRepository(openHomeClient = client)

    repository.refresh()
    val result = repository.sendCommand(IrRemote.Edifier, "bluetooth")

    assertTrue(result.isSuccess)
    val request = client.requests[1]
    assertEquals("/api/ir/edifier", request.path)
    assertEquals("POST", request.method)
    assertEquals("application/json", request.contentType)
    assertEquals("{" + "\"command\":\"bluetooth\"}", request.body?.toString(Charsets.UTF_8))
    assertEquals(
      IrState.Loaded(status),
      repository.state.value,
    )
  }

  @Test
  fun sendCommand_toLgTv_usesLgTvPath() = runTest {
    val client = FakeOpenHomeClient(successResponse())
    val repository = DefaultIrRepository(openHomeClient = client)

    val result = repository.sendCommand(IrRemote.LgTv, "power")

    assertTrue(result.isSuccess)
    assertEquals("/api/ir/lgtv", client.requests.single().path)
    assertEquals("{" + "\"command\":\"power\"}", client.requests.single().body?.toString(Charsets.UTF_8))
  }

  @Test
  fun sendCommand_withApiError_keepsSharedStateLoaded() = runTest {
    val readyStatus =
      IrStatus(
        message = "Living room remote ready",
        edifierCommands = setOf("bluetooth", "optical"),
        lgTvCommands = setOf("power"),
      )
    val readyState = IrState.Loaded(readyStatus)
    val client =
      QueueingOpenHomeClient(
        mutableListOf(
          irStatusResponse(readyStatus),
          errorResponse(message = "Unknown command 'party'", statusCode = 404),
        ),
      )
    val repository = DefaultIrRepository(openHomeClient = client)

    repository.refresh()
    val result = repository.sendCommand(IrRemote.Edifier, "party")

    assertTrue(result.isFailure)
    assertEquals("Unknown command 'party'", result.exceptionOrNull()?.message)
    assertEquals(readyState, repository.state.value)
  }
}

private class FakeOpenHomeClient(private val result: Result<OpenHomeResponse>) : OpenHomeClient {
  val requests = mutableListOf<OpenHomeRequest>()

  override suspend fun execute(request: OpenHomeRequest): Result<OpenHomeResponse> {
    requests += request
    return result
  }
}

private class BlockingOpenHomeClient(private val result: CompletableDeferred<Result<OpenHomeResponse>>) : OpenHomeClient {
  val requests = mutableListOf<OpenHomeRequest>()

  override suspend fun execute(request: OpenHomeRequest): Result<OpenHomeResponse> {
    requests += request
    return result.await()
  }
}

private class QueueingOpenHomeClient(private val results: MutableList<Result<OpenHomeResponse>>) : OpenHomeClient {
  val requests = mutableListOf<OpenHomeRequest>()

  override suspend fun execute(request: OpenHomeRequest): Result<OpenHomeResponse> {
    requests += request
    return results.removeFirstOrNull() ?: Result.failure(IllegalStateException("No response queued for ${request.path}"))
  }
}

private fun irStatusResponse(
  message: String,
  edifierCommands: List<String> = emptyList(),
  lgTvCommands: List<String> = emptyList(),
  statusCode: Int = 200,
): Result<OpenHomeResponse> {
  val edifierCommandsJson = edifierCommands.joinToString(separator = ", ") { command -> "\"$command\"" }
  val lgTvCommandsJson = lgTvCommands.joinToString(separator = ", ") { command -> "\"$command\"" }
  return Result.success(
    OpenHomeResponse(
      statusCode = statusCode,
      body =
        """
        {
          "message": "$message",
          "remotes": {
            "edifier": [$edifierCommandsJson],
            "lgtv": [$lgTvCommandsJson]
          }
        }
        """.trimIndent().encodeToByteArray(),
    ),
  )
}

private fun irStatusResponse(status: IrStatus, statusCode: Int = 200): Result<OpenHomeResponse> =
  irStatusResponse(
    message = status.message,
    edifierCommands = status.edifierCommands.toList(),
    lgTvCommands = status.lgTvCommands.toList(),
    statusCode = statusCode,
  )

private fun errorResponse(message: String, statusCode: Int): Result<OpenHomeResponse> =
  Result.success(OpenHomeResponse(statusCode = statusCode, body = """{"error":"$message"}""".encodeToByteArray()))

private fun successResponse(statusCode: Int = 200): Result<OpenHomeResponse> = Result.success(OpenHomeResponse(statusCode = statusCode, body = byteArrayOf()))
