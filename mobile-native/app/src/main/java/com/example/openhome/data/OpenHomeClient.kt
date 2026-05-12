package com.example.openhome.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import java.io.IOException
import java.io.InputStream
import java.net.ConnectException
import java.net.HttpURLConnection
import java.net.SocketTimeoutException
import java.net.URL
import java.net.UnknownHostException

data class OpenHomeRequest(
  val path: String,
  val method: String = "GET",
  val body: ByteArray? = null,
  val contentType: String? = null,
)

data class OpenHomeResponse(val statusCode: Int, val body: ByteArray)

fun interface OpenHomeClient {
  suspend fun execute(request: OpenHomeRequest): Result<OpenHomeResponse>
}

class DefaultOpenHomeClient(
  private val setupRepository: SetupRepository,
  private val connectionFactory: OpenHomeConnectionFactory = DefaultOpenHomeConnectionFactory(),
) : OpenHomeClient {
  override suspend fun execute(request: OpenHomeRequest): Result<OpenHomeResponse> =
    withContext(Dispatchers.IO) {
      runCatching {
        val configuration = setupRepository.configuration.first() ?: throw MissingConfigurationException()
        val connection = connectionFactory.open(URL(configuration.apiUrl(request.path)))
        connection.requestMethod = request.method
        connection.instanceFollowRedirects = false
        connection.connectTimeout = CONNECT_TIMEOUT_MILLIS
        connection.readTimeout = READ_TIMEOUT_MILLIS
        connection.setRequestProperty("Authorization", "Bearer ${configuration.apiKey}")

        request.contentType?.let { connection.setRequestProperty("Content-Type", it) }
        request.body?.let { body ->
          connection.doOutput = true
          connection.outputStream.use { it.write(body) }
        }

        try {
          val statusCode = connection.responseCode
          OpenHomeResponse(statusCode = statusCode, body = connection.readResponseBody(statusCode))
        } finally {
          connection.disconnect()
        }
      }.recoverCatching { throwable ->
        when (throwable) {
          is MissingConfigurationException -> throw throwable
          is UnknownHostException, is ConnectException, is SocketTimeoutException ->
            throw throwable.toReachabilityException()
          is IOException -> throw throwable
          else -> throw throwable.toReachabilityException()
        }
      }
    }

  private fun HttpURLConnection.readResponseBody(statusCode: Int): ByteArray {
    val responseStream =
      if (statusCode in SUCCESS_RESPONSE_CODES) {
        runCatching { inputStream }.getOrNull()
      } else {
        errorStream
      }

    return responseStream?.use { it.readBytes() } ?: ByteArray(0)
  }

  private companion object {
    val SUCCESS_RESPONSE_CODES = 200..299
    const val CONNECT_TIMEOUT_MILLIS = 5_000
    const val READ_TIMEOUT_MILLIS = 5_000
    const val REACHABILITY_ERROR_MESSAGE = "Couldn't reach the Axum API. Check the Base URL and try again."
  }

  private fun Throwable.toReachabilityException(): IOException = IOException(REACHABILITY_ERROR_MESSAGE, this)
}

fun interface OpenHomeConnectionFactory {
  fun open(url: URL): HttpURLConnection
}

class DefaultOpenHomeConnectionFactory : OpenHomeConnectionFactory {
  override fun open(url: URL): HttpURLConnection = url.openConnection() as HttpURLConnection
}

class MissingConfigurationException : IllegalStateException("Enter a Base URL and API Key to continue.")

internal fun StoredConfiguration.apiUrl(path: String): String {
  val normalizedPath = path.trim().removePrefix("/")
  require(normalizedPath.isNotEmpty()) { "API path must not be blank." }
  return "$baseUrl/$normalizedPath"
}
