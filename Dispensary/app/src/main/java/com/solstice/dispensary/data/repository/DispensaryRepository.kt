package com.solstice.dispensary.data.repository

import android.content.Context
import com.solstice.dispensary.data.auth.PasswordHasher
import com.solstice.dispensary.data.auth.PasswordPolicy
import com.solstice.dispensary.data.db.DispensaryDatabase
import com.solstice.dispensary.data.db.SeedCatalog
import com.solstice.dispensary.data.model.AccountRole
import com.solstice.dispensary.data.model.AuthResult
import com.solstice.dispensary.data.model.AutoLockTimeout
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
import com.solstice.dispensary.data.model.SecuritySettings
import com.solstice.dispensary.data.model.ThemeMode
import com.solstice.dispensary.data.model.toProfile
import com.solstice.dispensary.scan.NewProductFactory
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import java.util.UUID

@OptIn(ExperimentalCoroutinesApi::class)
class DispensaryRepository(context: Context) {
    private val db = DispensaryDatabase.get(context)
    private val prefs = context.getSharedPreferences("solstice_prefs", Context.MODE_PRIVATE)

    private val sessionCustomerId = MutableStateFlow(prefs.getString(KEY_CUSTOMER_ID, null))

    val products: Flow<List<Product>> = db.productDao().observeAll()
    val inventory: Flow<List<Product>> = db.productDao().observeInventory()

    /** Storefront catalog: customers only see published products; staff/admin see everything. */
    val catalog: Flow<List<Product>> = sessionCustomerId.flatMapLatest { id ->
        if (id.isNullOrBlank()) {
            db.productDao().observePublished()
        } else {
            val me = db.customerDao().getById(id)
            if (me?.role?.canManageInventory == true) {
                db.productDao().observeAll()
            } else {
                db.productDao().observePublished()
            }
        }
    }

    val featured: Flow<List<Product>> = sessionCustomerId.flatMapLatest { id ->
        if (id.isNullOrBlank()) {
            db.productDao().observeFeaturedPublished()
        } else {
            val me = db.customerDao().getById(id)
            if (me?.role?.canManageInventory == true) {
                db.productDao().observeFeatured()
            } else {
                db.productDao().observeFeaturedPublished()
            }
        }
    }
    val cartItems: Flow<List<CartItem>> = db.cartDao().observeAll()
    val intakes: Flow<List<InventoryIntake>> = db.inventoryDao().observeAll()
    val customers: Flow<List<CustomerProfile>> = db.customerDao().observeAll()
        .map { list -> list.map { it.toProfile() } }

    /** Admins/staff see all orders; customers only see their own. */
    val visibleOrders: Flow<List<Order>> = sessionCustomerId.flatMapLatest { id ->
        if (id.isNullOrBlank()) {
            flowOf(emptyList())
        } else {
            val me = db.customerDao().getById(id)
            if (me?.role?.canViewSensitiveInfo == true) {
                db.orderDao().observeAll()
            } else {
                db.orderDao().observeForCustomer(id)
            }
        }
    }

    val cartSummary: Flow<CartSummary> = combine(catalog, cartItems) { allProducts, items ->
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
        return sessionCustomerId.flatMapLatest { id ->
            val staff = id?.let { db.customerDao().getById(it) }?.role?.canManageInventory == true
            when {
                category == null && staff -> db.productDao().observeAll()
                category == null -> db.productDao().observePublished()
                staff -> db.productDao().observeByCategory(category)
                else -> db.productDao().observePublishedByCategory(category)
            }
        }
    }

    fun product(id: String): Flow<Product?> = sessionCustomerId.flatMapLatest { customerId ->
        db.productDao().observeById(id).map { product ->
            if (product == null) return@map null
            if (product.published) return@map product
            val me = customerId?.let { db.customerDao().getById(it) }
            if (me?.role?.canManageInventory == true) product else null
        }
    }

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

    fun getSecuritySettings(): SecuritySettings {
        val timeoutName = prefs.getString(KEY_AUTO_LOCK, AutoLockTimeout.FIVE_MINUTES.name)
            ?: AutoLockTimeout.FIVE_MINUTES.name
        val timeout = runCatching { AutoLockTimeout.valueOf(timeoutName) }
            .getOrDefault(AutoLockTimeout.FIVE_MINUTES)
        val hasPin = !prefs.getString(KEY_APP_PIN_HASH, null).isNullOrBlank()
        return SecuritySettings(
            appLockEnabled = prefs.getBoolean(KEY_APP_LOCK_ENABLED, false) && hasPin,
            autoLockTimeout = timeout,
            hasPin = hasPin
        )
    }

    fun setAutoLockTimeout(timeout: AutoLockTimeout) {
        prefs.edit().putString(KEY_AUTO_LOCK, timeout.name).apply()
    }

    /** Returns error message, or null on success. */
    fun enableAppLock(pin: String): String? {
        PasswordPolicy.validatePin(pin)?.let { return it }
        val salt = PasswordHasher.newSalt()
        prefs.edit()
            .putBoolean(KEY_APP_LOCK_ENABLED, true)
            .putString(KEY_APP_PIN_SALT, salt)
            .putString(KEY_APP_PIN_HASH, PasswordHasher.hash(pin, salt))
            .apply()
        return null
    }

    fun disableAppLock() {
        prefs.edit()
            .putBoolean(KEY_APP_LOCK_ENABLED, false)
            .remove(KEY_APP_PIN_HASH)
            .remove(KEY_APP_PIN_SALT)
            .apply()
    }

    fun verifyAppPin(pin: String): Boolean {
        val salt = prefs.getString(KEY_APP_PIN_SALT, null) ?: return false
        val hash = prefs.getString(KEY_APP_PIN_HASH, null) ?: return false
        return PasswordHasher.matches(pin, salt, hash)
    }

    fun markAppBackgrounded() {
        prefs.edit().putLong(KEY_LAST_BACKGROUND_AT, System.currentTimeMillis()).apply()
    }

    fun shouldLockOnResume(): Boolean {
        val settings = getSecuritySettings()
        if (!settings.appLockEnabled) return false
        val timeout = settings.autoLockTimeout
        if (timeout == AutoLockTimeout.NEVER) return false
        if (timeout == AutoLockTimeout.IMMEDIATE) return true
        val backgroundedAt = prefs.getLong(KEY_LAST_BACKGROUND_AT, 0L)
        if (backgroundedAt <= 0L) return false
        return System.currentTimeMillis() - backgroundedAt >= timeout.millis
    }

    fun clearBackgroundMark() {
        prefs.edit().remove(KEY_LAST_BACKGROUND_AT).apply()
    }

    fun loginLockoutRemainingMs(identifier: String): Long {
        val key = lockoutKey(identifier)
        val until = prefs.getLong(key, 0L)
        val remaining = until - System.currentTimeMillis()
        return remaining.coerceAtLeast(0L)
    }

    private fun lockoutKey(identifier: String) =
        KEY_LOCKOUT_UNTIL_PREFIX + identifier.trim().lowercase()

    private fun failedKey(identifier: String) =
        KEY_FAILED_LOGIN_PREFIX + identifier.trim().lowercase()

    private fun recordFailedLogin(identifier: String) {
        val key = failedKey(identifier)
        val count = prefs.getInt(key, 0) + 1
        val editor = prefs.edit().putInt(key, count)
        if (count >= PasswordPolicy.MAX_FAILED_ATTEMPTS) {
            editor.putLong(
                lockoutKey(identifier),
                System.currentTimeMillis() + PasswordPolicy.LOCKOUT_DURATION_MS
            )
            editor.putInt(key, 0)
        }
        editor.apply()
    }

    private fun clearFailedLogins(identifier: String) {
        prefs.edit()
            .remove(failedKey(identifier))
            .remove(lockoutKey(identifier))
            .apply()
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
        ensureMainAdmin()
        if (db.customerDao().getByEmail(DEMO_CUSTOMER_EMAIL) == null) {
            seedDemoCustomer()
        }
    }

    private suspend fun ensureMainAdmin() {
        val existing = db.customerDao().getByEmail(MAIN_ADMIN_EMAIL)
            ?: db.customerDao().getByUsername(MAIN_ADMIN_USERNAME)
        if (existing != null) return

        val salt = PasswordHasher.newSalt()
        db.customerDao().insert(
            Customer(
                id = MAIN_ADMIN_ID,
                email = MAIN_ADMIN_EMAIL,
                username = MAIN_ADMIN_USERNAME,
                passwordHash = PasswordHasher.hash(MAIN_ADMIN_PASSWORD, salt),
                passwordSalt = salt,
                fullName = "Main Admin",
                phone = "",
                notes = "Primary admin account",
                role = AccountRole.ADMIN
            )
        )
    }

    private suspend fun seedDemoCustomer() {
        val salt = PasswordHasher.newSalt()
        val customer = Customer(
            id = "cust-demo",
            email = DEMO_CUSTOMER_EMAIL,
            username = "",
            passwordHash = PasswordHasher.hash("demo1234", salt),
            passwordSalt = salt,
            fullName = "Demo Customer",
            phone = "(505) 555-0142",
            dateOfBirth = "1990-01-01",
            notes = "Seeded demo customer account",
            marketingOptIn = true,
            role = AccountRole.CUSTOMER
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
            PasswordPolicy.validatePassword(password) != null ->
                return AuthResult.Error(PasswordPolicy.validatePassword(password)!!)
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
            username = "",
            passwordHash = PasswordHasher.hash(password, salt),
            passwordSalt = salt,
            fullName = cleanName,
            phone = phone.trim(),
            dateOfBirth = dateOfBirth.trim(),
            createdAt = now,
            lastLoginAt = now,
            marketingOptIn = marketingOptIn,
            role = AccountRole.CUSTOMER
        )
        return try {
            db.customerDao().insert(customer)
            setSession(customer.id)
            AuthResult.Success(customer.toProfile())
        } catch (_: Exception) {
            AuthResult.Error("Could not create account. Try a different email.")
        }
    }

    /**
     * Main admin creates a staff sub-account by email.
     * Staff can view sensitive customer/order info and inventory.
     */
    suspend fun createStaffSubAccount(
        email: String,
        password: String,
        fullName: String
    ): AuthResult {
        val admin = currentCustomer()
            ?: return AuthResult.Error("Not signed in.")
        if (!admin.role.canManageStaff) {
            return AuthResult.Error("Only the main admin can create staff sub-accounts.")
        }

        val cleanEmail = email.trim().lowercase()
        val cleanName = fullName.trim().ifBlank { cleanEmail.substringBefore("@") }
        when {
            cleanEmail.isBlank() || "@" !in cleanEmail ->
                return AuthResult.Error("Enter a valid staff email.")
            PasswordPolicy.validatePassword(password) != null ->
                return AuthResult.Error(PasswordPolicy.validatePassword(password)!!)
            db.customerDao().getByEmail(cleanEmail) != null ->
                return AuthResult.Error("An account with that email already exists.")
        }

        val salt = PasswordHasher.newSalt()
        val staff = Customer(
            id = "staff-" + UUID.randomUUID().toString().take(8),
            email = cleanEmail,
            username = "",
            passwordHash = PasswordHasher.hash(password, salt),
            passwordSalt = salt,
            fullName = cleanName,
            role = AccountRole.STAFF,
            createdByAdminId = admin.id,
            notes = "Staff sub-account created by ${admin.email}",
            mustChangePassword = true
        )
        return try {
            db.customerDao().insert(staff)
            AuthResult.Success(staff.toProfile())
        } catch (_: Exception) {
            AuthResult.Error("Could not create staff account.")
        }
    }

    suspend fun login(emailOrUsername: String, password: String): AuthResult {
        val raw = emailOrUsername.trim()
        if (raw.isBlank()) return AuthResult.Error("Enter your email or username.")

        val remaining = loginLockoutRemainingMs(raw)
        if (remaining > 0) {
            val minutes = ((remaining + 59_999) / 60_000).coerceAtLeast(1)
            return AuthResult.Error(
                "Too many failed attempts. Try again in $minutes min."
            )
        }

        val customer = when {
            raw.contains("@") -> db.customerDao().getByEmail(raw.lowercase())
            else -> db.customerDao().getByUsername(raw)
                ?: db.customerDao().getByEmail(raw.lowercase())
        }
        if (customer == null) {
            recordFailedLogin(raw)
            return AuthResult.Error("No account found for that email or username.")
        }

        if (!PasswordHasher.matches(password, customer.passwordSalt, customer.passwordHash)) {
            recordFailedLogin(raw)
            val left = PasswordPolicy.MAX_FAILED_ATTEMPTS -
                prefs.getInt(failedKey(raw), 0)
            return if (loginLockoutRemainingMs(raw) > 0) {
                AuthResult.Error("Too many failed attempts. Account locked for 5 minutes.")
            } else {
                AuthResult.Error(
                    "Incorrect password. ${left.coerceAtLeast(0)} attempts left before lockout."
                )
            }
        }

        clearFailedLogins(raw)
        val updated = customer.copy(lastLoginAt = System.currentTimeMillis())
        db.customerDao().update(updated)
        setSession(updated.id)
        return AuthResult.Success(updated.toProfile())
    }

    suspend fun changePassword(
        currentPassword: String,
        newPassword: String,
        confirmPassword: String
    ): AuthResult {
        val id = currentCustomerId() ?: return AuthResult.Error("Not signed in.")
        val existing = db.customerDao().getById(id) ?: return AuthResult.Error("Account not found.")

        if (!PasswordHasher.matches(currentPassword, existing.passwordSalt, existing.passwordHash)) {
            return AuthResult.Error("Current password is incorrect.")
        }
        PasswordPolicy.validatePassword(newPassword)?.let { return AuthResult.Error(it) }
        if (newPassword != confirmPassword) {
            return AuthResult.Error("New passwords do not match.")
        }
        if (currentPassword == newPassword) {
            return AuthResult.Error("Choose a password different from your current one.")
        }

        val salt = PasswordHasher.newSalt()
        val updated = existing.copy(
            passwordHash = PasswordHasher.hash(newPassword, salt),
            passwordSalt = salt,
            mustChangePassword = false
        )
        db.customerDao().update(updated)
        return AuthResult.Success(updated.toProfile())
    }

    suspend fun forceChangePassword(
        newPassword: String,
        confirmPassword: String
    ): AuthResult {
        val id = currentCustomerId() ?: return AuthResult.Error("Not signed in.")
        val existing = db.customerDao().getById(id) ?: return AuthResult.Error("Account not found.")
        if (!existing.mustChangePassword) {
            return AuthResult.Error("Password change is not required.")
        }
        PasswordPolicy.validatePassword(newPassword)?.let { return AuthResult.Error(it) }
        if (newPassword != confirmPassword) {
            return AuthResult.Error("New passwords do not match.")
        }
        if (PasswordHasher.matches(newPassword, existing.passwordSalt, existing.passwordHash)) {
            return AuthResult.Error("Choose a password different from your temporary one.")
        }
        val salt = PasswordHasher.newSalt()
        val updated = existing.copy(
            passwordHash = PasswordHasher.hash(newPassword, salt),
            passwordSalt = salt,
            mustChangePassword = false
        )
        db.customerDao().update(updated)
        return AuthResult.Success(updated.toProfile())
    }

    suspend fun verifyAccountPassword(password: String): Boolean {
        val id = currentCustomerId() ?: return false
        val existing = db.customerDao().getById(id) ?: return false
        return PasswordHasher.matches(password, existing.passwordSalt, existing.passwordHash)
    }

    fun logout() {
        prefs.edit().remove(KEY_CUSTOMER_ID).apply()
        sessionCustomerId.value = null
        clearBackgroundMark()
    }

    private fun setSession(customerId: String) {
        prefs.edit().putString(KEY_CUSTOMER_ID, customerId).apply()
        sessionCustomerId.value = customerId
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
        val product = db.productDao().getById(productId) ?: return
        val me = currentCustomer()
        if (!product.published && me?.role?.canManageInventory != true) return
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
        val me = currentCustomer()
        if (me?.role?.canManageInventory != true) return null
        val product = db.productDao().getById(productId) ?: return null
        val next = (product.stockQuantity + delta).coerceAtLeast(0)
        val updated = product.copy(stockQuantity = next, inStock = next > 0)
        db.productDao().update(updated)
        return updated
    }

    suspend fun setPublished(productId: String, published: Boolean): Product? {
        val me = currentCustomer()
        if (me?.role?.canManageInventory != true) return null
        val product = db.productDao().getById(productId) ?: return null
        val updated = product.copy(published = published)
        db.productDao().update(updated)
        return updated
    }

    suspend fun applyScanIntake(
        result: LabelScanResult,
        quantity: Int,
        createIfMissing: Boolean = true
    ): InventoryIntake? {
        val me = currentCustomer()
        if (me?.role?.canManageInventory != true) return null

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

        // Only decrement stock if caller is staff/admin; customer orders still decrement via system
        lines.forEach { line ->
            val product = db.productDao().getById(line.product.id) ?: return@forEach
            val next = (product.stockQuantity - line.quantity).coerceAtLeast(0)
            db.productDao().update(product.copy(stockQuantity = next, inStock = next > 0))
        }

        db.cartDao().clear()
        return order
    }

    companion object {
        const val TAX_RATE = 0.08
        const val MAIN_ADMIN_ID = "admin-main"
        const val MAIN_ADMIN_USERNAME = "admin"
        const val MAIN_ADMIN_EMAIL = "fidelgutierrez33@gmail.com"
        const val MAIN_ADMIN_PASSWORD = "12345678"
        private const val DEMO_CUSTOMER_EMAIL = "demo@nativepure.example"
        private const val KEY_AGE = "age_verified"
        private const val KEY_CUSTOMER_ID = "customer_id"
        private const val KEY_THEME_MODE = "theme_mode"
        private const val KEY_APP_LOCK_ENABLED = "app_lock_enabled"
        private const val KEY_APP_PIN_HASH = "app_pin_hash"
        private const val KEY_APP_PIN_SALT = "app_pin_salt"
        private const val KEY_AUTO_LOCK = "auto_lock_timeout"
        private const val KEY_LAST_BACKGROUND_AT = "last_background_at"
        private const val KEY_FAILED_LOGIN_PREFIX = "failed_login_"
        private const val KEY_LOCKOUT_UNTIL_PREFIX = "lockout_until_"
    }
}
