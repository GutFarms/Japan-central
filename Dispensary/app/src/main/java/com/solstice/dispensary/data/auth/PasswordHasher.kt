package com.solstice.dispensary.data.auth

import java.security.MessageDigest
import java.security.SecureRandom

object PasswordHasher {
    private val random = SecureRandom()

    fun newSalt(): String {
        val bytes = ByteArray(16)
        random.nextBytes(bytes)
        return bytes.toHex()
    }

    fun hash(password: String, salt: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        val rounds = 10_000
        var current = "$salt:$password".toByteArray(Charsets.UTF_8)
        repeat(rounds) {
            current = digest.digest(current)
        }
        return current.toHex()
    }

    fun matches(password: String, salt: String, expectedHash: String): Boolean {
        return hash(password, salt) == expectedHash
    }

    private fun ByteArray.toHex(): String =
        joinToString("") { "%02x".format(it) }
}
