// SPIKE (TASK-013): Android Keystore round trip + biometric availability
// probe, executed on the emulator as an instrumented test.

package dev.typvia.spike.ime

import android.hardware.biometrics.BiometricManager
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.spec.GCMParameterSpec
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SecureStorageTest {
    private val alias = "typvia-spike-master-key"

    @Test
    fun keystoreEncryptDecryptRoundTrip() {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        ks.deleteEntry(alias)

        val generator = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(
                alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build())
        generator.generateKey()

        val plaintext = "SPIKE_FAKE_KEY_MATERIAL_0123456789".toByteArray()
        val encryptCipher = Cipher.getInstance("AES/GCM/NoPadding")
        encryptCipher.init(Cipher.ENCRYPT_MODE, ks.getKey(alias, null))
        val ciphertext = encryptCipher.doFinal(plaintext)
        val iv = encryptCipher.iv

        val decryptCipher = Cipher.getInstance("AES/GCM/NoPadding")
        decryptCipher.init(
            Cipher.DECRYPT_MODE, ks.getKey(alias, null), GCMParameterSpec(128, iv))
        val decrypted = decryptCipher.doFinal(ciphertext)

        assertArrayEquals(plaintext, decrypted)
        assertTrue(ciphertext.size > plaintext.size)
        ks.deleteEntry(alias)
        Log.i("SPIKE-secure", "android keystore roundtrip OK (AES-GCM, key non-exportable)")
    }

    @Test
    fun biometricAvailabilityProbe() {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val bm = ctx.getSystemService(BiometricManager::class.java)
        val strong = bm.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG)
        val credential = bm.canAuthenticate(
            BiometricManager.Authenticators.BIOMETRIC_STRONG
                or BiometricManager.Authenticators.DEVICE_CREDENTIAL)
        // Emulator without enrolled biometrics/lock reports NONE_ENROLLED — a
        // capability record, not an assertion target.
        Log.i("SPIKE-secure", "android biometric strong=$strong strongOrCredential=$credential")
    }
}
