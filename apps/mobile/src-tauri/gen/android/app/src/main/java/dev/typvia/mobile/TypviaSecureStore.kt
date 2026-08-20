package dev.typvia.mobile

import android.content.Context
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.hardware.fingerprint.FingerprintManager
import android.app.KeyguardManager
import android.os.Build
import android.os.CancellationSignal
import android.os.Handler
import android.os.Looper
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.spec.MGF1ParameterSpec
import java.security.spec.X509EncodedKeySpec
import java.util.Arrays
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.OAEPParameterSpec
import javax.crypto.spec.PSource

/**
 * Keystore-backed secure store for the mobile host. Kotlin owns only the
 * platform pieces Rust cannot reach — the Android Keystore key, the
 * wrapped-blob file and the system BiometricPrompt; every vault rule stays
 * in Rust.
 *
 * Two entry classes, decided in Rust and passed in as `gated`
 * (secure_store.rs `needs_user_presence` — the single classification both
 * mobile backends read):
 *
 *  - **Gated** (the vault master-key copy). One Keystore RSA-2048 keypair per
 *    entry. The private key is non-extractable and demands a per-use STRONG
 *    biometric, so decrypting the blob IS the biometric gate — like the iOS
 *    Keychain read presenting the Face ID sheet. The secret is wrapped with
 *    the public key (RSA-OAEP SHA-256, MGF1-SHA1 — the parameter set
 *    AndroidKeyStore decrypts) and stored as a private app file; Keystore
 *    holds keys, never data. Public-key wrapping needs no authentication, so
 *    enabling biometrics is silent (iOS store() parity); a Keystore AES key
 *    was rejected here because setUserAuthenticationRequired gates its
 *    encrypt path too and would force a prompt at enable time. The framework
 *    android.hardware.biometrics.BiometricPrompt (API 28+, equal to minSdk
 *    28) is used; androidx.biometric would add a dependency purely for
 *    backports below our minSdk.
 *  - **Ungated** (the sync device identity and the K_sync generations). A
 *    Keystore AES-256-GCM key with no authentication requirement; the blob
 *    is `iv || ciphertext` in the same private app file layout. These must
 *    be readable while the vault is locked and on a device with no enrolled
 *    biometric, otherwise every sync round would raise a prompt — or fail
 *    outright. The key is still non-extractable and hardware-backed where
 *    the device offers it, so the material never leaves Keystore in the
 *    clear.
 *
 * Threading contract (the Rust half is documented in src/secure_store.rs):
 *  - store/remove/retrieve are invoked over JNI from a Rust blocking thread,
 *    never the main thread (retrieve enforces this defensively).
 *  - retrieve returns PENDING and the outcome is delivered exactly once
 *    through nativeSecureStoreComplete: from the main looper after the
 *    prompt for a gated entry, inline on the calling thread for an ungated
 *    one (no prompt to wait for). Rust drops completions for requests it no
 *    longer waits on, so a stray double-completion is harmless.
 *
 * Red lines: nothing in this file logs; plaintext buffers are wiped with
 * Arrays.fill once they crossed JNI; the blob file holds ciphertext only.
 */
object TypviaSecureStore {
  // Status protocol shared with src/secure_store.rs (android_protocol).
  private const val STATUS_OK = 0
  private const val STATUS_PENDING = 1
  private const val STATUS_NOT_FOUND = 2
  private const val STATUS_UNAVAILABLE = 3
  private const val STATUS_DENIED = 4
  private const val STATUS_BACKEND = 5

  private const val KEYSTORE = "AndroidKeyStore"
  private const val ALIAS_PREFIX = "typvia.securestore."
  private const val BLOB_DIR = "typvia-secure-store"
  private const val WRAP_TRANSFORM = "RSA/ECB/OAEPWithSHA-256AndMGF1Padding"

  /** Ungated scheme: AES-256-GCM with a Keystore-generated random IV. */
  private const val SEAL_TRANSFORM = "AES/GCM/NoPadding"
  private const val SEAL_TAG_BITS = 128
  private const val SEAL_IV_BYTES = 12

  /** RSA-2048 OAEP-SHA256 payload ceiling; the MK copy is 32 bytes. */
  private const val MAX_SECRET_BYTES = 190

  // AndroidKeyStore performs OAEP with an MGF1-SHA1 mask regardless of the
  // main digest; both cipher inits state it explicitly so encrypt (default
  // provider) and decrypt (Keystore) agree.
  private val OAEP =
    OAEPParameterSpec("SHA-256", "MGF1", MGF1ParameterSpec.SHA1, PSource.PSpecified.DEFAULT)

  /**
   * Implemented in Rust (secure_store.rs, Java_dev_typvia_mobile_… export in
   * the app cdylib). Routes a retrieve outcome back to the waiting request;
   * `secret` is non-null only for STATUS_OK.
   */
  @JvmStatic
  private external fun nativeSecureStoreComplete(requestId: Long, status: Int, secret: ByteArray?)

  /** Seals `secret` for `entry`, replacing any previous copy (deterministic
   * re-enable, mirroring the iOS delete-then-add). Never prompts, in either
   * class. `gated` picks the scheme (see the class comment). */
  fun store(context: Context, entry: String, secret: ByteArray, gated: Boolean): Int {
    return try {
      if (!validEntry(entry) || secret.size > MAX_SECRET_BYTES) return STATUS_BACKEND
      // The biometric probe belongs to the gated scheme only: an ungated
      // entry must store on a phone with no lock screen at all.
      if (gated && !available(context)) return STATUS_UNAVAILABLE
      deleteEntry(context, entry)

      val sealed = if (gated) wrapGated(entry, secret) else sealUngated(entry, secret)

      // Atomic blob write: tmp + rename, same discipline as the snapshot file.
      val target = blobFile(context, entry)
      val tmp = File(target.parentFile, target.name + ".tmp")
      tmp.writeBytes(sealed)
      if (!tmp.renameTo(target)) {
        tmp.delete()
        return STATUS_BACKEND
      }
      STATUS_OK
    } catch (e: Exception) {
      // No message escapes: exception text can name key aliases but the
      // caller only needs the class of failure.
      STATUS_BACKEND
    } finally {
      // Rust passed a copy of the secret; wipe this side's buffer.
      Arrays.fill(secret, 0)
    }
  }

  /** Gated scheme: a per-use-biometric Keystore RSA key, wrapped with its
   * public half so enabling the gate never prompts. */
  private fun wrapGated(entry: String, secret: ByteArray): ByteArray {
    val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA, KEYSTORE)
    generator.initialize(
      KeyGenParameterSpec.Builder(ALIAS_PREFIX + entry, KeyProperties.PURPOSE_DECRYPT)
        .setKeySize(2048)
        .setDigests(KeyProperties.DIGEST_SHA256)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_OAEP)
        // The gate itself: a per-use STRONG biometric on every private-key
        // operation. Enrollment changes permanently invalidate the key
        // (platform default) — the user re-enables from the password path.
        .setUserAuthenticationRequired(true)
        .apply {
          if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
          } else {
            // Pre-R spelling of "per-use biometric".
            @Suppress("DEPRECATION")
            setUserAuthenticationValidityDurationSeconds(-1)
          }
        }
        .build()
    )
    val pair = generator.generateKeyPair()

    // Re-materialize the public key for the default provider: wrapping is a
    // plain public-key operation and must not require Keystore involvement.
    val publicKey = KeyFactory.getInstance(KeyProperties.KEY_ALGORITHM_RSA)
      .generatePublic(X509EncodedKeySpec(pair.public.encoded))
    val cipher = Cipher.getInstance(WRAP_TRANSFORM)
    cipher.init(Cipher.ENCRYPT_MODE, publicKey, OAEP)
    return cipher.doFinal(secret)
  }

  /** Ungated scheme: a Keystore AES-256-GCM key with no authentication
   * requirement. Keystore picks the IV (its randomized-encryption default),
   * which is prepended to the ciphertext so the read can rebuild the spec. */
  private fun sealUngated(entry: String, secret: ByteArray): ByteArray {
    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
    generator.init(
      KeyGenParameterSpec.Builder(
        ALIAS_PREFIX + entry,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
      )
        .setKeySize(256)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .build()
    )
    val key = generator.generateKey()
    val cipher = Cipher.getInstance(SEAL_TRANSFORM)
    cipher.init(Cipher.ENCRYPT_MODE, key)
    val iv = cipher.iv
    if (iv.size != SEAL_IV_BYTES) throw IllegalStateException("unexpected iv length")
    return iv + cipher.doFinal(secret)
  }

  /**
   * Starts a read of `entry`. Terminal states return directly;
   * STATUS_PENDING means the outcome will arrive through
   * nativeSecureStoreComplete — after the prompt the main looper renders for
   * a gated entry, inline on this thread for an ungated one. Must not be
   * called on the main thread: the caller blocks on the completion, and a
   * gated read needs the main thread free to render the prompt.
   */
  fun retrieve(context: Context, entry: String, requestId: Long, gated: Boolean): Int {
    if (Looper.myLooper() == Looper.getMainLooper()) return STATUS_BACKEND
    return try {
      if (!validEntry(entry)) return STATUS_BACKEND
      val blobFile = blobFile(context, entry)
      val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
      if (!blobFile.exists() || !keyStore.containsAlias(ALIAS_PREFIX + entry)) {
        return STATUS_NOT_FOUND
      }
      val blob = blobFile.readBytes()
      if (!gated) return openUngated(keyStore, entry, blob, requestId)
      if (!available(context)) return STATUS_UNAVAILABLE

      val privateKey = keyStore.getKey(ALIAS_PREFIX + entry, null) as? PrivateKey
        ?: return STATUS_BACKEND
      val cipher = Cipher.getInstance(WRAP_TRANSFORM)
      // Throws KeyPermanentlyInvalidatedException when biometric enrollment
      // changed since store(); surfaced as DENIED below — honest failure, the
      // password path stays and re-enabling rebuilds the key.
      cipher.init(Cipher.DECRYPT_MODE, privateKey, OAEP)

      Handler(Looper.getMainLooper()).post { showPrompt(context, cipher, blob, requestId) }
      STATUS_PENDING
    } catch (e: android.security.keystore.KeyPermanentlyInvalidatedException) {
      STATUS_DENIED
    } catch (e: Exception) {
      STATUS_BACKEND
    }
  }

  /**
   * Ungated read: no prompt and no main-thread hop. The result still travels
   * through the completion export so both classes share one delivery path;
   * Rust's waiter channel buffers it, so the caller's receive returns at
   * once. A blob whose alias does not hold an AES key is a scheme mismatch
   * and fails honestly rather than answering "nothing stored".
   */
  private fun openUngated(
    keyStore: KeyStore,
    entry: String,
    blob: ByteArray,
    requestId: Long
  ): Int {
    if (blob.size <= SEAL_IV_BYTES) return STATUS_BACKEND
    val key = keyStore.getKey(ALIAS_PREFIX + entry, null) as? SecretKey ?: return STATUS_BACKEND
    val cipher = Cipher.getInstance(SEAL_TRANSFORM)
    cipher.init(
      Cipher.DECRYPT_MODE,
      key,
      GCMParameterSpec(SEAL_TAG_BITS, blob, 0, SEAL_IV_BYTES)
    )
    val plain = cipher.doFinal(blob, SEAL_IV_BYTES, blob.size - SEAL_IV_BYTES)
    complete(requestId, STATUS_OK, plain)
    // Rust copied the bytes inside the native call; wipe ours.
    Arrays.fill(plain, 0)
    return STATUS_PENDING
  }

  /** Deletes the entry's key and blob; missing pieces are not an error
   * (disabling biometrics twice must stay idempotent). */
  fun remove(context: Context, entry: String): Int {
    return try {
      if (!validEntry(entry)) return STATUS_BACKEND
      deleteEntry(context, entry)
      STATUS_OK
    } catch (e: Exception) {
      STATUS_BACKEND
    }
  }

  /** Runs on the main looper: presents the system sheet bound to `cipher`. */
  private fun showPrompt(context: Context, cipher: Cipher, blob: ByteArray, requestId: Long) {
    try {
      val executor = context.mainExecutor
      val prompt = BiometricPrompt.Builder(context)
        .setTitle("Unlock your Typvia vault")
        .setSubtitle("解锁 Typvia 保险库")
        // The in-app master password IS the credential fallback (same as the
        // vault page's own "Use master password"); DENIED routes the UI there.
        .setNegativeButton("Use master password", executor) { _, _ ->
          complete(requestId, STATUS_DENIED, null)
        }
        .build()
      prompt.authenticate(
        BiometricPrompt.CryptoObject(cipher),
        CancellationSignal(),
        executor,
        object : BiometricPrompt.AuthenticationCallback() {
          override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
            try {
              val bound = result.cryptoObject?.cipher
              if (bound == null) {
                complete(requestId, STATUS_BACKEND, null)
                return
              }
              val plain = bound.doFinal(blob)
              complete(requestId, STATUS_OK, plain)
              // Rust copied the bytes inside the native call; wipe ours.
              Arrays.fill(plain, 0)
            } catch (e: Exception) {
              complete(requestId, STATUS_BACKEND, null)
            }
          }

          override fun onAuthenticationError(errorCode: Int, errString: CharSequence?) {
            val status = when (errorCode) {
              BiometricPrompt.BIOMETRIC_ERROR_CANCELED,
              BiometricPrompt.BIOMETRIC_ERROR_USER_CANCELED,
              BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT,
              BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT_PERMANENT,
              BiometricPrompt.BIOMETRIC_ERROR_TIMEOUT -> STATUS_DENIED
              BiometricPrompt.BIOMETRIC_ERROR_NO_BIOMETRICS,
              BiometricPrompt.BIOMETRIC_ERROR_HW_NOT_PRESENT,
              BiometricPrompt.BIOMETRIC_ERROR_HW_UNAVAILABLE -> STATUS_UNAVAILABLE
              else -> STATUS_BACKEND
            }
            complete(requestId, status, null)
          }

          // onAuthenticationFailed (one unrecognized finger) keeps the sheet
          // open; no completion — the OS lets the user retry.
        }
      )
    } catch (e: Exception) {
      complete(requestId, STATUS_BACKEND, null)
    }
  }

  /** A stray second completion for the same request is dropped Rust-side. */
  private fun complete(requestId: Long, status: Int, secret: ByteArray?) {
    try {
      nativeSecureStoreComplete(requestId, status, secret)
    } catch (t: Throwable) {
      // The waiter is gone or the lib is unwinding; nothing useful remains.
    }
  }

  /**
   * The gate can work: a secure lock screen plus an enrolled STRONG
   * biometric. Anything less reports unavailable so the UI keeps the
   * master-password path (UnavailableStore semantics).
   */
  private fun available(context: Context): Boolean {
    val keyguard = context.getSystemService(KeyguardManager::class.java) ?: return false
    if (!keyguard.isDeviceSecure) return false
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      val manager = context.getSystemService(BiometricManager::class.java) ?: return false
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        manager.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG) ==
          BiometricManager.BIOMETRIC_SUCCESS
      } else {
        @Suppress("DEPRECATION")
        manager.canAuthenticate() == BiometricManager.BIOMETRIC_SUCCESS
      }
    } else {
      // API 28: the biometric feature probe predates BiometricManager.
      @Suppress("DEPRECATION")
      val fingerprint = context.getSystemService(FingerprintManager::class.java)
      @Suppress("DEPRECATION")
      (fingerprint != null && fingerprint.isHardwareDetected &&
        fingerprint.hasEnrolledFingerprints())
    }
  }

  private fun deleteEntry(context: Context, entry: String) {
    val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
    if (keyStore.containsAlias(ALIAS_PREFIX + entry)) {
      keyStore.deleteEntry(ALIAS_PREFIX + entry)
    }
    val blob = blobFile(context, entry)
    if (blob.exists()) blob.delete()
  }

  private fun blobFile(context: Context, entry: String): File {
    val dir = File(context.filesDir, BLOB_DIR)
    dir.mkdirs()
    return File(dir, "$entry.bin")
  }

  /** Entry names come from the Rust core (e.g. "vault.mk.biometric"); the
   * filter keeps them safe as file names and key aliases. */
  private fun validEntry(entry: String): Boolean =
    entry.isNotEmpty() && entry.length <= 64 && entry.all {
      it.isLetterOrDigit() || it == '.' || it == '_' || it == '-'
    }
}
