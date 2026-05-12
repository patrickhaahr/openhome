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
  fun refresh_withSuccessfulResponse_loadsSharedIrStatus() = runTest {
    val client = FakeOpenHomeClient(irStatusResponse(message = "Living room remote ready", commands = listOf("bluetooth", "optical", "mute")))
    val repository = DefaultIrRepository(openHomeClient = client)

    val result = repository.refresh()

    assertTrue(result.isSuccess)
    assertEquals(OpenHomeRequest(path = "/api/ir"), client.requests.single())
    assertEquals(
      IrState.Loaded(IrStatus(message = "Living room remote ready", availableCommands = setOf("bluetooth", "optical", "mute"))),
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

    response.complete(irStatusResponse(message = "Living room remote ready", commands = listOf("bluetooth")))
    advanceUntilIdle()

    assertEquals(IrState.Idle, repository.state.value)
    refreshJob.join()
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

private fun irStatusResponse(message: String, commands: List<String>, statusCode: Int = 200): Result<OpenHomeResponse> {
  val commandsJson = commands.joinToString(separator = ", ") { command -> "\"$command\"" }
  return Result.success(
    OpenHomeResponse(
      statusCode = statusCode,
      body =
        """
        {
          "message": "$message",
          "available_commands": [$commandsJson]
        }
        """.trimIndent().encodeToByteArray(),
    ),
  )
}

private fun errorResponse(message: String, statusCode: Int): Result<OpenHomeResponse> =
  Result.success(OpenHomeResponse(statusCode = statusCode, body = """{"error":"$message"}""".encodeToByteArray()))
