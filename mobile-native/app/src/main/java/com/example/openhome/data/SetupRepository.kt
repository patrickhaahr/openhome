package com.example.openhome.data

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKeys
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import java.io.IOException
import java.net.ConnectException
import java.net.HttpURLConnection
import java.net.SocketTimeoutException
import java.net.URI
import java.net.URL
import java.net.UnknownHostException

data class StoredConfiguration(val baseUrl: String, val apiKey: String)

interface SetupRepository {
  val configuration: Flow<StoredConfiguration?>

  suspend fun validateAndSave(baseUrl: String, apiKey: String): Result<StoredConfiguration>
}

class DefaultSetupRepository(
  context: Context,
  private val healthCheckClient: HealthCheckClient = DefaultHealthCheckClient(),
) : SetupRepository {
  private val sharedPreferences = createEncryptedSharedPreferences(context)
  private val configurationState = MutableStateFlow(sharedPreferences.readConfiguration())

  override val configuration: Flow<StoredConfiguration?> = configurationState.asStateFlow()

  override suspend fun validateAndSave(baseUrl: String, apiKey: String): Result<StoredConfiguration> {
    val configuration = runCatching { createStoredConfiguration(baseUrl, apiKey) }.getOrElse { return Result.failure(it) }

    return healthCheckClient.validateHealth(configuration).mapCatching {
      saveConfiguration(configuration)
      configurationState.value = configuration
      configuration
    }
  }

  private fun createStoredConfiguration(baseUrl: String, apiKey: String): StoredConfiguration {
    val trimmedBaseUrl = normalizeSetupInput(baseUrl)
    require(trimmedBaseUrl.isNotEmpty()) { "Enter a Base URL." }

    val trimmedApiKey = normalizeSetupInput(apiKey)
    require(trimmedApiKey.isNotEmpty()) { "Enter an API Key." }

    validateBaseUrl(trimmedBaseUrl)
    return StoredConfiguration(baseUrl = trimmedBaseUrl.trimEnd('/'), apiKey = trimmedApiKey)
  }

  private fun validateBaseUrl(baseUrl: String) {
    val parsedBaseUrl = URI(baseUrl)
    require(parsedBaseUrl.scheme == "http" || parsedBaseUrl.scheme == "https") { "Base URL must use http or https." }
    require(!parsedBaseUrl.host.isNullOrBlank()) { "Base URL must include a host." }
  }

  private fun saveConfiguration(configuration: StoredConfiguration) {
    sharedPreferences.edit().apply {
      putString(BASE_URL_KEY, configuration.baseUrl)
      putString(API_KEY_KEY, configuration.apiKey)
      apply()
    }
  }

  private fun createEncryptedSharedPreferences(context: Context): SharedPreferences {
    val masterKeyAlias = MasterKeys.getOrCreate(MasterKeys.AES256_GCM_SPEC)

    return EncryptedSharedPreferences.create(
      PREFERENCES_NAME,
      masterKeyAlias,
      context,
      EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
      EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )
  }

  private fun SharedPreferences.readConfiguration(): StoredConfiguration? {
    val storedBaseUrl = getString(BASE_URL_KEY, null)?.trim().orEmpty()
    val storedApiKey = getString(API_KEY_KEY, null)?.trim().orEmpty()
    if (storedBaseUrl.isBlank() || storedApiKey.isBlank()) {
      return null
    }

    return runCatching { createStoredConfiguration(storedBaseUrl, storedApiKey) }.getOrNull()
  }

  private companion object {
    const val PREFERENCES_NAME = "openhome_configuration"
    const val BASE_URL_KEY = "base_url"
    const val API_KEY_KEY = "api_key"
  }
}

internal fun normalizeSetupInput(value: String): String = value.trim().removeSurrounding("\"").trim()

fun interface HealthCheckClient {
  suspend fun validateHealth(configuration: StoredConfiguration): Result<Unit>
}

class DefaultHealthCheckClient : HealthCheckClient {
  override suspend fun validateHealth(configuration: StoredConfiguration): Result<Unit> =
    withContext(Dispatchers.IO) {
      runCatching {
        val connection = (URL(configuration.healthCheckUrl()).openConnection() as HttpURLConnection)
        connection.requestMethod = "GET"
        connection.instanceFollowRedirects = false
        connection.connectTimeout = CONNECT_TIMEOUT_MILLIS
        connection.readTimeout = READ_TIMEOUT_MILLIS
        connection.setRequestProperty("Authorization", "Bearer ${configuration.apiKey}")

        try {
          val responseCode = connection.responseCode
          if (responseCode !in SUCCESS_RESPONSE_CODES) {
            throw IOException("OpenHome rejected that Base URL or API Key.")
          }
        } finally {
          connection.disconnect()
        }
      }.recoverCatching { throwable ->
        when (throwable) {
          is UnknownHostException, is ConnectException, is SocketTimeoutException ->
            throw IOException("Couldn't reach the Axum API. Check the Base URL and try again.", throwable)
          is IOException -> throw throwable
          else -> throw IOException("Couldn't reach the Axum API. Check the Base URL and try again.", throwable)
        }
      }
    }

  private fun StoredConfiguration.healthCheckUrl(): String = "${baseUrl}/api/health"

  private companion object {
    val SUCCESS_RESPONSE_CODES = 200..299
    const val CONNECT_TIMEOUT_MILLIS = 5_000
    const val READ_TIMEOUT_MILLIS = 5_000
  }
}
