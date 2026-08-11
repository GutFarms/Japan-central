package com.nativepure.companion.data

import com.nativepure.companion.mail.MailApiClient
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

    private val storeDir: File = LocalDataStore.dataDirectory()
    private val storeFile: File = LocalDataStore.storeFile()
    private val encryptedStoreFile: File = LocalDataStore.encryptedStoreFile()

    private var products: MutableList<Product> = mutableListOf()
    private var customers: MutableList<Customer> = mutableListOf()
    private var orders: MutableList<Order> = mutableListOf()
    private var orderLines: MutableList<OrderLine> = mutableListOf()
    private var cart: MutableList<CartItem> = mutableListOf()
    private var productRequests: MutableList<ProductRequest> = mutableListOf()
    private var sessionCustomerId: String? = null
    var ageVerified: Boolean = false
        private set
    /** One-time bootstrap password written for a fresh admin install (tests / first run). */
    var lastBootstrapAdminPassword: String? = null
        private set

    private val failedLogins = ConcurrentHashMap<String, Int>()
    private val lockoutUntil = ConcurrentHashMap<String, Long>()
    private var lastActivityAtMs: Long = System.currentTimeMillis()
    private var sessionLocked: Boolean = false

    private var emailCodeCustomerId: String? = null
    private var emailCodeSalt: String = ""
    private var emailCodeHash: String = ""
    private var emailCodeExpiresAt: Long = 0L
    private var emailCodeSentAt: Long = 0L
    private var lastIssuedEmailCode: EmailCodeIssue? = null
    private var emailCodeVerifyAttempts: Int = 0

    init {
        loadOrSeed()
        touchActivity()
    }

    /** Absolute path where inventory + customers are saved on the hard drive. */
    fun localDataFolderPath(): String = LocalDataStore.dataDirectoryPath()

    fun openLocalDataFolder(): Boolean = LocalDataStore.openInFileManager()

    /** Force rewrite of store + inventory/customer backup files. */
    fun saveAllToHardDrive(): OpResult {
        return try {
            persist()
            OpResult.Success(
                "Saved inventory (${products.size}) and customers (${customers.size}) to ${localDataFolderPath()}"
            )
        } catch (t: Throwable) {
            OpResult.Error(t.message ?: "Could not save to hard drive.")
        }
    }

    fun touchActivity() {
        lastActivityAtMs = System.currentTimeMillis()
        if (!sessionLocked) return
    }

    fun isSessionLocked(): Boolean {
        val me = sessionCustomerId ?: return false
        if (customers.none { it.id == me }) return false
        if (sessionLocked) return true
        if (System.currentTimeMillis() - lastActivityAtMs >= Privacy.IDLE_LOCK_MS) {
            sessionLocked = true
            return true
        }
        return false
    }

    fun unlockSession(password: String): AuthResult {
        val id = sessionCustomerId ?: return AuthResult.Error("Not signed in.")
        val customer = customers.find { it.id == id } ?: return AuthResult.Error("Not signed in.")
        if (!PasswordHasher.matches(password, customer.passwordSalt, customer.passwordHash)) {
            recordFailed(customer.email)
            return AuthResult.Error("Incorrect password.")
        }
        sessionLocked = false
        touchActivity()
        return AuthResult.Success(customer.toProfile())
    }

    fun lockSessionNow() {
        if (sessionCustomerId != null) sessionLocked = true
    }

    fun currentCustomer(): CustomerProfile? =
        sessionCustomerId?.let { id -> customers.find { it.id == id }?.toProfile() }

    fun allProducts(): List<Product> = products.toList()

    fun catalogProducts(): List<Product> = products.filter { it.published }

    fun featuredProducts(): List<Product> =
        catalogProducts().filter { it.featured }

    fun dealProducts(): List<Product> =
        catalogProducts().filter { it.hasActiveDeal }.sortedByDescending { it.dealPercent }

    fun staffAccounts(): List<CustomerProfile> =
        customers.filter { it.role == AccountRole.STAFF }.map { it.toProfile() }

    fun cartSummary(): CartSummary {
        // Resolve against full catalog so staff POS can ring unpublished draft stock if needed.
        val byId = products.associateBy { it.id }
        val lines = cart.mapNotNull { item ->
            byId[item.productId]?.let { CartLine(it, item.quantity) }
        }
        val subtotal = lines.sumOf { it.lineTotal }
        val tax = subtotal * TAX_RATE
        return CartSummary(lines, subtotal, tax, subtotal + tax, lines.sumOf { it.quantity })
    }

    /** Products available on the register: published + in stock, filtered by name/SKU/brand. */
    fun posSearchProducts(query: String): List<Product> {
        val q = query.trim().lowercase()
        val base = products.filter { it.inStock && (it.published || currentCustomer()?.role?.canManageInventory == true) }
        if (q.isBlank()) {
            return base.sortedWith(compareBy({ it.category.label }, { it.name }))
        }
        return base.filter { product ->
            product.name.lowercase().contains(q) ||
                product.brand.lowercase().contains(q) ||
                product.sku.lowercase().contains(q) ||
                product.category.label.lowercase().contains(q) ||
                product.id.lowercase().contains(q)
        }.sortedBy { it.name }
    }

    fun findCustomersForPos(query: String): List<CustomerProfile> {
        val me = currentCustomer() ?: return emptyList()
        if (!me.role.canViewSensitiveInfo) return emptyList()
        val q = query.trim().lowercase()
        if (q.length < 2) return emptyList()
        return customers
            .filter { it.role == AccountRole.CUSTOMER && it.enabled }
            .filter {
                it.fullName.lowercase().contains(q) ||
                    it.email.lowercase().contains(q) ||
                    it.phone.contains(q) ||
                    it.username.lowercase().contains(q)
            }
            .map { it.toProfile() }
            .take(12)
    }

    fun openPickupQueue(): List<Order> {
        val me = currentCustomer() ?: return emptyList()
        if (!me.role.canViewSensitiveInfo) return emptyList()
        return orders
            .filter { it.status == OrderStatus.READY }
            .sortedBy { it.createdAt }
    }

    fun todaysPosSales(): List<Order> {
        val me = currentCustomer() ?: return emptyList()
        if (!me.role.canViewSensitiveInfo) return emptyList()
        val startOfDay = java.util.Calendar.getInstance().apply {
            set(java.util.Calendar.HOUR_OF_DAY, 0)
            set(java.util.Calendar.MINUTE, 0)
            set(java.util.Calendar.SECOND, 0)
            set(java.util.Calendar.MILLISECOND, 0)
        }.timeInMillis
        return orders
            .filter { it.channel == SaleChannel.POS.name && it.createdAt >= startOfDay }
            .sortedByDescending { it.createdAt }
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
            return AuthResult.Error("Invalid email/username or password.")
        }

        if (!customer.enabled) {
            return AuthResult.Error("This account has been disabled. Contact an admin.")
        }

        if (!PasswordHasher.matches(password, customer.passwordSalt, customer.passwordHash)) {
            recordFailed(raw)
            val until = lockoutUntil[raw.lowercase()] ?: 0L
            return if (until > System.currentTimeMillis()) {
                AuthResult.Error("Too many failed attempts. Account locked for 5 minutes.")
            } else {
                AuthResult.Error("Invalid email/username or password.")
            }
        }

        failedLogins.remove(raw.lowercase())
        lockoutUntil.remove(raw.lowercase())
        var updated = customer.copy(lastLoginAt = System.currentTimeMillis())
        // Upgrade legacy hashes to PBKDF2 on successful login.
        if (PasswordHasher.needsRehash(customer.passwordHash)) {
            val salt = PasswordHasher.newSalt()
            updated = updated.copy(
                passwordHash = PasswordHasher.hash(password, salt),
                passwordSalt = salt
            )
        }
        replaceCustomer(updated)
        sessionCustomerId = updated.id
        sessionLocked = false
        touchActivity()
        if (!updated.emailVerified) {
            lastIssuedEmailCode = issueCodeFor(updated)
        }
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
            role = AccountRole.CUSTOMER,
            emailVerified = false
        )
        customers.add(customer)
        sessionCustomerId = customer.id
        lastIssuedEmailCode = issueCodeFor(customer)
        persist()
        return AuthResult.Success(customer.toProfile())
    }

    fun peekIssuedEmailCode(): EmailCodeIssue? {
        val issued = lastIssuedEmailCode ?: return null
        // Never surface plaintext codes in the UI — even offline.
        return issued.copy(code = "")
    }

    fun resendEmailVerificationCode(): AuthResult {
        val existing = customers.find { it.id == sessionCustomerId }
            ?: return AuthResult.Error("Not signed in.")
        if (existing.emailVerified) return AuthResult.Error("Email is already verified.")
        val waitMs = EMAIL_CODE_RESEND_COOLDOWN_MS - (System.currentTimeMillis() - emailCodeSentAt)
        if (waitMs > 0) {
            val secs = ((waitMs + 999) / 1000).coerceAtLeast(1)
            return AuthResult.Error("Wait ${secs}s before requesting another code.")
        }
        lastIssuedEmailCode = issueCodeFor(existing)
        return AuthResult.Success(existing.toProfile())
    }

    fun verifyEmailCode(code: String): AuthResult {
        val existing = customers.find { it.id == sessionCustomerId }
            ?: return AuthResult.Error("Not signed in.")
        if (existing.emailVerified) return AuthResult.Success(existing.toProfile())
        val clean = code.trim().filter { it.isDigit() }
        if (clean.length != 6) return AuthResult.Error("Enter the 6-digit code from your email.")
        if (emailCodeCustomerId != existing.id || emailCodeHash.isBlank() || emailCodeSalt.isBlank()) {
            return AuthResult.Error("No verification code on file. Resend a code.")
        }
        if (System.currentTimeMillis() > emailCodeExpiresAt) {
            clearEmailCode()
            return AuthResult.Error("That code expired. Resend a code.")
        }
        if (emailCodeVerifyAttempts >= 5) {
            clearEmailCode()
            lastIssuedEmailCode = null
            return AuthResult.Error("Too many incorrect codes. Resend a new code.")
        }
        if (!PasswordHasher.matchesVerificationCode(clean, emailCodeSalt, emailCodeHash)) {
            emailCodeVerifyAttempts++
            return AuthResult.Error("Incorrect code. Check the email and try again.")
        }
        val updated = existing.copy(emailVerified = true)
        replaceCustomer(updated)
        clearEmailCode()
        lastIssuedEmailCode = null
        persist()
        return AuthResult.Success(updated.toProfile())
    }

    fun visibleProductRequests(): List<ProductRequest> {
        val me = currentCustomer() ?: return emptyList()
        return if (me.role.canViewSensitiveInfo) {
            productRequests.sortedByDescending { it.createdAt }
        } else {
            productRequests.filter { it.customerId == me.id }.sortedByDescending { it.createdAt }
        }
    }

    fun requestBoxStats(): RequestBoxStats {
        val customerCount = customers.count { it.role == AccountRole.CUSTOMER }
        val unique = productRequests.map { it.customerId }.toSet().size
        val pct = if (customerCount <= 0) 0f else unique * 100f / customerCount
        return RequestBoxStats(
            totalRequests = productRequests.size,
            uniqueRequesters = unique,
            totalCustomers = customerCount,
            requesterToCustomerPercent = pct
        )
    }

    fun submitProductRequest(productName: String, notes: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        val name = productName.trim()
        if (name.length < 2) return OpResult.Error("Tell us what product you’re looking for.")
        productRequests.add(
            0,
            ProductRequest(
                id = "req-" + UUID.randomUUID().toString().take(8),
                customerId = me.id,
                customerName = me.fullName,
                customerEmail = me.email,
                productName = name,
                notes = notes.trim()
            )
        )
        persist()
        return OpResult.Success("Request sent for “$name”.")
    }

    fun markRequestFulfilled(id: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canViewSensitiveInfo) return OpResult.Error("Staff/admin only.")
        val idx = productRequests.indexOfFirst { it.id == id }
        if (idx < 0) return OpResult.Error("Request not found.")
        productRequests[idx] = productRequests[idx].copy(status = "Fulfilled")
        persist()
        return OpResult.Success("Marked fulfilled.")
    }

    fun createDraftFromRequest(requestId: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only staff/admin can create drafts from requests.")
        }
        val idx = productRequests.indexOfFirst { it.id == requestId }
        if (idx < 0) return OpResult.Error("Request not found.")
        val request = productRequests[idx]
        val id = "reqprod-" + UUID.randomUUID().toString().take(6)
        products.add(
            Product(
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
        )
        productRequests[idx] = request.copy(status = "Fulfilled")
        persist()
        return OpResult.Success(
            "Created draft “${request.productName}” in Stock (unpublished). Request marked fulfilled."
        )
    }

    fun logout() {
        sessionCustomerId = null
        sessionLocked = false
        cart.clear()
        lastIssuedEmailCode = null
        clearEmailCode()
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
            emailVerified = true,
            notes = "Staff sub-account created by ${admin.email}"
        )
        customers.add(staff)
        persist()
        return AuthResult.Success(staff.toProfile())
    }

    fun addToCart(productId: String, quantity: Int = 1) {
        val product = products.find { it.id == productId } ?: return
        val me = currentCustomer()
        if (!product.published && me?.role?.canManageInventory != true) return
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
        products[idx] = if (product.sizeInventoryEnabled) {
            val next = (product.stockEighth + delta).coerceAtLeast(0)
            product.copy(stockEighth = next).normalizedSizeInventory()
        } else {
            val next = (product.stockQuantity + delta).coerceAtLeast(0)
            product.copy(stockQuantity = next, inStock = next > 0)
        }
        persist()
        return true
    }

    fun setPublished(productId: String, published: Boolean): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) return OpResult.Error("Only staff/admin can publish.")
        val idx = products.indexOfFirst { it.id == productId }
        if (idx < 0) return OpResult.Error("Product not found.")
        val product = products[idx]
        if (published) {
            when {
                product.sku.isBlank() -> return OpResult.Error("Add a SKU before publishing.")
                product.sizeInventoryEnabled &&
                    listOf(product.priceGram, product.priceEighth, product.priceQuarter, product.priceOunce)
                        .none { it > 0 } ->
                    return OpResult.Error("Set at least one size price before publishing.")
                !product.sizeInventoryEnabled && product.price <= 0.0 ->
                    return OpResult.Error("Set a price greater than \$0 before publishing.")
            }
        }
        products[idx] = product.copy(
            published = published,
            publishedAt = if (published) System.currentTimeMillis() else 0L,
            publishedBy = if (published) me.email else ""
        )
        persist()
        return OpResult.Success(
            if (published) "Published ${product.name}" else "Unpublished ${product.name}"
        )
    }

    fun saveProduct(product: Product): OpResult {
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
            val prices = if (product.priceEighth <= 0 && price > 0) SizePricing.fromEighth(price) else null
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
                withSizes.sizeInventoryEnabled &&
                    listOf(withSizes.priceGram, withSizes.priceEighth, withSizes.priceQuarter, withSizes.priceOunce)
                        .none { it > 0 } ->
                    return OpResult.Error("Set at least one size price before publishing.")
                !withSizes.sizeInventoryEnabled && withSizes.price <= 0.0 ->
                    return OpResult.Error("Set a price greater than \$0 before publishing.")
            }
        }
        val idx = products.indexOfFirst { it.id == product.id }
        val existing = if (idx >= 0) products[idx] else null
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
            publishedBy = if (product.published) me.email else ""
        )
        if (idx >= 0) products[idx] = cleaned else products.add(cleaned)
        persist()
        return OpResult.Success(
            if (existing == null) "Created “${cleaned.name}”." else "Saved changes to “${cleaned.name}”."
        )
    }

    fun createBlankDraftProduct(): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) {
            return OpResult.Error("Only admin and staff can add products.")
        }
        val id = "draft-" + UUID.randomUUID().toString().take(8)
        val product = Product(
            id = id,
            name = "New product",
            brand = "Native Pure",
            category = ProductCategory.FLOWER,
            strainType = StrainType.HYBRID,
            thcPercent = 0.0,
            cbdPercent = 0.0,
            price = 0.0,
            unitLabel = "each",
            description = "",
            effects = "—",
            featured = false,
            inStock = false,
            stockQuantity = 0,
            sku = "",
            published = false
        )
        products.add(product)
        persist()
        return OpResult.Success("Draft “${product.name}” created — edit price and SKU, then publish.")
    }

    fun setStaffEnabled(staffId: String, enabled: Boolean): OpResult {
        val admin = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!admin.role.canManageStaff) return OpResult.Error("Only admin can manage staff.")
        val idx = customers.indexOfFirst { it.id == staffId }
        if (idx < 0) return OpResult.Error("Staff not found.")
        val staff = customers[idx]
        if (staff.role != AccountRole.STAFF) return OpResult.Error("Only staff accounts can be toggled.")
        customers[idx] = staff.copy(enabled = enabled)
        persist()
        return OpResult.Success(if (enabled) "Enabled ${staff.email}" else "Disabled ${staff.email}")
    }

    fun resetStaffPassword(staffId: String, newPassword: String): OpResult {
        val admin = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!admin.role.canManageStaff) return OpResult.Error("Only admin can reset staff passwords.")
        PasswordPolicy.validatePassword(newPassword)?.let { return OpResult.Error(it) }
        val idx = customers.indexOfFirst { it.id == staffId }
        if (idx < 0) return OpResult.Error("Staff not found.")
        val staff = customers[idx]
        if (staff.role != AccountRole.STAFF) return OpResult.Error("Only staff passwords can be reset.")
        val salt = PasswordHasher.newSalt()
        customers[idx] = staff.copy(
            passwordHash = PasswordHasher.hash(newPassword, salt),
            passwordSalt = salt,
            mustChangePassword = true
        )
        persist()
        return OpResult.Success("Password reset for ${staff.email}")
    }

    fun exportSyncJson(): String {
        val me = currentCustomer()
        require(me?.role?.canManageInventory == true) { "Only staff/admin can export." }
        // Never export password hashes/salts — credentials stay on each device.
        return json.encodeToString(
            SyncFile(
                format = "nativepure-sync-v1",
                exportedAt = System.currentTimeMillis(),
                products = products.toList(),
                customers = customers.map { it.toSyncCustomer() },
                orders = orders.toList(),
                orderLines = orderLines.toList()
            )
        )
    }

    fun importSyncJson(raw: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canManageInventory) return OpResult.Error("Only staff/admin can import.")
        return try {
            val parsed = json.decodeFromString<SyncFile>(raw)
            require(parsed.format == "nativepure-sync-v1") { "Unsupported sync format." }
            parsed.products.forEach { incoming ->
                val idx = products.indexOfFirst { it.id == incoming.id }
                if (idx >= 0) products[idx] = incoming else products.add(incoming)
            }
            var customersMerged = 0
            parsed.customers.forEach { incoming ->
                // Never overwrite the signed-in account mid-session with a stale copy.
                if (incoming.id == me.id) return@forEach
                val idx = customers.indexOfFirst {
                    it.id == incoming.id || it.email.equals(incoming.email, ignoreCase = true)
                }
                if (idx >= 0) {
                    val existing = customers[idx]
                    // Preserve credentials and role — sync cannot escalate privileges or steal passwords.
                    customers[idx] = existing.copy(
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
                } else {
                    val salt = PasswordHasher.newSalt()
                    customers.add(
                        Customer(
                            id = incoming.id.ifBlank { "cust-" + UUID.randomUUID().toString().take(8) },
                            email = incoming.email.trim().lowercase(),
                            username = incoming.username,
                            passwordHash = PasswordHasher.hash(PasswordHasher.randomUnusableSecret(), salt),
                            passwordSalt = salt,
                            fullName = incoming.fullName.trim().ifBlank { incoming.email.substringBefore("@") },
                            phone = incoming.phone,
                            dateOfBirth = incoming.dateOfBirth,
                            createdAt = if (incoming.createdAt > 0) incoming.createdAt else System.currentTimeMillis(),
                            lastLoginAt = incoming.lastLoginAt,
                            notes = incoming.notes,
                            marketingOptIn = incoming.marketingOptIn,
                            role = AccountRole.CUSTOMER, // never import elevated roles
                            createdByAdminId = "",
                            mustChangePassword = true,
                            enabled = incoming.enabled,
                            emailVerified = incoming.emailVerified,
                            loyaltyPoints = incoming.loyaltyPoints.coerceAtLeast(0),
                            lifetimeSpend = incoming.lifetimeSpend.coerceAtLeast(0.0)
                        )
                    )
                }
                customersMerged++
            }
            parsed.orders.forEach { incoming ->
                if (orders.none { it.id == incoming.id }) orders.add(incoming)
            }
            parsed.orderLines.forEach { incoming ->
                val exists = orderLines.any {
                    it.orderId == incoming.orderId &&
                        it.productId == incoming.productId &&
                        it.quantity == incoming.quantity &&
                        it.unitPrice == incoming.unitPrice
                }
                if (!exists) orderLines.add(incoming)
            }
            persist()
            OpResult.Success(
                "Imported ${parsed.products.size} products and $customersMerged customers (no passwords) to hard drive."
            )
        } catch (t: Throwable) {
            OpResult.Error(t.message ?: "Import failed.")
        }
    }

    fun placePickupOrder(pickupName: String, notes: String, redeemPoints: Int = 0): Order? {
        val summary = cartSummary()
        if (summary.lines.isEmpty()) return null
        val customer = currentCustomer()
        val customerRow = customer?.id?.let { id -> customers.find { it.id == id } }
        val canRedeem = customerRow?.role == AccountRole.CUSTOMER
        val maxRedeem = if (canRedeem && customerRow != null) {
            LoyaltyPoints.maxRedeemablePoints(customerRow.loyaltyPoints, summary.total)
        } else {
            0
        }
        val appliedRedeem = redeemPoints.coerceIn(0, maxRedeem)
            .let { it - (it % LoyaltyPoints.REDEEM_POINTS_PER_DOLLAR) }
        val discount = LoyaltyPoints.discountForPoints(appliedRedeem)
        val total = (summary.total - discount).coerceAtLeast(0.0)
        val pointsEarned = if (canRedeem) LoyaltyPoints.pointsForSpend(total) else 0
        val order = Order(
            id = UUID.randomUUID().toString().take(8).uppercase(),
            createdAt = System.currentTimeMillis(),
            total = total,
            itemCount = summary.itemCount,
            status = OrderStatus.READY,
            pickupName = pickupName.ifBlank { customer?.fullName ?: "Guest" },
            notes = notes.trim(),
            customerId = customer?.id.orEmpty(),
            customerEmail = customer?.email.orEmpty(),
            pointsEarned = pointsEarned,
            pointsRedeemed = appliedRedeem,
            discount = discount,
            channel = SaleChannel.PICKUP.name,
            cashierId = customer?.id.orEmpty(),
            cashierName = customer?.fullName.orEmpty()
        )
        val lines = summary.lines.map {
            OrderLine(order.id, it.product.id, it.product.name, it.product.effectivePrice, it.quantity)
        }
        orders.add(0, order)
        orderLines.addAll(lines)
        summary.lines.forEach { line ->
            adjustStockInternal(line.product.id, -line.quantity)
        }
        if (customerRow != null && (pointsEarned > 0 || appliedRedeem > 0)) {
            replaceCustomer(
                customerRow.copy(
                    loyaltyPoints = (customerRow.loyaltyPoints - appliedRedeem + pointsEarned)
                        .coerceAtLeast(0),
                    lifetimeSpend = customerRow.lifetimeSpend + total
                )
            )
        }
        cart.clear()
        persist()
        return order
    }

    /**
     * Complete an in-store POS sale from the current cart.
     * Staff/admin only. Marks the order as picked up immediately (walk-out).
     */
    fun completePosSale(request: PosSaleRequest): OpResult {
        val cashier = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!cashier.role.canViewSensitiveInfo) {
            return OpResult.Error("Only staff/admin can ring POS sales.")
        }
        val summary = cartSummary()
        if (summary.lines.isEmpty()) return OpResult.Error("Ticket is empty.")

        val loyaltyRow = request.loyaltyCustomerId
            ?.takeIf { it.isNotBlank() }
            ?.let { id -> customers.find { it.id == id && it.role == AccountRole.CUSTOMER && it.enabled } }

        val maxRedeem = if (loyaltyRow != null) {
            LoyaltyPoints.maxRedeemablePoints(loyaltyRow.loyaltyPoints, summary.total)
        } else {
            0
        }
        val appliedRedeem = request.redeemPoints.coerceIn(0, maxRedeem)
            .let { it - (it % LoyaltyPoints.REDEEM_POINTS_PER_DOLLAR) }
        val discount = LoyaltyPoints.discountForPoints(appliedRedeem)
        val total = (summary.total - discount).coerceAtLeast(0.0)
        val pointsEarned = if (loyaltyRow != null) LoyaltyPoints.pointsForSpend(total) else 0

        val tendered = when (request.paymentMethod) {
            PaymentMethod.CASH -> request.amountTendered
            PaymentMethod.CARD, PaymentMethod.OTHER -> total
        }
        if (request.paymentMethod == PaymentMethod.CASH && tendered + 0.001 < total) {
            return OpResult.Error(
                "Cash tendered ($${"%.2f".format(tendered)}) is less than total ($${"%.2f".format(total)})."
            )
        }
        val change = if (request.paymentMethod == PaymentMethod.CASH) {
            (tendered - total).coerceAtLeast(0.0)
        } else {
            0.0
        }

        val guestName = request.customerName.trim().ifBlank {
            loyaltyRow?.fullName ?: "Walk-in"
        }
        val order = Order(
            id = UUID.randomUUID().toString().take(8).uppercase(),
            createdAt = System.currentTimeMillis(),
            total = total,
            itemCount = summary.itemCount,
            status = OrderStatus.PICKED_UP,
            pickupName = guestName,
            notes = request.notes.trim(),
            customerId = loyaltyRow?.id.orEmpty(),
            customerEmail = loyaltyRow?.email.orEmpty(),
            pointsEarned = pointsEarned,
            pointsRedeemed = appliedRedeem,
            discount = discount,
            paymentMethod = request.paymentMethod.name,
            amountTendered = tendered,
            changeDue = change,
            channel = SaleChannel.POS.name,
            cashierId = cashier.id,
            cashierName = cashier.fullName
        )
        val lines = summary.lines.map {
            OrderLine(order.id, it.product.id, it.product.name, it.product.effectivePrice, it.quantity)
        }
        orders.add(0, order)
        orderLines.addAll(lines)
        summary.lines.forEach { line ->
            adjustStockInternal(line.product.id, -line.quantity)
        }
        if (loyaltyRow != null && (pointsEarned > 0 || appliedRedeem > 0 || total > 0)) {
            replaceCustomer(
                loyaltyRow.copy(
                    loyaltyPoints = (loyaltyRow.loyaltyPoints - appliedRedeem + pointsEarned)
                        .coerceAtLeast(0),
                    lifetimeSpend = loyaltyRow.lifetimeSpend + total
                )
            )
        }
        cart.clear()
        persist()
        val receipt = orderReceiptText(order.id).orEmpty()
        val changeNote = if (change > 0) " Change due $${"%.2f".format(change)}." else ""
        return OpResult.Success("Sale ${order.id} complete · $${"%.2f".format(total)}$changeNote\n\n$receipt")
    }

    fun updateOrderStatus(orderId: String, status: String): OpResult {
        val me = currentCustomer() ?: return OpResult.Error("Not signed in.")
        if (!me.role.canViewSensitiveInfo) {
            return OpResult.Error("Only staff/admin can update order status.")
        }
        val idx = orders.indexOfFirst { it.id == orderId }
        if (idx < 0) return OpResult.Error("Order not found.")
        val clean = status.trim()
        if (clean !in OrderStatus.staffActions) return OpResult.Error("Unknown status.")
        orders[idx] = orders[idx].copy(status = clean)
        persist()
        return OpResult.Success("Order $orderId marked $clean.")
    }

    fun orderReceiptText(orderId: String): String? {
        val me = currentCustomer() ?: return null
        val order = orders.find { it.id == orderId } ?: return null
        if (!me.role.canViewSensitiveInfo && order.customerId != me.id) return null
        val lines = orderLines.filter { it.orderId == orderId }
        val isPos = order.channel == SaleChannel.POS.name
        val sb = StringBuilder()
        sb.appendLine(if (isPos) "Native Pure — POS sale receipt" else "Native Pure — Pickup receipt")
        sb.appendLine("Order #${order.id}")
        sb.appendLine("Status: ${order.status}")
        if (isPos) {
            sb.appendLine("Sold to: ${order.pickupName}")
            if (order.cashierName.isNotBlank()) sb.appendLine("Cashier: ${order.cashierName}")
            if (order.paymentMethod.isNotBlank()) {
                val method = runCatching { PaymentMethod.valueOf(order.paymentMethod).label }
                    .getOrDefault(order.paymentMethod)
                sb.appendLine("Tender: $method")
                if (order.paymentMethod == PaymentMethod.CASH.name) {
                    sb.appendLine("Cash tendered: $${"%.2f".format(order.amountTendered)}")
                    if (order.changeDue > 0) sb.appendLine("Change: $${"%.2f".format(order.changeDue)}")
                }
            }
        } else {
            sb.appendLine("Pickup: ${order.pickupName}")
        }
        // Mask email on printable/shareable receipts to limit PII exposure.
        if (order.customerEmail.isNotBlank()) {
            sb.appendLine("Email: ${Privacy.maskEmail(order.customerEmail)}")
        }
        sb.appendLine()
        lines.forEach { line ->
            sb.appendLine(
                "${line.quantity} × ${line.productName} @ $${"%.2f".format(line.unitPrice)} = $${
                    "%.2f".format(line.unitPrice * line.quantity)
                }"
            )
        }
        sb.appendLine()
        if (order.discount > 0) {
            sb.appendLine("Points redeemed: ${order.pointsRedeemed} (−$${"%.2f".format(order.discount)})")
        }
        sb.appendLine("Total paid: $${"%.2f".format(order.total)}")
        if (order.pointsEarned > 0) sb.appendLine("Points earned: +${order.pointsEarned}")
        // Staff notes stay on-device in the order record; omit from shared receipt text.
        sb.appendLine()
        sb.appendLine(if (isPos) "Thank you — 18+ only. Valid ID required." else "Bring a valid ID for pickup. 18+ only.")
        return sb.toString()
    }

    private fun adjustStockInternal(productId: String, delta: Int) {
        val idx = products.indexOfFirst { it.id == productId }
        if (idx < 0) return
        val product = products[idx]
        products[idx] = if (product.sizeInventoryEnabled) {
            val next = (product.stockEighth + delta).coerceAtLeast(0)
            product.copy(stockEighth = next).normalizedSizeInventory()
        } else {
            val next = (product.stockQuantity + delta).coerceAtLeast(0)
            product.copy(stockQuantity = next, inStock = next > 0)
        }
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
        persistSecurityOnly()
    }

    /** Persist lockouts without rewriting the whole catalog when possible. */
    private fun persistSecurityOnly() {
        runCatching { persist() }
    }

    private fun replaceCustomer(updated: Customer) {
        val idx = customers.indexOfFirst { it.id == updated.id }
        if (idx >= 0) customers[idx] = updated else customers.add(updated)
    }

    private fun loadOrSeed() {
        val loaded = readPersistedStore()
        if (loaded != null) {
            products = loaded.products.toMutableList()
            customers = loaded.customers.toMutableList()
            orders = loaded.orders.toMutableList()
            orderLines = loaded.orderLines.toMutableList()
            cart = loaded.cart.toMutableList()
            productRequests = loaded.productRequests.toMutableList()
            // Staff/admin sessions are never restored across restarts.
            sessionCustomerId = loaded.sessionCustomerId?.let { id ->
                customers.find { it.id == id && it.role == AccountRole.CUSTOMER }?.id
            }
            ageVerified = loaded.ageVerified
            failedLogins.clear()
            failedLogins.putAll(loaded.failedLogins)
            lockoutUntil.clear()
            lockoutUntil.putAll(loaded.lockoutUntil)
            if (products.isEmpty()) products = SeedCatalog.products.toMutableList()
            ensureMainAdmin()
            ensureDemoCustomer()
            persist()
        } else {
            seedFresh()
        }
    }

    private fun readPersistedStore(): PersistedStore? {
        // Prefer encrypted store; migrate plaintext store.json if present.
        StoreEncryption.decryptFromFile(encryptedStoreFile)?.let { text ->
            return runCatching { json.decodeFromString<PersistedStore>(text) }.getOrNull()
        }
        if (storeFile.isFile) {
            val text = storeFile.readText()
            val data = runCatching { json.decodeFromString<PersistedStore>(text) }.getOrNull()
            if (data != null) {
                // Migrate to encrypted form and remove plaintext store when possible.
                runCatching {
                    StoreEncryption.encryptToFile(json.encodeToString(data), encryptedStoreFile)
                    storeFile.delete()
                }
                return data
            }
        }
        return null
    }

    private fun seedFresh() {
        storeDir.mkdirs()
        products = SeedCatalog.products.toMutableList()
        customers = mutableListOf()
        orders = mutableListOf()
        orderLines = mutableListOf()
        cart = mutableListOf()
        productRequests = mutableListOf()
        sessionCustomerId = null
        ageVerified = false
        ensureMainAdmin()
        ensureDemoCustomer()
        persist()
    }

    private fun ensureMainAdmin() {
        val existing = customers.find {
            it.email.equals(MAIN_ADMIN_EMAIL, true) || it.username == MAIN_ADMIN_USERNAME
        }
        if (existing != null) {
            var next = existing
            if (existing.role != AccountRole.ADMIN) {
                next = next.copy(role = AccountRole.ADMIN)
            }
            if (!existing.enabled) {
                next = next.copy(enabled = true)
            }
            if (existing.username.isBlank()) {
                next = next.copy(username = MAIN_ADMIN_USERNAME)
            }
            if (!existing.emailVerified) {
                next = next.copy(emailVerified = true)
            }
            if (next != existing) {
                replaceCustomer(next)
                persist()
            }
            return
        }
        val bootstrap = System.getProperty("nativepure.companion.adminPassword")
            ?.takeIf { it.isNotBlank() }
            ?: PasswordHasher.randomBootstrapPassword()
        lastBootstrapAdminPassword = bootstrap
        val salt = PasswordHasher.newSalt()
        customers.add(
            Customer(
                id = MAIN_ADMIN_ID,
                email = MAIN_ADMIN_EMAIL,
                username = MAIN_ADMIN_USERNAME,
                passwordHash = PasswordHasher.hash(bootstrap, salt),
                passwordSalt = salt,
                fullName = "Main Admin",
                role = AccountRole.ADMIN,
                notes = "Primary admin — change password on first login",
                mustChangePassword = true,
                emailVerified = true,
                enabled = true
            )
        )
        runCatching {
            LocalDataStore.writeAtomic(
                LocalDataStore.adminSetupFile(),
                buildString {
                    appendLine("Native Pure POS — one-time admin setup")
                    appendLine("Delete this file after first login.")
                    appendLine("Username: $MAIN_ADMIN_USERNAME")
                    appendLine("Email: $MAIN_ADMIN_EMAIL")
                    appendLine("Temporary password: $bootstrap")
                    appendLine("You will be required to change this password.")
                }
            )
        }
    }

    private fun ensureDemoCustomer() {
        val existing = customers.find { it.email.equals(DEMO_EMAIL, true) }
        if (existing != null) {
            if (!existing.emailVerified) {
                replaceCustomer(existing.copy(emailVerified = true))
                persist()
            }
            return
        }
        val salt = PasswordHasher.newSalt()
        customers.add(
            Customer(
                id = "cust-demo",
                email = DEMO_EMAIL,
                passwordHash = PasswordHasher.hash("demo1234", salt),
                passwordSalt = salt,
                fullName = "Demo Customer",
                phone = "",
                dateOfBirth = "",
                marketingOptIn = false,
                role = AccountRole.CUSTOMER,
                notes = "",
                emailVerified = true
            )
        )
    }

    private fun issueCodeFor(customer: Customer): EmailCodeIssue {
        val secure = java.security.SecureRandom()
        val code = secure.nextInt(1_000_000).toString().padStart(6, '0')
        val salt = PasswordHasher.newSalt()
        emailCodeCustomerId = customer.id
        emailCodeSalt = salt
        emailCodeHash = PasswordHasher.hashVerificationCode(code, salt)
        emailCodeExpiresAt = System.currentTimeMillis() + EMAIL_CODE_TTL_MS
        emailCodeSentAt = System.currentTimeMillis()
        emailCodeVerifyAttempts = 0
        val mail = MailApiClient.sendVerificationCodeBlocking(
            email = customer.email,
            code = code,
            expiresInMinutes = (EMAIL_CODE_TTL_MS / 60_000L).toInt().coerceAtLeast(5)
        )
        if (!mail.ok) {
            // Offline fallback: write to a local dev file — never the UI.
            runCatching {
                LocalDataStore.writeAtomic(
                    LocalDataStore.devEmailCodeFile(),
                    "DEV ONLY — delete after use\nemail=${customer.email}\ncode=$code\nexpires=$emailCodeExpiresAt\n"
                )
            }
        } else {
            runCatching { LocalDataStore.devEmailCodeFile().delete() }
        }
        return EmailCodeIssue(
            email = customer.email,
            code = code,
            expiresAtMs = emailCodeExpiresAt,
            deliveredByMail = mail.ok,
            mailError = mail.error
        )
    }

    private fun clearEmailCode() {
        emailCodeCustomerId = null
        emailCodeSalt = ""
        emailCodeHash = ""
        emailCodeExpiresAt = 0L
        emailCodeVerifyAttempts = 0
    }

    private fun persist() {
        storeDir.mkdirs()
        // Never persist staff/admin sessions — require login after restart.
        val durableSession = sessionCustomerId?.let { id ->
            customers.find { it.id == id && it.role == AccountRole.CUSTOMER }?.id
        }
        val data = PersistedStore(
            products = products.toList(),
            customers = customers.toList(),
            orders = orders.toList(),
            orderLines = orderLines.toList(),
            cart = if (durableSession != null) cart.toList() else emptyList(),
            productRequests = productRequests.toList(),
            sessionCustomerId = durableSession,
            ageVerified = ageVerified,
            failedLogins = failedLogins.toMap(),
            lockoutUntil = lockoutUntil.toMap()
        )
        val encoded = json.encodeToString(data)
        StoreEncryption.encryptToFile(encoded, encryptedStoreFile)
        // Remove legacy plaintext store if it still exists.
        runCatching { if (storeFile.exists()) storeFile.delete() }
        LocalDataStore.writeAtomic(
            LocalDataStore.inventoryBackupFile(),
            json.encodeToString(products.toList())
        )
        // Customer backup omits password hashes and full DOB/notes for staff ops copies.
        LocalDataStore.writeAtomic(
            LocalDataStore.customersBackupFile(),
            json.encodeToString(
                customers.map {
                    Privacy.redactedCustomerForBackup(it).copy(
                        dateOfBirth = "",
                        notes = "",
                        phone = Privacy.maskPhone(it.phone)
                    )
                }
            )
        )
    }

    companion object {
        const val TAX_RATE = 0.08
        const val MAIN_ADMIN_ID = "admin-main"
        const val MAIN_ADMIN_USERNAME = "admin"
        const val MAIN_ADMIN_EMAIL = "fidelgutierrez33@gmail.com"
        private const val DEMO_EMAIL = "demo@nativepure.example"
        private const val EMAIL_CODE_TTL_MS = 15 * 60_000L
        private const val EMAIL_CODE_RESEND_COOLDOWN_MS = 30_000L
    }
}
