package com.example.openhome.data

import junit.framework.TestCase.assertEquals
import junit.framework.TestCase.assertTrue
import kotlinx.coroutines.test.runTest
import org.junit.Test

class LightRepositoryTest {
  @Test
  fun sendCommand_on_postsToLightsOnEndpoint() = runTest {
    val client = RecordingOpenHomeClient(lightSuccessResponse())
    val repository = DefaultLightRepository(openHomeClient = client)

    val result = repository.sendCommand(LightCommand.On)

    assertTrue(result.isSuccess)
    assertEquals(
      OpenHomeRequest(path = "/api/lights/on", method = "POST"),
      client.requests.single(),
    )
  }

  @Test
  fun sendCommand_off_postsToLightsOffEndpoint() = runTest {
    val client = RecordingOpenHomeClient(lightSuccessResponse())
    val repository = DefaultLightRepository(openHomeClient = client)

    val result = repository.sendCommand(LightCommand.Off)

    assertTrue(result.isSuccess)
    assertEquals("/api/lights/off", client.requests.single().path)
  }

  @Test
  fun sendCommand_withApiError_usesReturnedErrorMessage() = runTest {
    val client =
      RecordingOpenHomeClient(
        Result.success(
          OpenHomeResponse(
            statusCode = 503,
            body = """{"error":"SwitchBot unavailable"}""".encodeToByteArray(),
          ),
        ),
      )
    val repository = DefaultLightRepository(openHomeClient = client)

    val result = repository.sendCommand(LightCommand.On)

    assertTrue(result.isFailure)
    assertEquals("SwitchBot unavailable", result.exceptionOrNull()?.message)
  }
}

private class RecordingOpenHomeClient(private val result: Result<OpenHomeResponse>) : OpenHomeClient {
  val requests = mutableListOf<OpenHomeRequest>()

  override suspend fun execute(request: OpenHomeRequest): Result<OpenHomeResponse> {
    requests += request
    return result
  }
}

private fun lightSuccessResponse(): Result<OpenHomeResponse> =
  Result.success(
    OpenHomeResponse(
      statusCode = 200,
      body = """{"message":"Light switched"}""".encodeToByteArray(),
    ),
  )
