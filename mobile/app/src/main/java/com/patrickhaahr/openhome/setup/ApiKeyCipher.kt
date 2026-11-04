package com.patrickhaahr.openhome.setup

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.ProviderException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class SealedApiKey(val ciphertext: ByteArray, val iv: ByteArray)

/** Seals the API Key with an AES key that stays inside the Android Keystore. */
class ApiKeyCipher {
    fun seal(apiKey: String): SealedApiKey {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, loadOrCreateKey())
        return SealedApiKey(cipher.doFinal(apiKey.toByteArray(Charsets.UTF_8)), cipher.iv)
    }

    /** Null when the Keystore can no longer open it, such as after a key was invalidated. */
    fun open(sealed: SealedApiKey): String? =
        try {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, loadOrCreateKey(), GCMParameterSpec(TAG_BITS, sealed.iv))
            String(cipher.doFinal(sealed.ciphertext), Charsets.UTF_8)
        } catch (_: GeneralSecurityException) {
            null
        } catch (_: ProviderException) {
            null
        }

    private fun loadOrCreateKey(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "openhome.api_key"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val TAG_BITS = 128
    }
}
