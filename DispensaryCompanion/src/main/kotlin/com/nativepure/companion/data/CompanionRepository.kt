package com.nativepure.companion.data

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap

class CompanionRepository {
    private val json = Json {
        prettyPrint = true
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    private val storeDir = File(System.getProperty("user.home"), ".nativepure-companion")
    private val storeFile = File(storeDir, "store.json")

    private var products: MutableList<Product> = mutableListOf()
    private var customers: MutableList<Customer> = mutableListOf()
    private var orders: MutableList<Order> = mutableListOf()
    private var orderLines: MutableList<OrderLine> = mutableListOf()
    private var cart: MutableList<CartItem> = mutableListOf()
    private var sessionCustomerId: String? = null
    var ageVerified: Boolean = false
        private set

    private val failedLogins = ConcurrentHashMap<String, Int>()
    private val lockoutUntil = ConcurrentHashMap<String, Long>()

    init {
        loadOrSeed()
    }

    fun currentCustomer(): CustomerProfile? =
        sessionCustomerId?.let { id -> customers.find { it.id == id }?.toProfile() }

    fun allProducts(): List<Product> = products.toList()

    fun featuredProducts(): List<Product> = products.filter { it.featured }

    fun cartSummary(): CartSummary {
        val byId = products.associateBy { it.id }
        val lines = cart.mapNotNull { item ->
            byId[item.productId]?.let { CartLine(it, item.quantity) }
        }
        val subtotal = lines.sumOf { it.lineTotal }
        val tax = subtotal * TAX_RATE
        return CartSummary(lines, subtotal, tax, subtotal + tax, lines.sumOf { it.quantity })
    }

    fun visibleOrders(): List<Order> {
        val me = currentCustomer() ?: return emptyList()
        return if (me.role.canViewSensitiveInfo) {
            orders.sortedByDescending { it.createdAt }
        } else {
            orders.filter { it.customerId == me.id }.sortedByDescending { it.createdAt }
        }
    }

    fun orderLinesFor(orderId: String): List<OrderLine> =
        orderLines.filter { it.orderId == orderId }

    fun allCustomers(): List<CustomerProfile> {
        val me = currentCustomer() ?: return emptyList()
        if (!me.role.canViewSensitiveInfo) return emptyList()
        return customers.map { it.toProfile() }.sortedByDescending { it.createdAt }
    }

    fun setAgeVerified() {
        ageVerified = true
        persist()
    }

    fun login(emailOrUsername: String, password: String): AuthResult {
        val raw = emailOrUsername.trim()
        if (raw.isBlank()) return AuthResult.Error("Enter your email or username.")

        val remaining = (lockoutUntil[raw.lowercase()] ?: 0L) - System.currentTimeMillis()
        if (remaining > 0) {
            val minutes = ((remaining + 59_999) / 60_000).coerceAtLeast(1)
            return AuthResult.Error("Too many failed attempts. Try again in $minutes min.")
        }

        val customer = if ("@" in raw) {
            customers.find { it.email.equals(raw, ignoreCase = true) }
        } else {
            customers.find { it.username.equals(raw, ignoreCase = true) && it.username.isNotBlank() }
                ?: customers.find { it.email.equals(raw, ignoreCase = true) }
        }

        if (customer == null) {
            recordFailed(raw)
            return AuthResult.Error("No account found for that email or username.")
        }

        if (!PasswordHasher.matches(password, customer.passwordSalt, customer.passwordHash)) {
            recordFailed(raw)
            val until = lockoutUntil[raw.lowercase()] ?: 0L
            return if (until > System.currentTimeMillis()) {
                AuthResult.Error("Too many failed attempts. Account locked for 5 minutes.")
            } else {
                val left = PasswordPolicy.MAX_FAILED_ATTEMPTS - (failedLogins[raw.lowercase()] ?: 0)
                AuthResult.Error("Incorrect password. ${left.coerceAtLeast(0)} attempts left before lockout.")
            }
        }

        failedLogins.remove(raw.lowercase())
        lockoutUntil.remove(raw.lowercase())
        val updated = customer.copy(lastLoginAt = System.currentTimeMillis())
        replaceCustomer(updated)
        sessionCustomerId = updated.id
        persist()
        return AuthResult.Success(updated.toProfile())
    }

    fun register(
        email: String,
        password: String,
        fullName: String,
        phone: String,
        dateOfBirth: String
    ): AuthResult {
        val cleanEmail = email.trim().lowercase()
        val cleanName = fullName.trim()
        when {
            cleanEmail.isBlank() || "@" !in cleanEmail ->
                return AuthResult.Error("Enter a valid email address.")
            PasswordPolicy.validatePassword(password) != null ->
                return AuthResult.Error(PasswordPolicy.validatePassword(password)!!)
            cleanName.length < 2 ->
                return AuthResult.Error("Enter your full name.")
            customers.any { it.email.equals(cleanEmail, ignoreCase = true) } ->
                return AuthResult.Error("An account with that email already exists.")
        }
        val salt = PasswordHasher.newSalt()
        val now = System.currentTimeMillis()
        val customer = Customer(
            id = "cust-" + UUID.randomUUID().toString().take(8),
            email = cleanEmail,
            passwordHash = PasswordHasher.hash(password, salt),
            passwordSalt = salt,
            fullName = cleanName,
            phone = phone.trim(),
            dateOfBirth = dateOfBirth.trim(),
            createdAt = now,
            lastLoginAt = now,
            role = AccountRole.CUSTOMER
        )
        customers.add(customer)
        sessionCustomerId = customer.id
        persist()
        return AuthResult.Success(customer.toProfile())
    }

    fun logout() {
        sessionCustomerId = null
        cart.clear()
        persist()
    }

    fun changePassword(current: String, newPassword: String, confirm: String): AuthResult {
        val existing = customers.find { it.id == sessionCustomerId }
            ?: return AuthResult.Error("Not signed in.")
        if (!PasswordHasher.matches(current, existing.passwordSalt, existing.passwordHash)) {
            return AuthResult.Error("Current password is incorrect.")
        }
        PasswordPolicy.validatePassword(newPassword)?.let { return AuthResult.Error(it) }
        if (newPassword != confirm) return AuthResult.Error("New passwords do not match.")
        if (current == newPassword) return AuthResult.Error("Choose a different password.")
        val salt = PasswordHasher.newSalt()
        val updated = existing.copy(
            passwordHash = PasswordHasher.hash(newPassword, salt),
            passwordSalt = salt,
            mustChangePassword = false
        )
        replaceCustomer(updated)
        persist()
        return AuthResult.Success(updated.toProfile())
    }

    fun forceChangePassword(newPassword: String, confirm: String): AuthResult {
        val existing = customers.find { it.id == sessionCustomerId }
            ?: return AuthResult.Error("Not signed in.")
        if (!existing.mustChangePassword) return AuthResult.Error("Password change is not required.")
        PasswordPolicy.validatePassword(newPassword)?.let { return AuthResult.Error(it) }
        if (newPassword != confirm) return AuthResult.Error("New passwords do not match.")
        val salt = PasswordHasher.newSalt()
        val updated = existing.copy(
            passwordHash = PasswordHasher.hash(newPassword, salt),
            passwordSalt = salt,
            mustChangePassword = false
        )
        replaceCustomer(updated)
        persist()
        return AuthResult.Success(updated.toProfile())
    }

    fun updateProfile(
        fullName: String,
        phone: String,
        dateOfBirth: String,
        notes: String,
        marketingOptIn: Boolean
    ): AuthResult {
        val existing = customers.find { it.id == sessionCustomerId }
            ?: return AuthResult.Error("Not signed in.")
        val updated = existing.copy(
            fullName = fullName.trim().ifBlank { existing.fullName },
            phone = phone.trim(),
            dateOfBirth = dateOfBirth.trim(),
            notes = notes.trim(),
            marketingOptIn = marketingOptIn
        )
        replaceCustomer(updated)
        persist()
        return AuthResult.Success(updated.toProfile())
    }

    fun createStaff(email: String, password: String, fullName: String): AuthResult {
        val admin = currentCustomer() ?: return AuthResult.Error("Not signed in.")
        if (!admin.role.canManageStaff) return AuthResult.Error("Only the main admin can create staff.")
        val cleanEmail = email.trim().lowercase()
        PasswordPolicy.validatePassword(password)?.let { return AuthResult.Error(it) }
        if (cleanEmail.isBlank() || "@" !in cleanEmail) return AuthResult.Error("Enter a valid staff email.")
        if (customers.any { it.email.equals(cleanEmail, true) }) {
            return AuthResult.Error("An account with that email already exists.")
        }
        val salt = PasswordHasher.newSalt()
        val staff = Customer(
            id = "staff-" + UUID.randomUUID().toString().take(8),
            email = cleanEmail,
            passwordHash = PasswordHasher.hash(password, salt),
            passwordSalt = salt,
            fullName = fullName.trim().ifBlank { cleanEmail.substringBefore("@") },
            role = AccountRole.STAFF,
            createdByAdminId = admin.id,
            mustChangePassword = true,
            notes = "Staff sub-account created by ${admin.email}"
        )
        customers.add(staff)
        persist()
        return AuthResult.Success(staff.toProfile())
    }

    fun addToCart(productId: String, quantity: Int = 1) {
        val existing = cart.find { it.productId == productId }
        if (existing == null) {
            cart.add(CartItem(productId, quantity.coerceAtLeast(1)))
        } else {
            cart.replaceAll {
                if (it.productId == productId) it.copy(quantity = it.quantity + quantity) else it
            }
        }
        persist()
    }

    fun setCartQuantity(productId: String, quantity: Int) {
        cart.removeAll { it.productId == productId }
        if (quantity > 0) {
            cart.add(CartItem(productId, quantity))
        }
        persist()
    }

    fun clearCart() {
        cart.clear()
        persist()
    }

    fun adjustStock(productId: String, delta: Int): Boolean {
        val me = currentCustomer() ?: return false
        if (!me.role.canManageInventory) return false
        val idx = products.indexOfFirst { it.id == productId }
        if (idx < 0) return false
        val product = products[idx]
        val next = (product.stockQuantity + delta).coerceAtLeast(0)
        products[idx] = product.copy(stockQuantity = next, inStock = next > 0)
        persist()
        return true
    }

    fun placePickupOrder(pickupName: String, notes: String): Order? {
        val summary = cartSummary()
        if (summary.lines.isEmpty()) return null
        val customer = currentCustomer()
        val order = Order(
            id = UUID.randomUUID().toString().take(8).uppercase(),
            createdAt = System.currentTimeMillis(),
            total = summary.total,
            itemCount = summary.itemCount,
            status = "Ready for pickup",
            pickupName = pickupName.ifBlank { customer?.fullName ?: "Guest" },
            notes = notes.trim(),
            customerId = customer?.id.orEmpty(),
            customerEmail = customer?.email.orEmpty()
        )
        val lines = summary.lines.map {
            OrderLine(order.id, it.product.id, it.product.name, it.product.price, it.quantity)
        }
        orders.add(0, order)
        orderLines.addAll(lines)
        summary.lines.forEach { line ->
            adjustStockInternal(line.product.id, -line.quantity)
        }
        cart.clear()
        persist()
        return order
    }

    private fun adjustStockInternal(productId: String, delta: Int) {
        val idx = products.indexOfFirst { it.id == productId }
        if (idx < 0) return
        val product = products[idx]
        val next = (product.stockQuantity + delta).coerceAtLeast(0)
        products[idx] = product.copy(stockQuantity = next, inStock = next > 0)
    }

    private fun recordFailed(raw: String) {
        val key = raw.lowercase()
        val count = (failedLogins[key] ?: 0) + 1
        if (count >= PasswordPolicy.MAX_FAILED_ATTEMPTS) {
            lockoutUntil[key] = System.currentTimeMillis() + PasswordPolicy.LOCKOUT_DURATION_MS
            failedLogins[key] = 0
        } else {
            failedLogins[key] = count
        }
    }

    private fun replaceCustomer(updated: Customer) {
        val idx = customers.indexOfFirst { it.id == updated.id }
        if (idx >= 0) customers[idx] = updated else customers.add(updated)
    }

    private fun loadOrSeed() {
        if (storeFile.exists()) {
            runCatching {
                val data = json.decodeFromString<PersistedStore>(storeFile.readText())
                products = data.products.toMutableList()
                customers = data.customers.toMutableList()
                orders = data.orders.toMutableList()
                orderLines = data.orderLines.toMutableList()
                cart = data.cart.toMutableList()
                sessionCustomerId = data.sessionCustomerId
                ageVerified = data.ageVerified
            }.onFailure { seedFresh() }
            if (products.isEmpty()) products = SeedCatalog.products.toMutableList()
            ensureMainAdmin()
            ensureDemoCustomer()
            persist()
        } else {
            seedFresh()
        }
    }

    private fun seedFresh() {
        storeDir.mkdirs()
        products = SeedCatalog.products.toMutableList()
        customers = mutableListOf()
        orders = mutableListOf()
        orderLines = mutableListOf()
        cart = mutableListOf()
        sessionCustomerId = null
        ageVerified = false
        ensureMainAdmin()
        ensureDemoCustomer()
        persist()
    }

    private fun ensureMainAdmin() {
        if (customers.any { it.email.equals(MAIN_ADMIN_EMAIL, true) || it.username == MAIN_ADMIN_USERNAME }) {
            return
        }
        val salt = PasswordHasher.newSalt()
        customers.add(
            Customer(
                id = MAIN_ADMIN_ID,
                email = MAIN_ADMIN_EMAIL,
                username = MAIN_ADMIN_USERNAME,
                passwordHash = PasswordHasher.hash(MAIN_ADMIN_PASSWORD, salt),
                passwordSalt = salt,
                fullName = "Main Admin",
                role = AccountRole.ADMIN,
                notes = "Primary admin account"
            )
        )
    }

    private fun ensureDemoCustomer() {
        if (customers.any { it.email.equals(DEMO_EMAIL, true) }) return
        val salt = PasswordHasher.newSalt()
        customers.add(
            Customer(
                id = "cust-demo",
                email = DEMO_EMAIL,
                passwordHash = PasswordHasher.hash("demo1234", salt),
                passwordSalt = salt,
                fullName = "Demo Customer",
                phone = "(505) 555-0142",
                dateOfBirth = "1990-01-01",
                marketingOptIn = true,
                role = AccountRole.CUSTOMER,
                notes = "Seeded demo customer account"
            )
        )
    }

    private fun persist() {
        storeDir.mkdirs()
        val data = PersistedStore(
            products = products.toList(),
            customers = customers.toList(),
            orders = orders.toList(),
            orderLines = orderLines.toList(),
            cart = cart.toList(),
            sessionCustomerId = sessionCustomerId,
            ageVerified = ageVerified
        )
        storeFile.writeText(json.encodeToString(data))
    }

    companion object {
        const val TAX_RATE = 0.08
        const val MAIN_ADMIN_ID = "admin-main"
        const val MAIN_ADMIN_USERNAME = "admin"
        const val MAIN_ADMIN_EMAIL = "fidelgutierrez33@gmail.com"
        const val MAIN_ADMIN_PASSWORD = "12345678"
        private const val DEMO_EMAIL = "demo@nativepure.example"
    }
}
