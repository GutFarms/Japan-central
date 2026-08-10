package com.solstice.dispensary.data.repository

import android.content.Context
import com.solstice.dispensary.data.auth.PasswordHasher
import com.solstice.dispensary.data.db.DispensaryDatabase
import com.solstice.dispensary.data.db.SeedCatalog
import com.solstice.dispensary.data.model.AuthResult
import com.solstice.dispensary.data.model.CartItem
import com.solstice.dispensary.data.model.CartLine
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.data.model.Customer
import com.solstice.dispensary.data.model.CustomerProfile
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.LabelScanResult
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.ThemeMode
import com.solstice.dispensary.data.model.toProfile
import com.solstice.dispensary.scan.NewProductFactory
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import java.util.UUID

class DispensaryRepository(context: Context) {
    private val db = DispensaryDatabase.get(context)
    private val prefs = context.getSharedPreferences("solstice_prefs", Context.MODE_PRIVATE)

    val products: Flow<List<Product>> = db.productDao().observeAll()
    val inventory: Flow<List<Product>> = db.productDao().observeInventory()
    val featured: Flow<List<Product>> = db.productDao().observeFeatured()
    val cartItems: Flow<List<CartItem>> = db.cartDao().observeAll()
    val orders: Flow<List<Order>> = db.orderDao().observeAll()
    val intakes: Flow<List<InventoryIntake>> = db.inventoryDao().observeAll()
    val customers: Flow<List<CustomerProfile>> = db.customerDao().observeAll()
        .map { list -> list.map { it.toProfile() } }

    val cartSummary: Flow<CartSummary> = combine(products, cartItems) { allProducts, items ->
        val byId = allProducts.associateBy { it.id }
        val lines = items.mapNotNull { item ->
            byId[item.productId]?.let { CartLine(it, item.quantity) }
        }
        val subtotal = lines.sumOf { it.lineTotal }
        val tax = subtotal * TAX_RATE
        CartSummary(
            lines = lines,
            subtotal = subtotal,
            tax = tax,
            total = subtotal + tax,
            itemCount = lines.sumOf { it.quantity }
        )
    }

    fun productsByCategory(category: ProductCategory?): Flow<List<Product>> {
        return if (category == null) products else db.productDao().observeByCategory(category)
    }

    fun product(id: String): Flow<Product?> = db.productDao().observeById(id)

    fun orderLines(orderId: String): Flow<List<OrderLine>> = db.orderDao().observeLines(orderId)

    suspend fun catalogSnapshot(): List<Product> = db.productDao().getAll()

    fun isAgeVerified(): Boolean = prefs.getBoolean(KEY_AGE, false)

    fun setAgeVerified(verified: Boolean) {
        prefs.edit().putBoolean(KEY_AGE, verified).apply()
    }

    fun getThemeMode(): ThemeMode {
        val stored = prefs.getString(KEY_THEME_MODE, ThemeMode.SYSTEM.name) ?: ThemeMode.SYSTEM.name
        return runCatching { ThemeMode.valueOf(stored) }.getOrDefault(ThemeMode.SYSTEM)
    }

    fun setThemeMode(mode: ThemeMode) {
        prefs.edit().putString(KEY_THEME_MODE, mode.name).apply()
    }

    fun currentCustomerId(): String? = prefs.getString(KEY_CUSTOMER_ID, null)

    suspend fun currentCustomer(): CustomerProfile? {
        val id = currentCustomerId() ?: return null
        return db.customerDao().getById(id)?.toProfile()
    }

    suspend fun ensureSeeded() {
        if (db.productDao().count() == 0) {
            db.productDao().insertAll(SeedCatalog.products)
        }
        if (db.customerDao().count() == 0) {
            seedDemoCustomer()
        }
    }

    private suspend fun seedDemoCustomer() {
        val salt = PasswordHasher.newSalt()
        val customer = Customer(
            id = "cust-demo",
            email = "demo@nativepure.example",
            passwordHash = PasswordHasher.hash("demo1234", salt),
            passwordSalt = salt,
            fullName = "Demo Customer",
            phone = "(505) 555-0142",
            dateOfBirth = "1990-01-01",
            notes = "Seeded demo account",
            marketingOptIn = true
        )
        db.customerDao().insert(customer)
    }

    suspend fun registerCustomer(
        email: String,
        password: String,
        fullName: String,
        phone: String,
        dateOfBirth: String,
        marketingOptIn: Boolean
    ): AuthResult {
        val cleanEmail = email.trim().lowercase()
        val cleanName = fullName.trim()
        when {
            cleanEmail.isBlank() || "@" !in cleanEmail ->
                return AuthResult.Error("Enter a valid email address.")
            password.length < 6 ->
                return AuthResult.Error("Password must be at least 6 characters.")
            cleanName.length < 2 ->
                return AuthResult.Error("Enter your full name.")
            db.customerDao().getByEmail(cleanEmail) != null ->
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
            marketingOptIn = marketingOptIn
        )
        return try {
            db.customerDao().insert(customer)
            setSession(customer.id)
            AuthResult.Success(customer.toProfile())
        } catch (_: Exception) {
            AuthResult.Error("Could not create account. Try a different email.")
        }
    }

    suspend fun login(email: String, password: String): AuthResult {
        val cleanEmail = email.trim().lowercase()
        val customer = db.customerDao().getByEmail(cleanEmail)
            ?: return AuthResult.Error("No account found for that email.")
        if (!PasswordHasher.matches(password, customer.passwordSalt, customer.passwordHash)) {
            return AuthResult.Error("Incorrect password.")
        }
        val updated = customer.copy(lastLoginAt = System.currentTimeMillis())
        db.customerDao().update(updated)
        setSession(updated.id)
        return AuthResult.Success(updated.toProfile())
    }

    fun logout() {
        prefs.edit().remove(KEY_CUSTOMER_ID).apply()
    }

    private fun setSession(customerId: String) {
        prefs.edit().putString(KEY_CUSTOMER_ID, customerId).apply()
    }

    suspend fun updateProfile(
        fullName: String,
        phone: String,
        dateOfBirth: String,
        notes: String,
        marketingOptIn: Boolean
    ): AuthResult {
        val id = currentCustomerId() ?: return AuthResult.Error("Not signed in.")
        val existing = db.customerDao().getById(id) ?: return AuthResult.Error("Account not found.")
        val updated = existing.copy(
            fullName = fullName.trim().ifBlank { existing.fullName },
            phone = phone.trim(),
            dateOfBirth = dateOfBirth.trim(),
            notes = notes.trim(),
            marketingOptIn = marketingOptIn
        )
        db.customerDao().update(updated)
        return AuthResult.Success(updated.toProfile())
    }

    suspend fun addToCart(productId: String, quantity: Int = 1) {
        val existing = db.cartDao().get(productId)
        if (existing == null) {
            db.cartDao().upsert(CartItem(productId, quantity.coerceAtLeast(1)))
        } else {
            db.cartDao().upsert(existing.copy(quantity = existing.quantity + quantity))
        }
    }

    suspend fun setCartQuantity(productId: String, quantity: Int) {
        if (quantity <= 0) {
            db.cartDao().delete(productId)
        } else {
            db.cartDao().upsert(CartItem(productId, quantity))
        }
    }

    suspend fun removeFromCart(productId: String) {
        db.cartDao().delete(productId)
    }

    suspend fun clearCart() {
        db.cartDao().clear()
    }

    suspend fun adjustStock(productId: String, delta: Int): Product? {
        val product = db.productDao().getById(productId) ?: return null
        val next = (product.stockQuantity + delta).coerceAtLeast(0)
        val updated = product.copy(stockQuantity = next, inStock = next > 0)
        db.productDao().update(updated)
        return updated
    }

    suspend fun setStock(productId: String, quantity: Int): Product? {
        val product = db.productDao().getById(productId) ?: return null
        val next = quantity.coerceAtLeast(0)
        val updated = product.copy(stockQuantity = next, inStock = next > 0)
        db.productDao().update(updated)
        return updated
    }

    suspend fun applyScanIntake(
        result: LabelScanResult,
        quantity: Int,
        createIfMissing: Boolean = true
    ): InventoryIntake? {
        val qty = quantity.coerceAtLeast(1)
        val product = when {
            result.matchedProduct != null -> result.matchedProduct
            createIfMissing -> {
                val draft = NewProductFactory.fromScan(result)
                db.productDao().upsert(draft)
                draft
            }
            else -> return null
        }

        val updatedQty = product.stockQuantity + qty
        db.productDao().update(
            product.copy(stockQuantity = updatedQty, inStock = true)
        )

        val intake = InventoryIntake(
            id = UUID.randomUUID().toString().take(8).uppercase(),
            createdAt = System.currentTimeMillis(),
            productId = product.id,
            productName = product.name,
            quantityAdded = qty,
            source = "ai_scan",
            rawLabelText = result.rawText.take(1000),
            confidence = result.matchConfidence,
            barcode = result.barcode.orEmpty()
        )
        db.inventoryDao().insert(intake)
        return intake
    }

    suspend fun placePickupOrder(pickupName: String, notes: String): Order? {
        val items = db.cartDao().getAll()
        if (items.isEmpty()) return null

        val catalog = products.first().associateBy { it.id }
        val lines = items.mapNotNull { item ->
            catalog[item.productId]?.let { product ->
                CartLine(product, item.quantity)
            }
        }
        if (lines.isEmpty()) return null

        val customer = currentCustomer()
        val subtotal = lines.sumOf { it.lineTotal }
        val tax = subtotal * TAX_RATE
        val order = Order(
            id = UUID.randomUUID().toString().take(8).uppercase(),
            createdAt = System.currentTimeMillis(),
            total = subtotal + tax,
            itemCount = lines.sumOf { it.quantity },
            status = "Ready for pickup",
            pickupName = pickupName.ifBlank { customer?.fullName ?: "Guest" },
            notes = notes.trim(),
            customerId = customer?.id.orEmpty(),
            customerEmail = customer?.email.orEmpty()
        )
        val orderLines = lines.map {
            OrderLine(
                orderId = order.id,
                productId = it.product.id,
                productName = it.product.name,
                unitPrice = it.product.price,
                quantity = it.quantity
            )
        }
        db.orderDao().placeOrder(order, orderLines)

        lines.forEach { line ->
            adjustStock(line.product.id, -line.quantity)
        }

        db.cartDao().clear()
        return order
    }

    companion object {
        const val TAX_RATE = 0.08
        private const val KEY_AGE = "age_verified"
        private const val KEY_CUSTOMER_ID = "customer_id"
        private const val KEY_THEME_MODE = "theme_mode"
    }
}
