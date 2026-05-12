package com.example.openhome.data

import junit.framework.TestCase.assertEquals
import org.junit.Test

class DataRepositoryTest {
  @Test
  fun normalizeQuotedApiKey_removesOneWrappingPairOfQuotes() {
    assertEquals("secret", normalizeSetupInput("\"secret\""))
  }

  @Test
  fun normalizeQuotedBaseUrl_removesOneWrappingPairOfQuotes() {
    assertEquals("https://openhome.haahr.me", normalizeSetupInput("\"https://openhome.haahr.me\""))
  }

  @Test
  fun normalizeSetupInput_keepsInnerQuotes() {
    assertEquals("abc\"123", normalizeSetupInput("abc\"123"))
  }
}
