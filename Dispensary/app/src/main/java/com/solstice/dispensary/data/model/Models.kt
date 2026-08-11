package com.solstice.dispensary.data.model

import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

enum class ProductCategory(val label: String) {
    FLOWER("Flower"),
    PREROLL("Pre-rolls"),
    EDIBLE("Edibles"),
    CONCENTRATE("Concentrates"),
    VAPE("Vapes"),
    TOPICAL("Topicals"),
    ACCESSORY("Accessories")
}

enum class StrainType(val label: String) {
    INDICA("Indica"),
    SATIVA("Sativa"),
    HYBRID("Hybrid"),
    CBD("CBD"),
    NONE("—")
}

@Entity(tableName = "products")
data class Product(
    @PrimaryKey val id: String,
    val name: String,
    val brand: String,
    val category: ProductCategory,
    val strainType: StrainType,
    val thcPercent: Double,
    val cbdPercent: Double,
    val price: Double,
    val unitLabel: String,
    val description: String,
    val effects: String,
    val featured: Boolean = false,
    val inStock: Boolean = true,
    val stockQuantity: Int = 0,
    val sku: String = "",
    /** When false, product is draft inventory — not shown on the customer menu. */
    val published: Boolean = true,
    val publishedAt: Long = 0L,
    val publishedBy: String = ""
)

@Entity(tableName = "cart_items")
data class CartItem(
    @PrimaryKey val productId: String,
    val quantity: Int
)

@Entity(tableName = "orders")
data class Order(
    @PrimaryKey val id: String,
    val createdAt: Long,
    val total: Double,
    val itemCount: Int,
    val status: String,
    val pickupName: String,
    val notes: String,
    val customerId: String = "",
    val customerEmail: String = "",
    val pointsEarned: Int = 0
)

@Entity(tableName = "order_lines")
data class OrderLine(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val orderId: String,
    val productId: String,
    val productName: String,
    val unitPrice: Double,
    val quantity: Int
)

@Entity(tableName = "inventory_intakes")
data class InventoryIntake(
    @PrimaryKey val id: String,
    val createdAt: Long,
    val productId: String,
    val productName: String,
    val quantityAdded: Int,
    val source: String,
    val rawLabelText: String,
    val confidence: Float,
    val barcode: String = ""
)

enum class AccountRole(val label: String) {
    CUSTOMER("Customer"),
    STAFF("Staff"),
    ADMIN("Admin");

    val canViewSensitiveInfo: Boolean
        get() = this == ADMIN || this == STAFF

    val canManageStaff: Boolean
        get() = this == ADMIN

    val canManageInventory: Boolean
        get() = this == ADMIN || this == STAFF
}

@Entity(
    tableName = "customers",
    indices = [
        Index(value = ["email"], unique = true),
        Index(value = ["username"])
    ]
)
data class Customer(
    @PrimaryKey val id: String,
    val email: String,
    val username: String = "",
    val passwordHash: String,
    val passwordSalt: String,
    val fullName: String,
    val phone: String = "",
    val dateOfBirth: String = "",
    val createdAt: Long = System.currentTimeMillis(),
    val lastLoginAt: Long = 0L,
    val notes: String = "",
    val marketingOptIn: Boolean = false,
    val role: AccountRole = AccountRole.CUSTOMER,
    val createdByAdminId: String = "",
    val mustChangePassword: Boolean = false,
    val enabled: Boolean = true,
    val emailVerified: Boolean = false,
    /** Loyalty points earned from order spend (1 point per $1 by default). */
    val loyaltyPoints: Int = 0,
    /** Lifetime order total used for points / progress. */
    val lifetimeSpend: Double = 0.0
)

/** Profile without password fields. */
data class CustomerProfile(
    val id: String,
    val email: String,
    val username: String,
    val fullName: String,
    val phone: String,
    val dateOfBirth: String,
    val createdAt: Long,
    val lastLoginAt: Long,
    val notes: String,
    val marketingOptIn: Boolean,
    val role: AccountRole,
    val createdByAdminId: String = "",
    val mustChangePassword: Boolean = false,
    val enabled: Boolean = true,
    val emailVerified: Boolean = false,
    val loyaltyPoints: Int = 0,
    val lifetimeSpend: Double = 0.0
) {
    val isAdminLike: Boolean get() = role.canViewSensitiveInfo
}

fun Customer.toProfile() = CustomerProfile(
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

object LoyaltyPoints {
    /** Points earned per $1.00 of order total (tax included). */
    const val POINTS_PER_DOLLAR = 1

    fun pointsForSpend(amount: Double): Int =
        kotlin.math.floor(amount.coerceAtLeast(0.0) * POINTS_PER_DOLLAR).toInt()

    fun earnLabel(points: Int): String =
        if (points == 1) "1 point" else "$points points"
}

/** Result of issuing an email verification code (plaintext shown once for offline delivery). */
data class EmailCodeIssue(
    val email: String,
    val code: String,
    val expiresAtMs: Long
)

@Entity(
    tableName = "product_requests",
    indices = [Index(value = ["customerId"]), Index(value = ["createdAt"])]
)
data class ProductRequest(
    @PrimaryKey val id: String,
    val createdAt: Long = System.currentTimeMillis(),
    val customerId: String,
    val customerName: String,
    val customerEmail: String,
    val productName: String,
    val notes: String = "",
    val status: String = "Open"
)

data class RequestBoxStats(
    val totalRequests: Int,
    val uniqueRequesters: Int,
    val totalCustomers: Int,
    /** Unique requesters ÷ customer accounts × 100. */
    val requesterToCustomerPercent: Float
) {
    val ratioLabel: String
        get() = if (totalCustomers <= 0) {
            "No customers yet"
        } else {
            String.format("%.0f%% of customers have sent a request", requesterToCustomerPercent)
        }
}

enum class AutoLockTimeout(val label: String, val millis: Long) {
    IMMEDIATE("Immediately", 0L),
    ONE_MINUTE("1 minute", 60_000L),
    FIVE_MINUTES("5 minutes", 5 * 60_000L),
    FIFTEEN_MINUTES("15 minutes", 15 * 60_000L),
    THIRTY_MINUTES("30 minutes", 30 * 60_000L),
    NEVER("Never", -1L)
}

data class SecuritySettings(
    val appLockEnabled: Boolean = false,
    val autoLockTimeout: AutoLockTimeout = AutoLockTimeout.FIVE_MINUTES,
    val hasPin: Boolean = false
)

enum class ThemeMode(val label: String) {
    SYSTEM("System"),
    LIGHT("Light"),
    DARK("Dark")
}

sealed class AuthResult {
    data class Success(val customer: CustomerProfile) : AuthResult()
    data class Error(val message: String) : AuthResult()
}

sealed class OpResult {
    data class Success(val message: String) : OpResult()
    data class Error(val message: String) : OpResult()
}

data class CartLine(
    val product: Product,
    val quantity: Int
) {
    val lineTotal: Double get() = product.price * quantity
}

data class CartSummary(
    val lines: List<CartLine>,
    val subtotal: Double,
    val tax: Double,
    val total: Double,
    val itemCount: Int
)

/** Result of on-device AI label / barcode analysis. */
data class LabelScanResult(
    val rawText: String,
    val barcode: String?,
    val detectedName: String?,
    val detectedBrand: String?,
    val detectedCategory: ProductCategory?,
    val detectedStrain: StrainType?,
    val detectedThc: Double?,
    val detectedCbd: Double?,
    val matchedProduct: Product?,
    val matchConfidence: Float,
    val suggestedQuantity: Int,
    val isNewProduct: Boolean
)
