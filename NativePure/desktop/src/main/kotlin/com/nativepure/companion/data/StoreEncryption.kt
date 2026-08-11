package com.nativepure.companion.data

import java.io.File
import java.nio.ByteBuffer
import java.security.SecureRandom
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * AES-GCM encryption for the companion on-disk store.
 * Key lives beside the store as `.store-key` (not for sync/export).
 */
object StoreEncryption {
    private const val PREFIX = "NPENC1:"
    private const val KEY_FILE = ".store-key"
    private const val GCM_TAG_BITS = 128
    private const val IV_BYTES = 12
    private val random = SecureRandom()

    fun encryptedStoreFile(): File = File(LocalDataStore.dataDirectory(), "store.enc")

    fun keyFile(): File = File(LocalDataStore.dataDirectory(), KEY_FILE)

    fun isEncryptedPayload(text: String): Boolean = text.startsWith(PREFIX)

    fun encryptToFile(plaintext: String, target: File = encryptedStoreFile()) {
        val key = loadOrCreateKey()
        val iv = ByteArray(IV_BYTES).also { random.nextBytes(it) }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key, GCMParameterSpec(GCM_TAG_BITS, iv))
        val cipherBytes = cipher.doFinal(plaintext.toByteArray(Charsets.UTF_8))
        val packed = ByteBuffer.allocate(iv.size + cipherBytes.size)
            .put(iv)
            .put(cipherBytes)
            .array()
        val encoded = PREFIX + Base64.getEncoder().encodeToString(packed)
        LocalDataStore.writeAtomic(target, encoded)
    }

    fun decryptFromFile(file: File = encryptedStoreFile()): String? {
        if (!file.isFile) return null
        return decrypt(file.readText())
    }

    fun decrypt(payload: String): String? {
        if (!isEncryptedPayload(payload)) return null
        return try {
            val key = loadOrCreateKey()
            val raw = Base64.getDecoder().decode(payload.removePrefix(PREFIX))
            if (raw.size <= IV_BYTES) return null
            val iv = raw.copyOfRange(0, IV_BYTES)
            val cipherBytes = raw.copyOfRange(IV_BYTES, raw.size)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_BITS, iv))
            String(cipher.doFinal(cipherBytes), Charsets.UTF_8)
        } catch (_: Throwable) {
            null
        }
    }

    private fun loadOrCreateKey(): SecretKey {
        val file = keyFile()
        if (file.isFile) {
            val bytes = Base64.getDecoder().decode(file.readText().trim())
            return SecretKeySpec(bytes, "AES")
        }
        val gen = KeyGenerator.getInstance("AES")
        gen.init(256)
        val key = gen.generateKey()
        file.parentFile?.mkdirs()
        LocalDataStore.writeAtomic(file, Base64.getEncoder().encodeToString(key.encoded))
        // Best-effort hide on POSIX
        runCatching {
            file.setReadable(false, false)
            file.setReadable(true, true)
            file.setWritable(false, false)
            file.setWritable(true, true)
        }
        return key
    }
}
