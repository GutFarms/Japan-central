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
    val publishedBy: String = ""
)

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
    val pointsEarned: Int = 0
)

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
    val lineTotal: Double get() = product.price * quantity
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
    val expiresAtMs: Long
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
    HOME("Home"),
    MENU("Menu"),
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
