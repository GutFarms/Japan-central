package com.nativepure.companion.data

import kotlinx.serialization.Serializable

/**
 * Helpers for protecting customer personal information in UI, receipts, and exports.
 */
object Privacy {
    /** Idle lock for POS after inactivity (staff must re-enter password). */
    const val IDLE_LOCK_MS = 5 * 60 * 1000L

    fun maskEmail(email: String): String {
        val clean = email.trim()
        if (clean.isBlank()) return ""
        val at = clean.indexOf('@')
        if (at <= 0) return "•••"
        val local = clean.substring(0, at)
        val domain = clean.substring(at + 1)
        val visible = local.take(1)
        return "$visible•••@$domain"
    }

    fun maskPhone(phone: String): String {
        val digits = phone.filter { it.isDigit() }
        if (digits.length < 4) return if (phone.isBlank()) "" else "•••"
        return "•••-•••-${digits.takeLast(4)}"
    }

    fun maskDob(dob: String): String {
        val clean = dob.trim()
        if (clean.isBlank()) return ""
        // Keep year only when ISO-like; otherwise fully mask.
        val year = Regex("""(19|20)\d{2}""").find(clean)?.value
        return if (year != null) "••••-$year" else "••••-••••"
    }

    fun maskName(fullName: String): String {
        val parts = fullName.trim().split(Regex("\\s+")).filter { it.isNotBlank() }
        if (parts.isEmpty()) return "Customer"
        return if (parts.size == 1) {
            parts[0].take(1) + "•••"
        } else {
            parts.first() + " " + parts.last().take(1) + "."
        }
    }

    fun redactedCustomerForBackup(customer: Customer): SyncCustomer =
        customer.toSyncCustomer()
}

@Serializable
data class SyncCustomer(
    val id: String,
    val email: String,
    val username: String = "",
    val fullName: String,
    val phone: String = "",
    val dateOfBirth: String = "",
    val createdAt: Long = 0L,
    val lastLoginAt: Long = 0L,
    val notes: String = "",
    val marketingOptIn: Boolean = false,
    /** Informational only — import never escalates privileges from this field. */
    val role: AccountRole = AccountRole.CUSTOMER,
    val createdByAdminId: String = "",
    val mustChangePassword: Boolean = false,
    val enabled: Boolean = true,
    val emailVerified: Boolean = true,
    val loyaltyPoints: Int = 0,
    val lifetimeSpend: Double = 0.0
)

fun Customer.toSyncCustomer() = SyncCustomer(
    id = id,
    email = email,
    username = username,
    fullName = fullName,
    phone = phone,
    dateOfBirth = dateOfBirth,
    createdAt = createdAt,
    lastLoginAt = lastLoginAt,
    notes = notes,
    marketingOptIn = marketingOptIn,
    role = role,
    createdByAdminId = createdByAdminId,
    mustChangePassword = mustChangePassword,
    enabled = enabled,
    emailVerified = emailVerified,
    loyaltyPoints = loyaltyPoints,
    lifetimeSpend = lifetimeSpend
)
