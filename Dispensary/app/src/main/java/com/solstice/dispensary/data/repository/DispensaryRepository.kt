package com.solstice.dispensary.data.repository

import android.content.Context
import android.content.SharedPreferences
import android.graphics.Bitmap
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import com.solstice.dispensary.data.auth.PasswordHasher
import com.solstice.dispensary.data.auth.PasswordPolicy
import com.solstice.dispensary.data.db.DispensaryDatabase
import com.solstice.dispensary.data.db.SeedCatalog
import com.solstice.dispensary.data.images.ProductImageStore
import com.solstice.dispensary.data.model.AccountRole
import com.solstice.dispensary.data.model.AuthResult
import com.solstice.dispensary.data.model.AutoLockTimeout
import com.solstice.dispensary.data.model.CartItem
import com.solstice.dispensary.data.model.CartLine
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.data.model.Customer
import com.solstice.dispensary.data.model.CustomerProfile
import com.solstice.dispensary.data.mail.MailApiClient
import com.solstice.dispensary.data.model.EmailCodeIssue
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.LabelScanResult
import com.solstice.dispensary.data.model.LoyaltyPoints
import com.solstice.dispensary.data.model.OpResult
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.OrderStatus
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.ProductSize
import com.solstice.dispensary.data.model.RequestBoxStats
import com.solstice.dispensary.data.model.ProductRequest
import com.solstice.dispensary.data.model.SecuritySettings
import com.solstice.dispensary.data.model.SizePricing
import com.solstice.dispensary.data.model.StrainType
import com.solstice.dispensary.data.model.ThemeMode
import com.solstice.dispensary.data.model.toProfile
import com.solstice.dispensary.data.sync.InventorySync
import com.solstice.dispensary.scan.NewProductFactory
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import java.security.SecureRandom
import java.util.UUID

@OptIn(ExperimentalCoroutinesApi::class)
class DispensaryRepository(context: Context) {
    private val appContext = context.applicationContext
    private val db = DispensaryDatabase.get(appContext)
    private val prefs: SharedPreferences = createSecurePrefs(appContext)

    private val sessionCustomerId = MutableStateFlow(prefs.getString(KEY_CUSTOMER_ID, null))

    val products: Flow<List<Product>> = db.productDao().observeAll()
    val inventory: Flow<List<Product>> = db.productDao().observeInventory()

    /** Customer-facing storefront: published products only (drafts live in Stock). */
    val catalog: Flow<List<Product>> = db.productDao().observePublished()

    val featured: Flow<List<Product>> = db.productDao().observeFeaturedPublished()
    val deals: Flow<List<Product>> = db.productDao().observePublishedDeals()
    val cartItems: Flow<List<CartItem>> = db.cartDao().observeAll()
    val intakes: Flow<List<InventoryIntake>> = db.inventoryDao().observeAll()
    val customers: Flow<List<CustomerProfile>> = combine(
        sessionCustomerId,
        db.customerDao().observeAll()
    ) { id, all ->
        if (id.isNullOrBlank()) return@combine emptyList()
        val me = all.find { it.id == id }
        if (me?.role?.canViewSensitiveInfo == true) {
            all.map { it.toProfile() }
        } else {
            emptyList()
        }
    }

    val staffAccounts: Flow<List<CustomerProfile>> =
        db.customerDao().observeByRole(AccountRole.STAFF)
            .map { list -> list.map { it.toProfile() } }

    val productRequests: Flow<List<ProductRequest>> =
        sessionCustomerId.flatMapLatest { id ->
            if (id.isNullOrBlank()) {
                flowOf(emptyList())
            } else {
                val me = db.customerDao().getById(id)
                if (me?.role?.canViewSensitiveInfo == true) {
                    db.productRequestDao().observeAll()
                } else {
                    db.productRequestDao().observeForCustomer(id)
                }
            }
        }

    val requestBoxStats: Flow<RequestBoxStats> = combine(
        db.productRequestDao().observeAll(),
        db.customerDao().observeByRole(AccountRole.CUSTOMER)
    ) { requests, customers ->
        val unique = requests.map { it.customerId }.toSet().size
        val totalCustomers = customers.size
        val pct = if (totalCustomers <= 0) {
            0f
        } else {
            unique * 100f / totalCustomers.toFloat()
        }
        RequestBoxStats(
            totalRequests = requests.size,
            uniqueRequesters = unique,
            totalCustomers = totalCustomers,
            requesterToCustomerPercent = pct
        )
    }

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
            val product = byId[item.productId] ?: return@mapNotNull null
            val size = ProductSize.fromKey(item.sizeKey)
            if (product.sizeInventoryEnabled && size == null && item.sizeKey != ProductSize.UNIT_KEY) {
                return@mapNotNull null
            }
            CartLine(product, item.quantity, size)
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
        return if (category == null) {
            db.productDao().observePublished()
        } else {
            db.productDao().observePublishedByCategory(category)
        }
    }

    fun product(id: String): Flow<Product?> = db.productDao().observeById(id).map { product ->
        if (product == null) return@map null
        if (product.published) product else null
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
        val demo = db.customerDao().getByEmail(DEMO_CUSTOMER_EMAIL)
        if (demo == null) {
            seedDemoCustomer()
        } else if (!demo.emailVerified) {
            db.customerDao().update(demo.copy(emailVerified = true))
        }
    }

    private suspend fun ensureMainAdmin() {
        val existing = db.customerDao().getByEmail(MAIN_ADMIN_EMAIL)
            ?: db.customerDao().getByUsername(MAIN_ADMIN_USERNAME)
        if (existing != null) {
            var next = existing
            // Always keep the bootstrap admin account privileged and usable.
            if (existing.role != AccountRole.ADMIN) {
                next = next.copy(role = AccountRole.ADMIN)
            }
            if (!existing.enabled) {
                next = next.copy(enabled = true)
            }
            if (existing.username.isBlank()) {
                next = next.copy(username = MAIN_ADMIN_USERNAME)
            }
            // Force password change if still using the bootstrap default.
            if (!existing.mustChangePassword &&
                PasswordHasher.matches(MAIN_ADMIN_PASSWORD, existing.passwordSalt, existing.passwordHash)
            ) {
                next = next.copy(mustChangePassword = true)
            }
            if (!existing.emailVerified) {
                next = next.copy(emailVerified = true)
            }
            if (next != existing) {
                db.customerDao().update(next)
            }
            return
        }

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
                notes = "Primary admin account — change password on first login",
                role = AccountRole.ADMIN,
                mustChangePassword = true,
                emailVerified = true,
                enabled = true
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
            phone = "",
            dateOfBirth = "",
            notes = "",
            marketingOptIn = false,
            role = AccountRole.CUSTOMER,
            emailVerified = true
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
            role = AccountRole.CUSTOMER,
            emailVerified = false
        )
        return try {
            db.customerDao().insert(customer)
            setSession(customer.id)
            lastIssuedEmailCode = storeEmailVerificationCode(customer)
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
            mustChangePassword = true,
            emailVerified = true
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

        if (!customer.enabled) {
            return AuthResult.Error("This account has been disabled. Contact an admin.")
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
        if (!updated.emailVerified) {
            lastIssuedEmailCode = storeEmailVerificationCode(updated)
        }
        return AuthResult.Success(updated.toProfile())
    }

    /**
     * Issues a fresh 6-digit email verification code for the signed-in customer.
     * Attempts delivery through the Native Pure mail server; plaintext is kept for
     * on-device fallback when mail is unreachable.
     */
    suspend fun resendEmailVerificationCode(): AuthResult {
        val id = currentCustomerId() ?: return AuthResult.Error("Not signed in.")
        val existing = db.customerDao().getById(id) ?: return AuthResult.Error("Account not found.")
        if (existing.emailVerified) {
            return AuthResult.Error("Email is already verified.")
        }
        val lastSent = prefs.getLong(KEY_EMAIL_CODE_SENT_AT, 0L)
        val waitMs = EMAIL_CODE_RESEND_COOLDOWN_MS - (System.currentTimeMillis() - lastSent)
        if (waitMs > 0) {
            val secs = ((waitMs + 999) / 1000).coerceAtLeast(1)
            return AuthResult.Error("Wait ${secs}s before requesting another code.")
        }
        lastIssuedEmailCode = storeEmailVerificationCode(existing)
        return AuthResult.Success(existing.toProfile())
    }

    fun peekIssuedEmailCode(): EmailCodeIssue? = lastIssuedEmailCode

    suspend fun verifyEmailCode(code: String): AuthResult {
        val id = currentCustomerId() ?: return AuthResult.Error("Not signed in.")
        val existing = db.customerDao().getById(id) ?: return AuthResult.Error("Account not found.")
        if (existing.emailVerified) {
            return AuthResult.Success(existing.toProfile())
        }
        val clean = code.trim().filter { it.isDigit() }
        if (clean.length != EMAIL_CODE_LENGTH) {
            return AuthResult.Error("Enter the $EMAIL_CODE_LENGTH-digit code from your email.")
        }
        val expectedCustomer = prefs.getString(KEY_EMAIL_CODE_CUSTOMER, null)
        val salt = prefs.getString(KEY_EMAIL_CODE_SALT, null)
        val hash = prefs.getString(KEY_EMAIL_CODE_HASH, null)
        val expiresAt = prefs.getLong(KEY_EMAIL_CODE_EXPIRES_AT, 0L)
        if (expectedCustomer != id || salt.isNullOrBlank() || hash.isNullOrBlank()) {
            return AuthResult.Error("No verification code on file. Tap Resend code.")
        }
        if (System.currentTimeMillis() > expiresAt) {
            clearEmailVerificationCode()
            return AuthResult.Error("That code expired. Tap Resend code.")
        }
        if (!PasswordHasher.matches(clean, salt, hash)) {
            return AuthResult.Error("Incorrect code. Check the email and try again.")
        }
        val updated = existing.copy(emailVerified = true)
        db.customerDao().update(updated)
        clearEmailVerificationCode()
        lastIssuedEmailCode = null
        return AuthResult.Success(updated.toProfile())
    }

    @Volatile
    private var lastIssuedEmailCode: EmailCodeIssue? = null

    private suspend fun storeEmailVerificationCode(customer: Customer): EmailCodeIssue {
        val code = generateEmailCode()
        val salt = PasswordHasher.newSalt()
        val expiresAt = System.currentTimeMillis() + EMAIL_CODE_TTL_MS
        prefs.edit()
            .putString(KEY_EMAIL_CODE_CUSTOMER, customer.id)
            .putString(KEY_EMAIL_CODE_SALT, salt)
            .putString(KEY_EMAIL_CODE_HASH, PasswordHasher.hash(code, salt))
            .putLong(KEY_EMAIL_CODE_EXPIRES_AT, expiresAt)
            .putLong(KEY_EMAIL_CODE_SENT_AT, System.currentTimeMillis())
            .apply()
        val mail = MailApiClient.sendVerificationCode(
            email = customer.email,
            code = code,
            expiresInMinutes = (EMAIL_CODE_TTL_MS / 60_000L).toInt().coerceAtLeast(5)
        )
        return EmailCodeIssue(
            email = customer.email,
            code = code,
            expiresAtMs = expiresAt,
            deliveredByMail = mail.ok,
            mailError = mail.error
        )
    }

    private fun clearEmailVerificationCode() {
        prefs.edit()
            .remove(KEY_EMAIL_CODE_CUSTOMER)
            .remove(KEY_EMAIL_CODE_SALT)
            .remove(KEY_EMAIL_CODE_HASH)
            .remove(KEY_EMAIL_CODE_EXPIRES_AT)
            .apply()
    }

    private fun generateEmailCode(): String {
        val n = SecureRandom().nextInt(1_000_000)
        return n.toString().padStart(EMAIL_CODE_LENGTH, '0')
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
        lastIssuedEmailCode = null
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

    suspend fun addToCart(productId: String, quantity: Int = 1, size: ProductSize? = null) {
        val product = db.productDao().getById(productId) ?: return
        val me = currentCustomer()
        if (!product.published && me?.role?.canManageInventory != true) return
        val resolvedSize = when {
            product.sizeInventoryEnabled -> size ?: product.offeredSizes().firstOrNull { it.inStock }?.size
            else -> null
        }
        if (product.sizeInventoryEnabled && resolvedSize == null) return
        if (product.sizeInventoryEnabled && product.stockFor(resolvedSize) <= 0) return
        val sizeKey = ProductSize.cartKey(resolvedSize)
        val existing = db.cartDao().get(productId, sizeKey)
        if (existing == null) {
            db.cartDao().upsert(CartItem(productId, sizeKey, quantity.coerceAtLeast(1)))
        } else {
            db.cartDao().upsert(existing.copy(quantity = existing.quantity + quantity))
        }
    }

    suspend fun setCartQuantity(productId: String, quantity: Int, size: ProductSize? = null) {
        val sizeKey = ProductSize.cartKey(size)
        if (quantity <= 0) {
            db.cartDao().delete(productId, sizeKey)
        } else {
            db.cartDao().upsert(CartItem(productId, sizeKey, quantity))
        }
    }

    suspend fun removeFromCart(productId: String, size: ProductSize? = null) {
        db.cartDao().delete(productId, ProductSize.cartKey(size))
    }

    suspend fun clearCart() {
        db.cartDao().clear()
    }

    suspend fun adjustStock(productId: String, delta: Int): Product? {
        val me = currentCustomer()
        if (me?.role?.canManageInventory != true) return null
        val product = db.productDao().getById(productId) ?: return null
        val updated = if (product.sizeInventoryEnabled) {
            // Quick ± on the inventory list adjusts the 3.5g size by default.
            product.withSizeStock(
                ProductSize.EIGHTH,
                product.stockEighth + delta
            )
        } else {
            val next = (product.stockQuantity + delta).coerceAtLeast(0)
            product.copy(stockQuantity = next, inStock = next > 0)
        }
        db.productDao().update(updated)
        return updated
    }

    suspend fun setPublished(productId: String, published: Boolean): OpResult {
        val me = currentCustomer()
            ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only staff and admin can publish products.")
        }
        val product = db.productDao().getById(productId)
            ?: return OpResult.Error("Product not found.")

        if (published) {
            when {
                product.sku.isBlank() ->
                    return OpResult.Error("Add a SKU before publishing.")
                product.sizeInventoryEnabled && product.offeredSizes().isEmpty() ->
                    return OpResult.Error("Set at least one size price (1g / 3.5g / 7g / oz) before publishing.")
                !product.sizeInventoryEnabled && product.price <= 0.0 ->
                    return OpResult.Error("Set a price greater than \$0 before publishing.")
                product.name.isBlank() ->
                    return OpResult.Error("Product needs a name before publishing.")
            }
        }

        val updated = product.copy(
            published = published,
            publishedAt = if (published) System.currentTimeMillis() else 0L,
            publishedBy = if (published) me.email else ""
        )
        db.productDao().update(updated)
        return OpResult.Success(
            if (published) {
                "Published ${updated.name} — now on the customer menu."
            } else {
                "Unpublished ${updated.name} — hidden from the customer menu."
            }
        )
    }

    /** Admin/staff: create or update product fields (price, SKU, name, stock, etc.). */
    suspend fun saveProduct(product: Product): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only admin and staff can edit inventory.")
        }
        val name = product.name.trim()
        if (name.length < 2) return OpResult.Error("Product name is required.")
        val price = product.price.coerceAtLeast(0.0)
        val stock = product.stockQuantity.coerceAtLeast(0)
        val sku = product.sku.trim()
        val withSizes = if (product.sizeInventoryEnabled) {
            val prices = if (product.priceEighth <= 0 && price > 0) {
                SizePricing.fromEighth(price)
            } else null
            product.copy(
                stockGram = product.stockGram.coerceAtLeast(0),
                stockEighth = product.stockEighth.coerceAtLeast(0),
                stockQuarter = product.stockQuarter.coerceAtLeast(0),
                stockOunce = product.stockOunce.coerceAtLeast(0),
                priceGram = product.priceGram.coerceAtLeast(0.0).let {
                    if (it <= 0 && prices != null) prices.getValue(ProductSize.GRAM) else it
                },
                priceEighth = product.priceEighth.coerceAtLeast(0.0).let {
                    if (it <= 0 && prices != null) prices.getValue(ProductSize.EIGHTH) else it
                },
                priceQuarter = product.priceQuarter.coerceAtLeast(0.0).let {
                    if (it <= 0 && prices != null) prices.getValue(ProductSize.QUARTER) else it
                },
                priceOunce = product.priceOunce.coerceAtLeast(0.0).let {
                    if (it <= 0 && prices != null) prices.getValue(ProductSize.OUNCE) else it
                }
            ).normalizedSizeInventory()
        } else {
            product.copy(
                sizeInventoryEnabled = false,
                stockQuantity = stock,
                inStock = stock > 0,
                price = price,
                unitLabel = product.unitLabel.trim().ifBlank { "each" }
            )
        }
        if (product.published) {
            when {
                sku.isBlank() -> return OpResult.Error("Add a SKU before keeping this product published.")
                withSizes.sizeInventoryEnabled && withSizes.offeredSizes().isEmpty() ->
                    return OpResult.Error("Set at least one size price before publishing.")
                !withSizes.sizeInventoryEnabled && withSizes.price <= 0.0 ->
                    return OpResult.Error("Set a price greater than \$0 before publishing.")
            }
        }
        val existing = db.productDao().getById(product.id)
        val cleaned = withSizes.copy(
            name = name,
            brand = product.brand.trim().ifBlank { "Native Pure" },
            sku = sku,
            description = product.description.trim(),
            effects = product.effects.trim().ifBlank { "—" },
            thcPercent = product.thcPercent.coerceAtLeast(0.0),
            cbdPercent = product.cbdPercent.coerceAtLeast(0.0),
            onDeal = product.onDeal && product.dealPercent > 0,
            dealPercent = if (product.onDeal) product.dealPercent.coerceIn(0, 90) else 0,
            dealLabel = product.dealLabel.trim(),
            publishedAt = when {
                product.published && existing?.published != true -> System.currentTimeMillis()
                product.published -> existing?.publishedAt ?: System.currentTimeMillis()
                else -> 0L
            },
            publishedBy = if (product.published) me.email else "",
            imagePath = product.imagePath.ifBlank { existing?.imagePath.orEmpty() }
        )
        db.productDao().upsert(cleaned)
        return OpResult.Success(
            if (existing == null) "Created “${cleaned.name}”." else "Saved changes to “${cleaned.name}”."
        )
    }

    /** Staff: save a camera photo for this product and update [Product.imagePath]. */
    suspend fun updateProductPhoto(productId: String, bitmap: Bitmap): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only admin and staff can update product pictures.")
        }
        val product = db.productDao().getById(productId)
            ?: return OpResult.Error("Product not found.")
        val relative = ProductImageStore.save(appContext, productId, bitmap)
        db.productDao().update(product.copy(imagePath = relative))
        return OpResult.Success("Updated photo for “${product.name}”.")
    }

    suspend fun clearProductPhoto(productId: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only admin and staff can update product pictures.")
        }
        val product = db.productDao().getById(productId)
            ?: return OpResult.Error("Product not found.")
        ProductImageStore.deleteForProduct(appContext, productId)
        if (product.imagePath.isNotBlank()) {
            ProductImageStore.delete(appContext, product.imagePath)
        }
        db.productDao().update(product.copy(imagePath = ""))
        return OpResult.Success("Removed photo for “${product.name}”.")
    }

    suspend fun createBlankDraftProduct(): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only admin and staff can add products.")
        }
        val id = "draft-" + UUID.randomUUID().toString().take(8)
        val prices = SizePricing.fromEighth(40.0)
        val product = Product(
            id = id,
            name = "New product",
            brand = "Native Pure",
            category = ProductCategory.FLOWER,
            strainType = StrainType.HYBRID,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 40.0,
            unitLabel = "1g–1oz",
            description = "",
            effects = "—",
            featured = false,
            inStock = false,
            stockQuantity = 0,
            sku = "",
            published = false,
            sizeInventoryEnabled = true,
            priceGram = prices.getValue(ProductSize.GRAM),
            priceEighth = prices.getValue(ProductSize.EIGHTH),
            priceQuarter = prices.getValue(ProductSize.QUARTER),
            priceOunce = prices.getValue(ProductSize.OUNCE)
        ).normalizedSizeInventory()
        db.productDao().upsert(product)
        return OpResult.Success("Draft “${product.name}” created — set size stock, then publish.")
    }

    suspend fun setStaffEnabled(staffId: String, enabled: Boolean): OpResult {
        val admin = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!admin.role.canManageStaff) return OpResult.Error("Only admin can manage staff.")
        val staff = db.customerDao().getById(staffId) ?: return OpResult.Error("Staff not found.")
        if (staff.role != AccountRole.STAFF) return OpResult.Error("Only staff accounts can be toggled.")
        db.customerDao().update(staff.copy(enabled = enabled))
        return OpResult.Success(
            if (enabled) "Enabled ${staff.email}." else "Disabled ${staff.email}."
        )
    }

    suspend fun resetStaffPassword(staffId: String, newPassword: String): OpResult {
        val admin = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!admin.role.canManageStaff) return OpResult.Error("Only admin can reset staff passwords.")
        val staff = db.customerDao().getById(staffId) ?: return OpResult.Error("Staff not found.")
        if (staff.role != AccountRole.STAFF) return OpResult.Error("Only staff passwords can be reset here.")
        PasswordPolicy.validatePassword(newPassword)?.let { return OpResult.Error(it) }
        val salt = PasswordHasher.newSalt()
        db.customerDao().update(
            staff.copy(
                passwordHash = PasswordHasher.hash(newPassword, salt),
                passwordSalt = salt,
                mustChangePassword = true
            )
        )
        return OpResult.Success("Password reset for ${staff.email}. They must change it on next login.")
    }

    suspend fun submitProductRequest(productName: String, notes: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        val name = productName.trim()
        if (name.length < 2) {
            return OpResult.Error("Tell us what product you’re looking for.")
        }
        val request = ProductRequest(
            id = "req-" + UUID.randomUUID().toString().take(8),
            customerId = me.id,
            customerName = me.fullName,
            customerEmail = me.email,
            productName = name,
            notes = notes.trim(),
            status = "Open"
        )
        db.productRequestDao().insert(request)
        return OpResult.Success("Request sent for “$name”. Staff will see it in the request box.")
    }

    suspend fun setProductRequestStatus(requestId: String, status: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canViewSensitiveInfo) {
            return OpResult.Error("Only staff/admin can update request status.")
        }
        val clean = status.trim().ifBlank { "Open" }
        db.productRequestDao().setStatus(requestId, clean)
        return OpResult.Success("Request marked $clean.")
    }

    suspend fun exportSyncJson(): String {
        val me = currentCustomer()
        if (me?.role?.canManageInventory != true) {
            error("Only staff/admin can export inventory sync.")
        }
        val products = db.productDao().getAll()
        val customers = db.customerDao().observeAll().first()
        val orders = db.orderDao().observeAll().first()
        val lines = orders.flatMap { db.orderDao().observeLines(it.id).first() }
        return InventorySync.exportJson(products, orders, lines, customers)
    }

    suspend fun importSyncJson(json: String): OpResult {
        val me = currentCustomer()
            ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only staff/admin can import sync files.")
        }
        return try {
            val products = InventorySync.parseProducts(json)
            products.forEach { db.productDao().upsert(it) }
            val customers = InventorySync.parseCustomers(json)
            var customerCount = 0
            customers.forEach { incoming ->
                if (incoming.id == me.id) return@forEach
                val existing = db.customerDao().getById(incoming.id)
                    ?: db.customerDao().getByEmail(incoming.email)
                if (existing == null) {
                    // New loyalty profile from sync — unusable password until staff resets.
                    db.customerDao().upsert(incoming.copy(role = AccountRole.CUSTOMER))
                } else {
                    // Preserve credentials and role; merge PII / loyalty only.
                    db.customerDao().update(
                        existing.copy(
                            email = incoming.email.trim().lowercase().ifBlank { existing.email },
                            username = incoming.username.ifBlank { existing.username },
                            fullName = incoming.fullName.trim().ifBlank { existing.fullName },
                            phone = incoming.phone,
                            dateOfBirth = incoming.dateOfBirth,
                            notes = incoming.notes,
                            marketingOptIn = incoming.marketingOptIn,
                            enabled = incoming.enabled,
                            emailVerified = incoming.emailVerified || existing.emailVerified,
                            loyaltyPoints = incoming.loyaltyPoints.coerceAtLeast(0),
                            lifetimeSpend = incoming.lifetimeSpend.coerceAtLeast(0.0),
                            lastLoginAt = maxOf(existing.lastLoginAt, incoming.lastLoginAt)
                        )
                    )
                }
                customerCount++
            }
            OpResult.Success(
                "Imported ${products.size} products and $customerCount customers (no passwords) from sync file."
            )
        } catch (t: Throwable) {
            OpResult.Error(t.message ?: "Could not import sync file.")
        }
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

    suspend fun placePickupOrder(
        pickupName: String,
        notes: String,
        redeemPoints: Int = 0
    ): Order? {
        val items = db.cartDao().getAll()
        if (items.isEmpty()) return null

        val catalog = db.productDao().getAll()
            .filter { it.published }
            .associateBy { it.id }
        val lines = items.mapNotNull { item ->
            catalog[item.productId]?.let { product ->
                val size = ProductSize.fromKey(item.sizeKey)
                if (product.sizeInventoryEnabled && size == null) return@mapNotNull null
                CartLine(product, item.quantity, size)
            }
        }
        if (lines.isEmpty()) return null

        val customerProfile = currentCustomer()
        val customerRow = customerProfile?.id?.let { db.customerDao().getById(it) }
        val subtotal = lines.sumOf { it.lineTotal }
        val tax = LoyaltyPoints.taxOn(subtotal, TAX_RATE)
        val gross = subtotal + tax

        val canRedeem = customerRow?.role == AccountRole.CUSTOMER
        val maxRedeem = if (canRedeem && customerRow != null) {
            LoyaltyPoints.maxRedeemablePoints(customerRow.loyaltyPoints, gross)
        } else {
            0
        }
        val appliedRedeem = redeemPoints.coerceIn(0, maxRedeem)
            .let { it - (it % LoyaltyPoints.REDEEM_POINTS_PER_DOLLAR) }
        val discount = LoyaltyPoints.discountForPoints(appliedRedeem)
        val total = (gross - discount).coerceAtLeast(0.0)

        val pointsEarned = if (canRedeem) LoyaltyPoints.pointsForSpend(total) else 0
        val order = Order(
            id = UUID.randomUUID().toString().take(8).uppercase(),
            createdAt = System.currentTimeMillis(),
            total = total,
            itemCount = lines.sumOf { it.quantity },
            status = OrderStatus.READY,
            pickupName = pickupName.ifBlank { customerProfile?.fullName ?: "Guest" },
            notes = notes.trim(),
            customerId = customerProfile?.id.orEmpty(),
            customerEmail = customerProfile?.email.orEmpty(),
            pointsEarned = pointsEarned,
            pointsRedeemed = appliedRedeem,
            discount = discount
        )
        val orderLines = lines.map {
            OrderLine(
                orderId = order.id,
                productId = it.product.id,
                productName = it.product.name,
                unitPrice = it.unitPrice,
                quantity = it.quantity,
                sizeKey = it.sizeKey,
                sizeLabel = it.unitLabel
            )
        }
        db.orderDao().placeOrder(order, orderLines)

        if (customerRow != null && (pointsEarned > 0 || appliedRedeem > 0)) {
            db.customerDao().update(
                customerRow.copy(
                    loyaltyPoints = (customerRow.loyaltyPoints - appliedRedeem + pointsEarned)
                        .coerceAtLeast(0),
                    lifetimeSpend = customerRow.lifetimeSpend + total
                )
            )
        }

        lines.forEach { line ->
            val product = db.productDao().getById(line.product.id) ?: return@forEach
            val updated = if (product.sizeInventoryEnabled && line.size != null) {
                product.withSizeStock(line.size, product.stockFor(line.size) - line.quantity)
            } else {
                val next = (product.stockQuantity - line.quantity).coerceAtLeast(0)
                product.copy(stockQuantity = next, inStock = next > 0)
            }
            db.productDao().update(updated)
        }

        db.cartDao().clear()
        return order
    }

    suspend fun updateOrderStatus(orderId: String, status: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canViewSensitiveInfo) {
            return OpResult.Error("Only staff/admin can update order status.")
        }
        val order = db.orderDao().getById(orderId) ?: return OpResult.Error("Order not found.")
        val clean = status.trim()
        if (clean !in OrderStatus.staffActions) {
            return OpResult.Error("Unknown status.")
        }
        db.orderDao().update(order.copy(status = clean))
        return OpResult.Success("Order ${order.id} marked $clean.")
    }

    fun formatOrderReceipt(order: Order, lines: List<OrderLine>): String {
        val sb = StringBuilder()
        sb.appendLine("Native Pure — Pickup receipt")
        sb.appendLine("Order #${order.id}")
        sb.appendLine("Status: ${order.status}")
        sb.appendLine("Pickup: ${order.pickupName}")
        if (order.customerEmail.isNotBlank()) {
            sb.appendLine("Email: ${maskEmail(order.customerEmail)}")
        }
        sb.appendLine()
        lines.forEach { line ->
            sb.appendLine(
                buildString {
                    append("${line.quantity} × ${line.productName}")
                    if (line.sizeLabel.isNotBlank()) append(" (${line.sizeLabel})")
                    append(" @ $${"%.2f".format(line.unitPrice)} = $${
                        "%.2f".format(line.unitPrice * line.quantity)
                    }")
                }
            )
        }
        sb.appendLine()
        if (order.discount > 0) {
            sb.appendLine("Points redeemed: ${order.pointsRedeemed} (−$${"%.2f".format(order.discount)})")
        }
        sb.appendLine("Total paid: $${"%.2f".format(order.total)}")
        if (order.pointsEarned > 0) sb.appendLine("Points earned: +${order.pointsEarned}")
        sb.appendLine()
        sb.appendLine("Bring a valid ID for pickup. 18+ only.")
        return sb.toString()
    }

    private fun maskEmail(email: String): String {
        val clean = email.trim()
        val at = clean.indexOf('@')
        if (at <= 0) return "•••"
        return "${clean.take(1)}•••@${clean.substring(at + 1)}"
    }

    suspend fun orderReceiptText(orderId: String): String? {
        val order = db.orderDao().getById(orderId) ?: return null
        val lines = db.orderDao().getLines(orderId)
        return formatOrderReceipt(order, lines)
    }

    suspend fun createDraftFromRequest(requestId: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only staff/admin can create drafts from requests.")
        }
        val request = db.productRequestDao().getById(requestId)
            ?: return OpResult.Error("Request not found.")
        val id = "reqprod-" + UUID.randomUUID().toString().take(6)
        val product = Product(
            id = id,
            name = request.productName.trim(),
            brand = "Customer request",
            category = ProductCategory.ACCESSORY,
            strainType = StrainType.NONE,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 0.0,
            unitLabel = "each",
            description = buildString {
                append("Draft from request by ${request.customerName}.")
                if (request.notes.isNotBlank()) append(" Notes: ${request.notes}")
            },
            effects = "—",
            featured = false,
            inStock = false,
            stockQuantity = 0,
            sku = "",
            published = false
        )
        db.productDao().upsert(product)
        db.productRequestDao().setStatus(requestId, "Fulfilled")
        return OpResult.Success(
            "Created draft “${product.name}” in Stock (unpublished). Request marked fulfilled."
        )
    }

    companion object {
        const val TAX_RATE = 0.08
        /** @see LoyaltyPoints.POINTS_PER_DOLLAR */
        const val LOYALTY_POINTS_PER_DOLLAR = LoyaltyPoints.POINTS_PER_DOLLAR
        const val MAIN_ADMIN_ID = "admin-main"
        const val MAIN_ADMIN_USERNAME = "admin"
        const val MAIN_ADMIN_EMAIL = "fidelgutierrez33@gmail.com"
        /** Bootstrap only — must be changed on first login. */
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
        private const val KEY_EMAIL_CODE_CUSTOMER = "email_code_customer"
        private const val KEY_EMAIL_CODE_SALT = "email_code_salt"
        private const val KEY_EMAIL_CODE_HASH = "email_code_hash"
        private const val KEY_EMAIL_CODE_EXPIRES_AT = "email_code_expires_at"
        private const val KEY_EMAIL_CODE_SENT_AT = "email_code_sent_at"
        private const val EMAIL_CODE_LENGTH = 6
        private const val EMAIL_CODE_TTL_MS = 15 * 60_000L
        private const val EMAIL_CODE_RESEND_COOLDOWN_MS = 30_000L

        private fun createSecurePrefs(context: Context): SharedPreferences {
            return try {
                val masterKey = MasterKey.Builder(context)
                    .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                    .build()
                val encrypted = EncryptedSharedPreferences.create(
                    context,
                    "solstice_secure_prefs",
                    masterKey,
                    EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                    EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
                )
                // One-time migrate from legacy plaintext prefs.
                val legacy = context.getSharedPreferences("solstice_prefs", Context.MODE_PRIVATE)
                if (legacy.all.isNotEmpty() && encrypted.all.isEmpty()) {
                    encrypted.edit().apply {
                        legacy.all.forEach { (key, value) ->
                            when (value) {
                                is String -> putString(key, value)
                                is Boolean -> putBoolean(key, value)
                                is Int -> putInt(key, value)
                                is Long -> putLong(key, value)
                                is Float -> putFloat(key, value)
                            }
                        }
                        apply()
                    }
                    legacy.edit().clear().apply()
                }
                encrypted
            } catch (_: Exception) {
                context.getSharedPreferences("solstice_prefs", Context.MODE_PRIVATE)
            }
        }
    }
}
