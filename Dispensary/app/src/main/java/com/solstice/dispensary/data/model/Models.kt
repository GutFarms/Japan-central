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

/** Weight sizes offered for flower (and other weight-sold inventory). */
enum class ProductSize(val label: String, val grams: Double) {
    GRAM("1g", 1.0),
    EIGHTH("3.5g", 3.5),
    QUARTER("7g", 7.0),
    OUNCE("1oz", 28.0);

    companion object {
        const val UNIT_KEY = "UNIT"

        fun fromKey(key: String?): ProductSize? =
            entries.firstOrNull { it.name.equals(key, ignoreCase = true) }

        fun cartKey(size: ProductSize?): String = size?.name ?: UNIT_KEY
    }
}

data class SizeOffer(
    val size: ProductSize,
    val price: Double,
    val stock: Int
) {
    val inStock: Boolean get() = stock > 0 && price > 0.0
    val offered: Boolean get() = price > 0.0
}

object SizePricing {
    /** Derive 1g / 3.5g / 7g / oz shelf prices from an eighth (3.5g) base. */
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
    val publishedBy: String = "",
    /** When true and dealPercent > 0, product appears on Deals and sells at a discount. */
    val onDeal: Boolean = false,
    /** Percent off shelf price (1–90). */
    val dealPercent: Int = 0,
    /** Short promo label, e.g. "Happy Hour" or "Weekend special". */
    val dealLabel: String = "",
    /**
     * When true, customers order by weight size (1g / 3.5g / 7g / oz)
     * with per-size price + stock. Legacy [price]/[stockQuantity]/[unitLabel] still used
     * as the default/display baseline (usually the 3.5g eighth).
     */
    val sizeInventoryEnabled: Boolean = false,
    val stockGram: Int = 0,
    val priceGram: Double = 0.0,
    val stockEighth: Int = 0,
    val priceEighth: Double = 0.0,
    val stockQuarter: Int = 0,
    val priceQuarter: Double = 0.0,
    val stockOunce: Int = 0,
    val priceOunce: Double = 0.0,
    /**
     * Relative path under app filesDir (e.g. `product-images/{id}.jpg`),
     * or empty when no staff photo has been captured.
     */
    val imagePath: String = ""
) {
    val hasActiveDeal: Boolean
        get() = onDeal && dealPercent > 0

    /** Price charged to customers (deal applied when active) for the default/unit price. */
    val effectivePrice: Double
        get() = applyDeal(price)

    fun sizeOffer(size: ProductSize): SizeOffer = when (size) {
        ProductSize.GRAM -> SizeOffer(size, priceGram, stockGram)
        ProductSize.EIGHTH -> SizeOffer(size, priceEighth, stockEighth)
        ProductSize.QUARTER -> SizeOffer(size, priceQuarter, stockQuarter)
        ProductSize.OUNCE -> SizeOffer(size, priceOunce, stockOunce)
    }

    /** Sizes staff/customers can choose (priced > $0). */
    fun offeredSizes(): List<SizeOffer> =
        if (!sizeInventoryEnabled) emptyList()
        else ProductSize.entries.map(::sizeOffer).filter { it.offered }

    fun shelfPriceFor(size: ProductSize?): Double =
        if (size != null && sizeInventoryEnabled) sizeOffer(size).price else price

    fun effectivePriceFor(size: ProductSize?): Double = applyDeal(shelfPriceFor(size))

    fun stockFor(size: ProductSize?): Int =
        if (size != null && sizeInventoryEnabled) sizeOffer(size).stock else stockQuantity

    fun displayUnitLabel(size: ProductSize? = null): String = when {
        size != null -> size.label
        sizeInventoryEnabled -> "1g–1oz"
        else -> unitLabel
    }

    fun withSizeStock(size: ProductSize, stock: Int): Product {
        val next = stock.coerceAtLeast(0)
        return when (size) {
            ProductSize.GRAM -> copy(stockGram = next)
            ProductSize.EIGHTH -> copy(stockEighth = next)
            ProductSize.QUARTER -> copy(stockQuarter = next)
            ProductSize.OUNCE -> copy(stockOunce = next)
        }.normalizedSizeInventory()
    }

    /** Sync aggregate stock / inStock / unit label when size inventory is on. */
    fun normalizedSizeInventory(): Product {
        if (!sizeInventoryEnabled) {
            return copy(inStock = stockQuantity > 0)
        }
        val total = stockGram + stockEighth + stockQuarter + stockOunce
        val eighth = if (priceEighth > 0) priceEighth else price
        return copy(
            stockQuantity = total,
            inStock = offeredSizes().any { it.inStock },
            unitLabel = "1g–1oz",
            price = if (eighth > 0) eighth else price
        )
    }

    private fun applyDeal(shelf: Double): Double =
        if (hasActiveDeal) {
            (shelf * (100 - dealPercent.coerceIn(1, 90)) / 100.0).coerceAtLeast(0.0)
        } else {
            shelf
        }
}

@Entity(
    tableName = "cart_items",
    primaryKeys = ["productId", "sizeKey"]
)
data class CartItem(
    val productId: String,
    val sizeKey: String = ProductSize.UNIT_KEY,
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
    val pointsEarned: Int = 0,
    val pointsRedeemed: Int = 0,
    val discount: Double = 0.0
)

object OrderStatus {
    const val READY = "Ready for pickup"
    const val PICKED_UP = "Picked up"
    const val CANCELLED = "Cancelled"

    val staffActions = listOf(READY, PICKED_UP, CANCELLED)
}

@Entity(tableName = "order_lines")
data class OrderLine(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val orderId: String,
    val productId: String,
    val productName: String,
    val unitPrice: Double,
    val quantity: Int,
    val sizeKey: String = ProductSize.UNIT_KEY,
    val sizeLabel: String = ""
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
    /** Points earned per $1.00 of amount paid (after discount). */
    const val POINTS_PER_DOLLAR = 1

    /** Points required to redeem $1.00 off. */
    const val REDEEM_POINTS_PER_DOLLAR = 100

    fun pointsForSpend(amount: Double): Int =
        kotlin.math.floor(amount.coerceAtLeast(0.0) * POINTS_PER_DOLLAR).toInt()

    fun discountForPoints(points: Int): Double =
        (points.coerceAtLeast(0) / REDEEM_POINTS_PER_DOLLAR).toDouble()

    /** Largest redeemable point amount (whole dollars of discount) for this balance/total. */
    fun maxRedeemablePoints(availablePoints: Int, grossTotal: Double): Int {
        if (availablePoints < REDEEM_POINTS_PER_DOLLAR || grossTotal <= 0) return 0
        val maxByBalance = availablePoints - (availablePoints % REDEEM_POINTS_PER_DOLLAR)
        val maxByTotal = kotlin.math.floor(grossTotal).toInt() * REDEEM_POINTS_PER_DOLLAR
        return minOf(maxByBalance, maxByTotal).coerceAtLeast(0)
    }

    fun earnLabel(points: Int): String =
        if (points == 1) "1 point" else "$points points"

    fun taxOn(subtotal: Double, rate: Double = 0.08): Double = subtotal * rate
}

/** Result of issuing an email verification code (plaintext shown once for offline delivery). */
data class EmailCodeIssue(
    val email: String,
    val code: String,
    val expiresAtMs: Long,
    /** True when the Native Pure mail server accepted delivery. */
    val deliveredByMail: Boolean = false,
    val mailError: String? = null
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
    val quantity: Int,
    val size: ProductSize? = null
) {
    val sizeKey: String get() = ProductSize.cartKey(size)
    val unitLabel: String get() = product.displayUnitLabel(size)
    val unitPrice: Double get() = product.effectivePriceFor(size)
    val lineTotal: Double get() = unitPrice * quantity
    val lineKey: String get() = "${product.id}:$sizeKey"
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
