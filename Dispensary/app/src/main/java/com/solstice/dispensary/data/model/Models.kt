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
    val sku: String = ""
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
    val customerEmail: String = ""
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
    val createdByAdminId: String = ""
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
    val createdByAdminId: String = ""
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
    createdByAdminId = createdByAdminId
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
