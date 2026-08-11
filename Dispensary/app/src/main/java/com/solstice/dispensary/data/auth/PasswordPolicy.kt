package com.solstice.dispensary.data.auth

object PasswordPolicy {
    const val MIN_LENGTH = 8
    const val MAX_FAILED_ATTEMPTS = 5
    const val LOCKOUT_DURATION_MS = 5 * 60 * 1000L
    const val PIN_MIN_LENGTH = 4
    const val PIN_MAX_LENGTH = 6

    fun validatePassword(password: String): String? {
        return when {
            password.length < MIN_LENGTH ->
                "Password must be at least $MIN_LENGTH characters."
            password.any { it.isWhitespace() } ->
                "Password cannot contain spaces."
            !password.any { it.isLetter() } ->
                "Password must include at least one letter."
            !password.any { it.isDigit() } ->
                "Password must include at least one number."
            else -> null
        }
    }

    fun validatePin(pin: String): String? {
        return when {
            pin.length !in PIN_MIN_LENGTH..PIN_MAX_LENGTH ->
                "PIN must be $PIN_MIN_LENGTH–$PIN_MAX_LENGTH digits."
            !pin.all { it.isDigit() } ->
                "PIN must contain only digits."
            else -> null
        }
    }

    val requirementsLabel: String =
        "At least $MIN_LENGTH characters, with a letter and a number"
}
