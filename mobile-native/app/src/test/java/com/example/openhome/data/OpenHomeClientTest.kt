package com.example.openhome.data

import junit.framework.TestCase.assertEquals
import junit.framework.TestCase.assertNotNull
import junit.framework.TestCase.assertTrue
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URL

class OpenHomeClientTest {
  @Test
  fun execute_withStoredConfiguration_setsBearerAuthorizationAndBaseUrl() = runTest {
    val repository = FakeSetupRepository(initialConfiguration = PRIMARY_CONFIGURATION)
    val connectionFactory = RecordingConnectionFactory(responseCode = 204)
    val client = DefaultOpenHomeClient(setupRepository = repository, connectionFactory = connectionFactory)

    val result = client.execute(OpenHomeRequest(path = "/api/ir"))

    assertTrue(result.isSuccess)
    val connection = connectionFactory.singleConnection()
    assertEquals("http://192.168.1.20:8000/api/ir", connection.url.toString())
    assertEquals("GET", connection.requestMethod)
    assertEquals("Bearer secret", connection.headerValue("Authorization"))
  }

  @Test
  fun execute_readsLatestStoredConfigurationForEveryRequest() = runTest {
    val repository = FakeSetupRepository(initialConfiguration = PRIMARY_CONFIGURATION)
    val connectionFactory = RecordingConnectionFactory(responseCode = 200)
    val client = DefaultOpenHomeClient(setupRepository = repository, connectionFactory = connectionFactory)

    client.execute(OpenHomeRequest(path = "/api/ir"))
    repository.updateConfiguration(REPLACEMENT_CONFIGURATION)
    client.execute(OpenHomeRequest(path = "/api/health"))

    val firstConnection = connectionFactory.connections[0]
    val secondConnection = connectionFactory.connections[1]
    assertEquals("http://192.168.1.20:8000/api/ir", firstConnection.url.toString())
    assertEquals("Bearer secret", firstConnection.headerValue("Authorization"))
    assertEquals("https://openhome.example/api/health", secondConnection.url.toString())
    assertEquals("Bearer replacement", secondConnection.headerValue("Authorization"))
  }

  @Test
  fun execute_withoutStoredConfiguration_returnsFailure() = runTest {
    val client = DefaultOpenHomeClient(setupRepository = FakeSetupRepository(), connectionFactory = RecordingConnectionFactory(responseCode = 200))

    val result = client.execute(OpenHomeRequest(path = "/api/ir"))

    assertTrue(result.isFailure)
    assertNotNull(result.exceptionOrNull())
  }

  @Test
  fun execute_withRequestBody_writesBodyAndContentType() = runTest {
    val repository = FakeSetupRepository(initialConfiguration = PRIMARY_CONFIGURATION)
    val connectionFactory = RecordingConnectionFactory(responseCode = 202)
    val client = DefaultOpenHomeClient(setupRepository = repository, connectionFactory = connectionFactory)

    val result = client.execute(
      OpenHomeRequest(
        path = "/api/ir/send",
        method = "POST",
        body = "{\"command\":\"bluetooth\"}".encodeToByteArray(),
        contentType = "application/json",
      ),
    )

    assertTrue(result.isSuccess)
    val connection = connectionFactory.singleConnection()
    assertEquals("POST", connection.requestMethod)
    assertEquals("application/json", connection.headerValue("Content-Type"))
    assertEquals("{\"command\":\"bluetooth\"}", connection.writtenBody.toString(Charsets.UTF_8.name()))
  }
}

private class FakeSetupRepository(initialConfiguration: StoredConfiguration? = null) : SetupRepository {
  private val configurationState = MutableStateFlow(initialConfiguration)

  override val configuration: Flow<StoredConfiguration?> = configurationState

  override suspend fun validateAndSave(baseUrl: String, apiKey: String): Result<StoredConfiguration> {
    val configuration = StoredConfiguration(baseUrl = baseUrl, apiKey = apiKey)
    configurationState.value = configuration
    return Result.success(configuration)
  }

  fun updateConfiguration(configuration: StoredConfiguration?) {
    configurationState.value = configuration
  }
}

private class RecordingConnectionFactory(private val responseCode: Int) : OpenHomeConnectionFactory {
  val connections = mutableListOf<RecordingHttpURLConnection>()

  override fun open(url: URL): HttpURLConnection {
    return RecordingHttpURLConnection(url, responseCode).also(connections::add)
  }

  fun singleConnection(): RecordingHttpURLConnection = connections.single()
}

private class RecordingHttpURLConnection(url: URL, private val configuredResponseCode: Int) : HttpURLConnection(url) {
  private val headers = linkedMapOf<String, String>()
  val writtenBody = ByteArrayOutputStream()

  override fun disconnect() = Unit

  override fun usingProxy(): Boolean = false

  override fun connect() = Unit

  override fun setRequestProperty(key: String, value: String) {
    headers[key] = value
  }

  override fun getRequestProperty(key: String): String? = headers[key]

  fun headerValue(name: String): String? = headers[name]

  override fun getOutputStream() = writtenBody

  override fun getInputStream(): InputStream = ByteArrayInputStream(byteArrayOf())

  override fun getErrorStream(): InputStream? = null

  override fun getResponseCode(): Int = configuredResponseCode
}

private val PRIMARY_CONFIGURATION = StoredConfiguration(baseUrl = "http://192.168.1.20:8000", apiKey = "secret")

private val REPLACEMENT_CONFIGURATION = StoredConfiguration(baseUrl = "https://openhome.example", apiKey = "replacement")
