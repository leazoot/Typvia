// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ffi

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import java.security.MessageDigest
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import uniffi.typvia_mobile_ffi.KeyKeeper
import uniffi.typvia_mobile_ffi.KeyKeeperException

/**
 * Where this device keeps key material.
 *
 * Until now it kept none: the bridge's own store had one implementation, the
 * iOS Keychain, and every other platform got a stand-in that answered
 * "unavailable" to everything. On Android that meant the device identity
 * could never be written, which meant sync reported itself unavailable, which
 * meant **no account, no pairing and no round on this platform at all** — a
 * whole half of the product, absent because the floor under it was a stub.
 *
 * What this does: one AES-256-GCM key inside the AndroidKeyStore, which the
 * application can use but **cannot read or export** — it lives in the
 * platform's own key material store, hardware-backed where the device has the
 * hardware. Every entry is sealed under it with a fresh random IV and written
 * into the app's private directory, where the OS's own file encryption is the
 * second lock.
 *
 * What this deliberately does not do: gate on the user's presence. The sync
 * device identity and the key generations have to be readable while the app
 * comes back to the foreground and while the vault is shut — a gate here would
 * put a system prompt in front of every sync round, and would make sync
 * impossible on a phone with no lock screen. The vault's own master key is a
 * different entry with a different rule, and it is not written here today.
 */
class AndroidKeyKeeper(private val directory: File) : KeyKeeper {
    override fun store(entry: String, secret: ByteArray) = sealedOff {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val sealed = cipher.doFinal(secret)
        val file = fileFor(entry)
        file.parentFile?.mkdirs()
        // Written beside and renamed over: a process that dies mid-write must
        // not leave an entry that is half of one key and half of another —
        // that reads as corruption on the next launch, which is the worst way
        // to lose a device identity.
        val pending = File(file.parentFile, "${file.name}.writing")
        try {
            pending.outputStream().use { out ->
                out.write(cipher.iv.size)
                out.write(cipher.iv)
                out.write(sealed)
            }
            if (!pending.renameTo(file)) throw KeyKeeperException.Failed()
        } catch (trouble: Throwable) {
            pending.delete()
            throw trouble
        }
    }

    override fun retrieve(entry: String): ByteArray? = sealedOff {
        val file = fileFor(entry)
        if (!file.exists()) {
            null
        } else {
            val bytes = file.readBytes()
            // A file too short to hold its own IV is not an entry. Saying
            // "nothing is stored" would invite the caller to write a second
            // identity over a damaged first one.
            if (bytes.isEmpty()) throw KeyKeeperException.Failed()
            val ivSize = bytes[0].toInt()
            if (ivSize <= 0 || bytes.size <= 1 + ivSize) throw KeyKeeperException.Failed()
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(
                Cipher.DECRYPT_MODE,
                key(),
                GCMParameterSpec(TAG_BITS, bytes, 1, ivSize),
            )
            cipher.doFinal(bytes, 1 + ivSize, bytes.size - 1 - ivSize)
        }
    }

    /** Deleting what is not there is not a failure: removing twice has to be
     * as safe as removing once. */
    override fun remove(entry: String) = sealedOff {
        val file = fileFor(entry)
        if (file.exists() && !file.delete()) throw KeyKeeperException.Failed()
    }

    /**
     * Runs one act and lets **nothing** out except this store's own three
     * refusals.
     *
     * This is the whole robustness rule of the class, in one place. A key
     * store that throws something the bridge has never heard of does not
     * degrade — it comes back as an internal fault at the call that opened the
     * database, and the app fails to start at all. The layers above already
     * know what to do with "there is no key store here": carry on locally and
     * report sync as unavailable. So every other failure is turned into that
     * answer rather than into a dead app. (A host JVM with no AndroidKeyStore
     * provider takes exactly this path, which is why the shared tests still
     * open a store.)
     */
    private inline fun <T> sealedOff(act: () -> T): T = try {
        act()
    } catch (trouble: KeyKeeperException) {
        throw trouble
    } catch (trouble: Throwable) {
        throw failure(trouble)
    }

    /**
     * The one key everything here is sealed under, made on first use.
     *
     * It never leaves the AndroidKeyStore — this class holds a handle to it,
     * not its bytes, and there is no API that would hand the bytes over.
     */
    private fun key(): SecretKey {
        val store = KeyStore.getInstance(PROVIDER).apply { load(null) }
        (store.getEntry(KEY_NAME, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_NAME,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(KEY_BITS)
                .build(),
        )
        return generator.generateKey()
    }

    private fun fileFor(entry: String): File = File(directory, fileName(entry))

    /**
     * The kind of failure, never the platform's words.
     *
     * A key store's diagnostics are sentences about keys, and this one is
     * crossing into a layer that may write what it is told into a log.
     */
    private fun failure(trouble: Throwable): KeyKeeperException = when (trouble) {
        is android.security.keystore.UserNotAuthenticatedException -> KeyKeeperException.Denied()
        is java.security.KeyStoreException,
        is java.security.NoSuchProviderException,
        is java.security.NoSuchAlgorithmException,
        -> KeyKeeperException.Unavailable()
        else -> KeyKeeperException.Failed()
    }

    internal companion object {
        /**
         * One file per entry, named by the digest of the entry rather than by
         * the entry itself: entry names come from the layer above, and a name
         * is a path here — `../` in one would write outside this directory. A
         * digest cannot contain a separator.
         *
         * It must also be stable: the same entry has to resolve to the same
         * file on the next launch, or this device loses the identity it wrote
         * yesterday and quietly becomes a new one.
         */
        internal fun fileName(entry: String): String =
            MessageDigest.getInstance("SHA-256")
                .digest(entry.toByteArray())
                .joinToString("") { "%02x".format(it) }

        const val PROVIDER = "AndroidKeyStore"
        const val KEY_NAME = "typvia.key-store"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val KEY_BITS = 256
        const val TAG_BITS = 128
    }
}
