package com.solstice.dispensary.data.auth

import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.PBEKeySpec

/**
 * PBKDF2-HMAC-SHA256 password hashing with legacy SHA-256×10k verify for migration.
 */
object PasswordHasher {
    private val random = SecureRandom()
    const val PBKDF2_ITERATIONS = 210_000
    private const val PBKDF2_KEY_BITS = 256
    private const val LEGACY_ROUNDS = 10_000

    fun newSalt(): String {
        val bytes = ByteArray(16)
        random.nextBytes(bytes)
        return bytes.toHex()
    }

    fun randomUnusableSecret(): String {
        val bytes = ByteArray(32)
        random.nextBytes(bytes)
        return bytes.toHex()
    }

    fun hash(password: String, salt: String): String {
        val derived = pbkdf2(password, salt, PBKDF2_ITERATIONS)
        return "pbkdf2$$PBKDF2_ITERATIONS$${derived.toHex()}"
    }

    fun needsRehash(storedHash: String): Boolean =
        !storedHash.startsWith("pbkdf2$")

    fun matches(password: String, salt: String, expectedHash: String): Boolean {
        return when {
            expectedHash.startsWith("pbkdf2$") -> {
                val parts = expectedHash.split('$')
                if (parts.size != 3) return false
                val iterations = parts[1].toIntOrNull() ?: return false
                val expected = parts[2]
                val actual = pbkdf2(password, salt, iterations).toHex()
                MessageDigest.isEqual(
                    actual.toByteArray(Charsets.UTF_8),
                    expected.toByteArray(Charsets.UTF_8)
                )
            }
            else -> {
                val actual = legacyHash(password, salt)
                MessageDigest.isEqual(
                    actual.toByteArray(Charsets.UTF_8),
                    expectedHash.toByteArray(Charsets.UTF_8)
                )
            }
        }
    }

    fun hashVerificationCode(code: String, salt: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        return "code$${digest.digest("$salt:code:$code".toByteArray(Charsets.UTF_8)).toHex()}"
    }

    fun matchesVerificationCode(code: String, salt: String, expectedHash: String): Boolean {
        val actual = hashVerificationCode(code, salt)
        return MessageDigest.isEqual(
            actual.toByteArray(Charsets.UTF_8),
            expectedHash.toByteArray(Charsets.UTF_8)
        )
    }

    private fun pbkdf2(password: String, salt: String, iterations: Int): ByteArray {
        val spec = PBEKeySpec(
            password.toCharArray(),
            salt.toByteArray(Charsets.UTF_8),
            iterations.coerceAtLeast(50_000),
            PBKDF2_KEY_BITS
        )
        return SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
            .generateSecret(spec).encoded
    }

    private fun legacyHash(password: String, salt: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        var current = "$salt:$password".toByteArray(Charsets.UTF_8)
        repeat(LEGACY_ROUNDS) { current = digest.digest(current) }
        return current.toHex()
    }

    private fun ByteArray.toHex(): String =
        joinToString("") { "%02x".format(it) }
}
