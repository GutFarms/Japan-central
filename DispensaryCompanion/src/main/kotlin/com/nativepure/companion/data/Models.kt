package com.nativepure.companion.data

import kotlinx.serialization.Serializable

@Serializable
enum class ProductCategory(val label: String) {
    FLOWER("Flower"),
    PREROLL("Pre-rolls"),
    EDIBLE("Edibles"),
    CONCENTRATE("Concentrates"),
    VAPE("Vapes"),
    TOPICAL("Topicals"),
    ACCESSORY("Accessories")
}

@Serializable
enum class StrainType(val label: String) {
    INDICA("Indica"),
    SATIVA("Sativa"),
    HYBRID("Hybrid"),
    CBD("CBD"),
    NONE("—")
}

@Serializable
enum class ProductSize(val label: String, val grams: Double) {
    GRAM("1g", 1.0),
    EIGHTH("3.5g", 3.5),
    QUARTER("7g", 7.0),
    OUNCE("1oz", 28.0);

    companion object {
        const val UNIT_KEY = "UNIT"
    }
}

object SizePricing {
    fun fromEighth(eighthPrice: Double): Map<ProductSize, Double> {
        val base = eighthPrice.coerceAtLeast(0.0)
        return mapOf(
            ProductSize.GRAM to roundMoney(base / 3.5),
            ProductSize.EIGHTH to roundMoney(base),
            ProductSize.QUARTER to roundMoney(base * 1.85),
            ProductSize.OUNCE to roundMoney(base * 6.5)
        )
    }

    private fun roundMoney(value: Double): Double =
        kotlin.math.round(value * 100.0) / 100.0
}

@Serializable
enum class AccountRole(val label: String) {
    CUSTOMER("Customer"),
    STAFF("Staff"),
    ADMIN("Admin");

    val canViewSensitiveInfo: Boolean get() = this == ADMIN || this == STAFF
    val canManageStaff: Boolean get() = this == ADMIN
    val canManageInventory: Boolean get() = this == ADMIN || this == STAFF
}

@Serializable
data class Product(
    val id: String,
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
    val published: Boolean = true,
    val publishedAt: Long = 0L,
    val publishedBy: String = "",
    val onDeal: Boolean = false,
    val dealPercent: Int = 0,
    val dealLabel: String = "",
    val sizeInventoryEnabled: Boolean = false,
    val stockGram: Int = 0,
    val priceGram: Double = 0.0,
    val stockEighth: Int = 0,
    val priceEighth: Double = 0.0,
    val stockQuarter: Int = 0,
    val priceQuarter: Double = 0.0,
    val stockOunce: Int = 0,
    val priceOunce: Double = 0.0,
    val imagePath: String = ""
) {
    val hasActiveDeal: Boolean
        get() = onDeal && dealPercent > 0

    val effectivePrice: Double
        get() = if (hasActiveDeal) {
            (price * (100 - dealPercent.coerceIn(1, 90)) / 100.0).coerceAtLeast(0.0)
        } else {
            price
        }

    fun displayUnitLabel(): String =
        if (sizeInventoryEnabled) "1g–1oz" else unitLabel

    fun normalizedSizeInventory(): Product {
        if (!sizeInventoryEnabled) return copy(inStock = stockQuantity > 0)
        val total = stockGram + stockEighth + stockQuarter + stockOunce
        val eighth = if (priceEighth > 0) priceEighth else price
        val anyPriced = listOf(priceGram, priceEighth, priceQuarter, priceOunce).any { it > 0 }
        val anyStock = listOf(stockGram, stockEighth, stockQuarter, stockOunce).any { it > 0 }
        return copy(
            stockQuantity = total,
            inStock = anyPriced && anyStock,
            unitLabel = "1g–1oz",
            price = if (eighth > 0) eighth else price
        )
    }
}

@Serializable
data class Customer(
    val id: String,
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
    val emailVerified: Boolean = true,
    val loyaltyPoints: Int = 0,
    val lifetimeSpend: Double = 0.0
)

@Serializable
enum class PaymentMethod(val label: String) {
    CASH("Cash"),
    CARD("Card"),
    OTHER("Other")
}

@Serializable
enum class SaleChannel(val label: String) {
    POS("In-store POS"),
    PICKUP("Pickup order")
}

@Serializable
data class Order(
    val id: String,
    val createdAt: Long,
    val total: Double,
    val itemCount: Int,
    val status: String,
    val pickupName: String,
    val notes: String,
    val customerId: String = "",
    val customerEmail: String = "",
    val pointsEarned: Int = 0,
    val pointsRedeemed: Int = 0,
    val discount: Double = 0.0,
    val paymentMethod: String = "",
    val amountTendered: Double = 0.0,
    val changeDue: Double = 0.0,
    val channel: String = SaleChannel.PICKUP.name,
    val cashierId: String = "",
    val cashierName: String = ""
)

object OrderStatus {
    const val READY = "Ready for pickup"
    const val PICKED_UP = "Picked up"
    const val CANCELLED = "Cancelled"

    val staffActions = listOf(READY, PICKED_UP, CANCELLED)
}

data class PosSaleRequest(
    val customerName: String,
    val paymentMethod: PaymentMethod,
    val amountTendered: Double,
    val notes: String = "",
    val loyaltyCustomerId: String? = null,
    val redeemPoints: Int = 0
)

object LoyaltyPoints {
    const val POINTS_PER_DOLLAR = 1
    const val REDEEM_POINTS_PER_DOLLAR = 100

    fun pointsForSpend(amount: Double): Int =
        kotlin.math.floor(amount.coerceAtLeast(0.0) * POINTS_PER_DOLLAR).toInt()

    fun discountForPoints(points: Int): Double =
        (points.coerceAtLeast(0) / REDEEM_POINTS_PER_DOLLAR).toDouble()

    fun maxRedeemablePoints(availablePoints: Int, grossTotal: Double): Int {
        if (availablePoints < REDEEM_POINTS_PER_DOLLAR || grossTotal <= 0) return 0
        val maxByBalance = availablePoints - (availablePoints % REDEEM_POINTS_PER_DOLLAR)
        val maxByTotal = kotlin.math.floor(grossTotal).toInt() * REDEEM_POINTS_PER_DOLLAR
        return minOf(maxByBalance, maxByTotal).coerceAtLeast(0)
    }

    fun taxOn(subtotal: Double, rate: Double = 0.08): Double = subtotal * rate
}

@Serializable
data class OrderLine(
    val orderId: String,
    val productId: String,
    val productName: String,
    val unitPrice: Double,
    val quantity: Int
)

@Serializable
data class CartItem(
    val productId: String,
    val quantity: Int
)

data class CartLine(val product: Product, val quantity: Int) {
    val unitPrice: Double get() = product.effectivePrice
    val lineTotal: Double get() = unitPrice * quantity
}

data class CartSummary(
    val lines: List<CartLine>,
    val subtotal: Double,
    val tax: Double,
    val total: Double,
    val itemCount: Int
)

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
    val mustChangePassword: Boolean = false,
    val enabled: Boolean = true,
    val emailVerified: Boolean = true,
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
    mustChangePassword = mustChangePassword,
    enabled = enabled,
    emailVerified = emailVerified,
    loyaltyPoints = loyaltyPoints,
    lifetimeSpend = lifetimeSpend
)

data class EmailCodeIssue(
    val email: String,
    val code: String,
    val expiresAtMs: Long,
    val deliveredByMail: Boolean = false,
    val mailError: String? = null
)

sealed class AuthResult {
    data class Success(val customer: CustomerProfile) : AuthResult()
    data class Error(val message: String) : AuthResult()
}

sealed class OpResult {
    data class Success(val message: String) : OpResult()
    data class Error(val message: String) : OpResult()
}

enum class NavSection(val label: String) {
    POS("Register"),
    HOME("Home"),
    MENU("Menu"),
    DEALS("Deals"),
    CART("Cart"),
    ORDERS("Orders"),
    INVENTORY("Stock"),
    CUSTOMERS("Customers"),
    REQUESTS("Requests"),
    STORE("Store"),
    ACCOUNT("Account")
}

@Serializable
data class ProductRequest(
    val id: String,
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
    val requesterToCustomerPercent: Float
)

@Serializable
data class SyncFile(
    val format: String = "nativepure-sync-v1",
    val exportedAt: Long = 0L,
    val products: List<Product> = emptyList(),
    /** Customers without password hashes — credentials never leave the device via sync. */
    val customers: List<SyncCustomer> = emptyList(),
    val orders: List<Order> = emptyList(),
    val orderLines: List<OrderLine> = emptyList()
)

@Serializable
data class PersistedStore(
    val products: List<Product> = emptyList(),
    val customers: List<Customer> = emptyList(),
    val orders: List<Order> = emptyList(),
    val orderLines: List<OrderLine> = emptyList(),
    val cart: List<CartItem> = emptyList(),
    val productRequests: List<ProductRequest> = emptyList(),
    val sessionCustomerId: String? = null,
    val ageVerified: Boolean = false
)
