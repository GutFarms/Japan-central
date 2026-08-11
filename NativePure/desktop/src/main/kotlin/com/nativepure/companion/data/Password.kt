package com.nativepure.companion.data

import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.PBEKeySpec

/**
 * Password hashing with PBKDF2-HMAC-SHA256 (v2) and legacy SHA-256×10k verify for migration.
 *
 * Stored hash formats:
 * - `pbkdf2$210000$<hex>` (current)
 * - bare hex (legacy iterated SHA-256)
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

    /** One-time bootstrap password for a fresh admin install. */
    fun randomBootstrapPassword(): String {
        val alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789"
        return buildString(14) {
            repeat(14) { append(alphabet[random.nextInt(alphabet.length)]) }
        }
    }

    fun hash(password: String, salt: String): String {
        val derived = pbkdf2(password, salt, PBKDF2_ITERATIONS)
        return "pbkdf2$$PBKDF2_ITERATIONS$${derived.toHex()}"
    }

    /** Fast hash for short-lived email verification codes (not account passwords). */
    fun hashVerificationCode(code: String, salt: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        val bytes = digest.digest("$salt:code:$code".toByteArray(Charsets.UTF_8))
        return "code$${bytes.toHex()}"
    }

    fun matchesVerificationCode(code: String, salt: String, expectedHash: String): Boolean {
        val actual = hashVerificationCode(code, salt)
        return MessageDigest.isEqual(
            actual.toByteArray(Charsets.UTF_8),
            expectedHash.toByteArray(Charsets.UTF_8)
        )
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

    private fun pbkdf2(password: String, salt: String, iterations: Int): ByteArray {
        val spec = PBEKeySpec(
            password.toCharArray(),
            salt.toByteArray(Charsets.UTF_8),
            iterations.coerceAtLeast(50_000),
            PBKDF2_KEY_BITS
        )
        val factory = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
        return factory.generateSecret(spec).encoded
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

object PasswordPolicy {
    const val MIN_LENGTH = 8
    const val MAX_FAILED_ATTEMPTS = 5
    const val LOCKOUT_DURATION_MS = 5 * 60 * 1000L

    fun validatePassword(password: String): String? = when {
        password.length < MIN_LENGTH -> "Password must be at least $MIN_LENGTH characters."
        password.any { it.isWhitespace() } -> "Password cannot contain spaces."
        !password.any { it.isLetter() } -> "Password must include at least one letter."
        !password.any { it.isDigit() } -> "Password must include at least one number."
        else -> null
    }

    val requirementsLabel: String =
        "At least $MIN_LENGTH characters, with a letter and a number"
}
