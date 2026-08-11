package com.nativepure.companion.data

import java.security.MessageDigest
import java.security.SecureRandom

object PasswordHasher {
    private val random = SecureRandom()

    fun newSalt(): String {
        val bytes = ByteArray(16)
        random.nextBytes(bytes)
        return bytes.toHex()
    }

    /** Random unusable password material for sync-imported loyalty profiles. */
    fun randomUnusableSecret(): String {
        val bytes = ByteArray(32)
        random.nextBytes(bytes)
        return bytes.toHex()
    }

    fun hash(password: String, salt: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        var current = "$salt:$password".toByteArray(Charsets.UTF_8)
        repeat(10_000) { current = digest.digest(current) }
        return current.toHex()
    }

    fun matches(password: String, salt: String, expectedHash: String): Boolean {
        val actual = hash(password, salt)
        return MessageDigest.isEqual(
            actual.toByteArray(Charsets.UTF_8),
            expectedHash.toByteArray(Charsets.UTF_8)
        )
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
