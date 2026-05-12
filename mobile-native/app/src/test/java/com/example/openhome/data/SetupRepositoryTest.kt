package com.example.openhome.data

import android.content.SharedPreferences
import junit.framework.TestCase.assertEquals
import junit.framework.TestCase.assertNull
import junit.framework.TestCase.assertTrue
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Test

class SetupRepositoryTest {
  @Test
  fun validateAndSave_withValidReplacement_persistsNewConfiguration() = runTest {
    val sharedPreferences = InMemorySharedPreferences()
    sharedPreferences.edit().putString(BASE_URL_KEY, PRIMARY_CONFIGURATION.baseUrl).putString(API_KEY_KEY, PRIMARY_CONFIGURATION.apiKey).apply()
    val repository =
      DefaultSetupRepository(sharedPreferences = sharedPreferences, healthCheckClient = HealthCheckClient { Result.success(Unit) })

    val result = repository.validateAndSave("  \"${REPLACEMENT_CONFIGURATION.baseUrl}\"  ", "  \"${REPLACEMENT_CONFIGURATION.apiKey}\"  ")

    assertTrue(result.isSuccess)
    assertEquals(REPLACEMENT_CONFIGURATION, repository.configuration.first())
    assertEquals(REPLACEMENT_CONFIGURATION.baseUrl, sharedPreferences.getString(BASE_URL_KEY, null))
    assertEquals(REPLACEMENT_CONFIGURATION.apiKey, sharedPreferences.getString(API_KEY_KEY, null))
  }

  @Test
  fun validateAndSave_withFailedReplacement_keepsPreviousConfigurationActive() = runTest {
    val sharedPreferences = InMemorySharedPreferences()
    sharedPreferences.edit().putString(BASE_URL_KEY, PRIMARY_CONFIGURATION.baseUrl).putString(API_KEY_KEY, PRIMARY_CONFIGURATION.apiKey).apply()
    val repository =
      DefaultSetupRepository(
        sharedPreferences = sharedPreferences,
        healthCheckClient = HealthCheckClient { Result.failure(IllegalStateException("OpenHome rejected that Base URL or API Key.")) },
      )

    val result = repository.validateAndSave(REPLACEMENT_CONFIGURATION.baseUrl, REPLACEMENT_CONFIGURATION.apiKey)

    assertTrue(result.isFailure)
    assertEquals(PRIMARY_CONFIGURATION, repository.configuration.first())
    assertEquals(PRIMARY_CONFIGURATION.baseUrl, sharedPreferences.getString(BASE_URL_KEY, null))
    assertEquals(PRIMARY_CONFIGURATION.apiKey, sharedPreferences.getString(API_KEY_KEY, null))
  }

  @Test
  fun validateAndSave_withBlankApiKey_failsBeforePersistence() = runTest {
    val sharedPreferences = InMemorySharedPreferences()
    val repository =
      DefaultSetupRepository(sharedPreferences = sharedPreferences, healthCheckClient = HealthCheckClient { Result.success(Unit) })

    val result = repository.validateAndSave("https://openhome.example", "   ")

    assertTrue(result.isFailure)
    assertNull(sharedPreferences.getString(BASE_URL_KEY, null))
    assertNull(sharedPreferences.getString(API_KEY_KEY, null))
  }

  @Test
  fun validateAndSave_whenPersistenceFails_keepsPreviousConfigurationActive() = runTest {
    val sharedPreferences = InMemorySharedPreferences(commitResult = false)
    sharedPreferences.edit().putString(BASE_URL_KEY, PRIMARY_CONFIGURATION.baseUrl).putString(API_KEY_KEY, PRIMARY_CONFIGURATION.apiKey).apply()
    val repository =
      DefaultSetupRepository(sharedPreferences = sharedPreferences, healthCheckClient = HealthCheckClient { Result.success(Unit) })

    val result = repository.validateAndSave(REPLACEMENT_CONFIGURATION.baseUrl, REPLACEMENT_CONFIGURATION.apiKey)

    assertTrue(result.isFailure)
    assertEquals(PRIMARY_CONFIGURATION, repository.configuration.first())
    assertEquals(PRIMARY_CONFIGURATION.baseUrl, sharedPreferences.getString(BASE_URL_KEY, null))
    assertEquals(PRIMARY_CONFIGURATION.apiKey, sharedPreferences.getString(API_KEY_KEY, null))

    val recreatedRepository =
      DefaultSetupRepository(sharedPreferences = sharedPreferences, healthCheckClient = HealthCheckClient { Result.success(Unit) })
    assertEquals(PRIMARY_CONFIGURATION, recreatedRepository.configuration.first())
  }

  @Test
  fun validateAndSave_withQueryInBaseUrl_failsBeforePersistence() = runTest {
    val sharedPreferences = InMemorySharedPreferences()
    val repository =
      DefaultSetupRepository(sharedPreferences = sharedPreferences, healthCheckClient = HealthCheckClient { Result.success(Unit) })

    val result = repository.validateAndSave("https://openhome.example?foo=bar", "secret")

    assertTrue(result.isFailure)
    assertEquals("Base URL must not include a query or fragment.", result.exceptionOrNull()?.message)
    assertNull(sharedPreferences.getString(BASE_URL_KEY, null))
    assertNull(sharedPreferences.getString(API_KEY_KEY, null))
  }
}

private class InMemorySharedPreferences(private val commitResult: Boolean = true) : SharedPreferences {
  private val values = linkedMapOf<String, Any?>()

  override fun getAll(): MutableMap<String, *> = values.toMutableMap()

  override fun getString(key: String?, defValue: String?): String? = values[key] as? String ?: defValue

  override fun getStringSet(key: String?, defValues: MutableSet<String>?): MutableSet<String>? = defValues

  override fun getInt(key: String?, defValue: Int): Int = values[key] as? Int ?: defValue

  override fun getLong(key: String?, defValue: Long): Long = values[key] as? Long ?: defValue

  override fun getFloat(key: String?, defValue: Float): Float = values[key] as? Float ?: defValue

  override fun getBoolean(key: String?, defValue: Boolean): Boolean = values[key] as? Boolean ?: defValue

  override fun contains(key: String?): Boolean = values.containsKey(key)

  override fun edit(): SharedPreferences.Editor = Editor(values, commitResult)

  override fun registerOnSharedPreferenceChangeListener(listener: SharedPreferences.OnSharedPreferenceChangeListener?) = Unit

  override fun unregisterOnSharedPreferenceChangeListener(listener: SharedPreferences.OnSharedPreferenceChangeListener?) = Unit

  private class Editor(
    private val values: MutableMap<String, Any?>,
    private val commitResult: Boolean,
  ) : SharedPreferences.Editor {
    private val pending = linkedMapOf<String, Any?>()
    private var clearAll = false

    override fun putString(key: String?, value: String?): SharedPreferences.Editor = apply {
      pending[key.orEmpty()] = value
    }

    override fun putStringSet(key: String?, values: MutableSet<String>?): SharedPreferences.Editor = apply {
      pending[key.orEmpty()] = values
    }

    override fun putInt(key: String?, value: Int): SharedPreferences.Editor = apply {
      pending[key.orEmpty()] = value
    }

    override fun putLong(key: String?, value: Long): SharedPreferences.Editor = apply {
      pending[key.orEmpty()] = value
    }

    override fun putFloat(key: String?, value: Float): SharedPreferences.Editor = apply {
      pending[key.orEmpty()] = value
    }

    override fun putBoolean(key: String?, value: Boolean): SharedPreferences.Editor = apply {
      pending[key.orEmpty()] = value
    }

    override fun remove(key: String?): SharedPreferences.Editor = apply {
      pending[key.orEmpty()] = null
    }

    override fun clear(): SharedPreferences.Editor = apply {
      clearAll = true
    }

    override fun commit(): Boolean {
      apply()
      return commitResult
    }

    override fun apply() {
      if (clearAll) {
        values.clear()
      }

      pending.forEach { (key, value) ->
        if (value == null) {
          values.remove(key)
        } else {
          values[key] = value
        }
      }
    }
  }
}

private val PRIMARY_CONFIGURATION = StoredConfiguration(baseUrl = "http://192.168.1.20:8000", apiKey = "secret")

private val REPLACEMENT_CONFIGURATION = StoredConfiguration(baseUrl = "https://openhome.example", apiKey = "replacement")
