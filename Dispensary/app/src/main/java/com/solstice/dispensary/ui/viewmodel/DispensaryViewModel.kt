package com.solstice.dispensary.ui.viewmodel

import android.graphics.Bitmap
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.solstice.dispensary.data.model.AuthResult
import com.solstice.dispensary.data.model.AutoLockTimeout
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.data.model.CustomerProfile
import com.solstice.dispensary.data.model.EmailCodeIssue
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.LabelScanResult
import com.solstice.dispensary.data.model.OpResult
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.ProductRequest
import com.solstice.dispensary.data.model.RequestBoxStats
import com.solstice.dispensary.data.model.ThemeMode
import com.solstice.dispensary.data.repository.DispensaryRepository
import com.solstice.dispensary.data.update.AppUpdateChecker
import com.solstice.dispensary.data.update.AppUpdateInfo
import com.solstice.dispensary.data.update.UpdateCheckResult
import com.solstice.dispensary.data.update.UpdateUiState
import com.solstice.dispensary.scan.LabelAiScanner
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.io.File

@OptIn(ExperimentalCoroutinesApi::class)
class DispensaryViewModel(
    private val repository: DispensaryRepository,
    private val updateChecker: AppUpdateChecker
) : ViewModel() {

    private val scanner = LabelAiScanner()

    var ageVerified by mutableStateOf(repository.isAgeVerified())
        private set

    var themeMode by mutableStateOf(repository.getThemeMode())
        private set

    var securitySettings by mutableStateOf(repository.getSecuritySettings())
        private set

    var currentCustomer by mutableStateOf<CustomerProfile?>(null)
        private set

    var authBusy by mutableStateOf(false)
        private set

    var authError by mutableStateOf<String?>(null)
        private set

    var pendingEmailCode by mutableStateOf<EmailCodeIssue?>(null)
        private set

    var updateState by mutableStateOf<UpdateUiState>(UpdateUiState.Idle)
        private set

    var downloadedApk by mutableStateOf<File?>(null)
        private set

    private var autoUpdateChecked = false

    var accountMessage by mutableStateOf<String?>(null)
        private set

    var appLocked by mutableStateOf(false)
        private set

    var lockError by mutableStateOf<String?>(null)
        private set

    var selectedCategory by mutableStateOf<ProductCategory?>(null)
        private set

    var searchQuery by mutableStateOf("")
        private set

    var lastPlacedOrder by mutableStateOf<Order?>(null)
        private set

    var checkoutMessage by mutableStateOf<String?>(null)
        private set

    var scanResult by mutableStateOf<LabelScanResult?>(null)
        private set

    var scanBusy by mutableStateOf(false)
        private set

    var scanError by mutableStateOf<String?>(null)
        private set

    var intakeMessage by mutableStateOf<String?>(null)
        private set

    private val categoryFilter = MutableStateFlow<ProductCategory?>(null)

    val products: StateFlow<List<Product>> = categoryFilter
        .flatMapLatest { repository.productsByCategory(it) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val inventory: StateFlow<List<Product>> = repository.inventory
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val intakes: StateFlow<List<InventoryIntake>> = repository.intakes
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val featured: StateFlow<List<Product>> = repository.featured
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val cart: StateFlow<CartSummary> = repository.cartSummary
        .stateIn(
            viewModelScope,
            SharingStarted.WhileSubscribed(5_000),
            CartSummary(emptyList(), 0.0, 0.0, 0.0, 0)
        )

    val orders: StateFlow<List<Order>> = repository.visibleOrders
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val customers: StateFlow<List<CustomerProfile>> = repository.customers
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val staffAccounts: StateFlow<List<CustomerProfile>> = repository.staffAccounts
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val productRequests: StateFlow<List<ProductRequest>> = repository.productRequests
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val requestBoxStats: StateFlow<RequestBoxStats> = repository.requestBoxStats
        .stateIn(
            viewModelScope,
            SharingStarted.WhileSubscribed(5_000),
            RequestBoxStats(0, 0, 0, 0f)
        )

    var lastSyncExport by mutableStateOf<String?>(null)
        private set

    var requestMessage by mutableStateOf<String?>(null)
        private set

    val isLoggedIn: Boolean get() = currentCustomer != null

    val mustChangePassword: Boolean
        get() = currentCustomer?.mustChangePassword == true

    val needsEmailVerification: Boolean
        get() = currentCustomer?.emailVerified == false

    val canViewSensitiveInfo: Boolean
        get() = currentCustomer?.role?.canViewSensitiveInfo == true

    val canManageStaff: Boolean
        get() = currentCustomer?.role?.canManageStaff == true

    val canManageInventory: Boolean
        get() = currentCustomer?.role?.canManageInventory == true

    init {
        viewModelScope.launch {
            currentCustomer = repository.currentCustomer()
            refreshSecuritySettings()
            if (currentCustomer != null && repository.getSecuritySettings().appLockEnabled) {
                appLocked = true
            }
            if (currentCustomer?.emailVerified == false && pendingEmailCode == null) {
                when (repository.resendEmailVerificationCode()) {
                    is AuthResult.Success -> captureIssuedEmailCode()
                    is AuthResult.Error -> Unit
                }
            }
        }
    }

    fun productFlow(id: String) = repository.product(id)

    fun orderLines(orderId: String): StateFlow<List<OrderLine>> =
        repository.orderLines(orderId)
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun verifyAge() {
        repository.setAgeVerified(true)
        ageVerified = true
    }

    fun updateThemeMode(mode: ThemeMode) {
        repository.setThemeMode(mode)
        themeMode = mode
    }

    fun refreshSecuritySettings() {
        securitySettings = repository.getSecuritySettings()
    }

    fun clearAuthError() {
        authError = null
    }

    private fun captureIssuedEmailCode() {
        pendingEmailCode = repository.peekIssuedEmailCode()
    }

    fun login(email: String, password: String) {
        viewModelScope.launch {
            authBusy = true
            authError = null
            when (val result = repository.login(email, password)) {
                is AuthResult.Success -> {
                    currentCustomer = result.customer
                    captureIssuedEmailCode()
                    refreshSecuritySettings()
                    appLocked = false
                    repository.clearBackgroundMark()
                }
                is AuthResult.Error -> authError = result.message
            }
            authBusy = false
        }
    }

    fun register(
        email: String,
        password: String,
        fullName: String,
        phone: String,
        dateOfBirth: String,
        marketingOptIn: Boolean
    ) {
        viewModelScope.launch {
            authBusy = true
            authError = null
            when (
                val result = repository.registerCustomer(
                    email, password, fullName, phone, dateOfBirth, marketingOptIn
                )
            ) {
                is AuthResult.Success -> {
                    currentCustomer = result.customer
                    captureIssuedEmailCode()
                    refreshSecuritySettings()
                    appLocked = false
                }
                is AuthResult.Error -> authError = result.message
            }
            authBusy = false
        }
    }

    fun verifyEmailCode(code: String) {
        viewModelScope.launch {
            authBusy = true
            authError = null
            when (val result = repository.verifyEmailCode(code)) {
                is AuthResult.Success -> {
                    currentCustomer = result.customer
                    pendingEmailCode = null
                }
                is AuthResult.Error -> authError = result.message
            }
            authBusy = false
        }
    }

    fun resendEmailVerificationCode() {
        viewModelScope.launch {
            authBusy = true
            authError = null
            when (val result = repository.resendEmailVerificationCode()) {
                is AuthResult.Success -> {
                    currentCustomer = result.customer
                    captureIssuedEmailCode()
                }
                is AuthResult.Error -> authError = result.message
            }
            authBusy = false
        }
    }

    fun logout() {
        repository.logout()
        currentCustomer = null
        accountMessage = null
        appLocked = false
        lockError = null
        pendingEmailCode = null
    }

    fun createStaffSubAccount(email: String, password: String, fullName: String) {
        viewModelScope.launch {
            when (val result = repository.createStaffSubAccount(email, password, fullName)) {
                is AuthResult.Success ->
                    accountMessage =
                        "Staff account created for ${result.customer.email}. They must change the temporary password on first login."
                is AuthResult.Error ->
                    accountMessage = result.message
            }
        }
    }

    fun saveProfile(
        fullName: String,
        phone: String,
        dateOfBirth: String,
        notes: String,
        marketingOptIn: Boolean
    ) {
        viewModelScope.launch {
            when (
                val result = repository.updateProfile(
                    fullName, phone, dateOfBirth, notes, marketingOptIn
                )
            ) {
                is AuthResult.Success -> {
                    currentCustomer = result.customer
                    accountMessage = "Profile saved."
                }
                is AuthResult.Error -> accountMessage = result.message
            }
        }
    }

    fun changePassword(current: String, newPassword: String, confirm: String) {
        viewModelScope.launch {
            when (val result = repository.changePassword(current, newPassword, confirm)) {
                is AuthResult.Success -> {
                    currentCustomer = result.customer
                    accountMessage = "Password updated."
                }
                is AuthResult.Error -> accountMessage = result.message
            }
        }
    }

    fun forceChangePassword(newPassword: String, confirm: String) {
        viewModelScope.launch {
            authBusy = true
            authError = null
            when (val result = repository.forceChangePassword(newPassword, confirm)) {
                is AuthResult.Success -> {
                    currentCustomer = result.customer
                    accountMessage = "Password updated. Your account is now secured."
                }
                is AuthResult.Error -> authError = result.message
            }
            authBusy = false
        }
    }

    fun enableAppLock(pin: String, confirmPin: String) {
        if (pin != confirmPin) {
            accountMessage = "PINs do not match."
            return
        }
        val error = repository.enableAppLock(pin)
        if (error != null) {
            accountMessage = error
        } else {
            refreshSecuritySettings()
            accountMessage = "App lock enabled. Your PIN protects this device session."
        }
    }

    fun disableAppLock(accountPassword: String) {
        viewModelScope.launch {
            if (!repository.verifyAccountPassword(accountPassword)) {
                accountMessage = "Account password is incorrect."
                return@launch
            }
            repository.disableAppLock()
            refreshSecuritySettings()
            appLocked = false
            accountMessage = "App lock disabled."
        }
    }

    fun setAutoLockTimeout(timeout: AutoLockTimeout) {
        repository.setAutoLockTimeout(timeout)
        refreshSecuritySettings()
        accountMessage = "Auto-lock set to ${timeout.label.lowercase()}."
    }

    fun onAppBackgrounded() {
        if (isLoggedIn && securitySettings.appLockEnabled) {
            repository.markAppBackgrounded()
        }
    }

    fun onAppResumed() {
        if (!isLoggedIn) return
        refreshSecuritySettings()
        if (repository.shouldLockOnResume()) {
            appLocked = true
        }
    }

    fun unlockWithPin(pin: String) {
        if (repository.verifyAppPin(pin)) {
            appLocked = false
            lockError = null
            repository.clearBackgroundMark()
        } else {
            lockError = "Incorrect PIN."
        }
    }

    fun clearLockError() {
        lockError = null
    }

    fun clearAccountMessage() {
        accountMessage = null
    }

    fun setCategory(category: ProductCategory?) {
        selectedCategory = category
        categoryFilter.value = category
    }

    fun setSearch(query: String) {
        searchQuery = query
    }

    fun addToCart(productId: String, quantity: Int = 1) {
        viewModelScope.launch { repository.addToCart(productId, quantity) }
    }

    fun setQuantity(productId: String, quantity: Int) {
        viewModelScope.launch { repository.setCartQuantity(productId, quantity) }
    }

    fun removeFromCart(productId: String) {
        viewModelScope.launch { repository.removeFromCart(productId) }
    }

    fun clearCart() {
        viewModelScope.launch { repository.clearCart() }
    }

    fun placeOrder(pickupName: String, notes: String, redeemPoints: Int = 0) {
        viewModelScope.launch {
            val order = repository.placePickupOrder(pickupName, notes, redeemPoints)
            if (order == null) {
                checkoutMessage = "Your bag is empty."
            } else {
                lastPlacedOrder = order
                currentCustomer = repository.currentCustomer()
                checkoutMessage = buildString {
                    append("Order ${order.id} is ready for pickup.")
                    if (order.discount > 0) {
                        append(" Saved $${"%.2f".format(order.discount)} with ${order.pointsRedeemed} points.")
                    }
                    if (order.pointsEarned > 0) {
                        append(" You earned ${order.pointsEarned} points!")
                    }
                }
            }
        }
    }

    fun updateOrderStatus(orderId: String, status: String) {
        viewModelScope.launch {
            when (val result = repository.updateOrderStatus(orderId, status)) {
                is OpResult.Success -> accountMessage = result.message
                is OpResult.Error -> accountMessage = result.message
            }
        }
    }

    fun shareOrderReceipt(orderId: String, onReady: (String) -> Unit) {
        viewModelScope.launch {
            val text = repository.orderReceiptText(orderId)
            if (text != null) onReady(text)
            else accountMessage = "Could not build receipt."
        }
    }

    fun createDraftFromRequest(requestId: String) {
        viewModelScope.launch {
            when (val result = repository.createDraftFromRequest(requestId)) {
                is OpResult.Success -> requestMessage = result.message
                is OpResult.Error -> requestMessage = result.message
            }
        }
    }

    fun clearCheckoutMessage() {
        checkoutMessage = null
    }

    fun clearIntakeMessage() {
        intakeMessage = null
    }

    fun clearScanResult() {
        scanResult = null
        scanError = null
    }

    fun adjustStock(productId: String, delta: Int) {
        viewModelScope.launch { repository.adjustStock(productId, delta) }
    }

    fun setPublished(productId: String, published: Boolean) {
        viewModelScope.launch {
            when (val result = repository.setPublished(productId, published)) {
                is OpResult.Success -> intakeMessage = result.message
                is OpResult.Error -> intakeMessage = result.message
            }
        }
    }

    fun saveProduct(product: Product) {
        viewModelScope.launch {
            when (val result = repository.saveProduct(product)) {
                is OpResult.Success -> intakeMessage = result.message
                is OpResult.Error -> intakeMessage = result.message
            }
        }
    }

    fun createBlankDraftProduct() {
        viewModelScope.launch {
            when (val result = repository.createBlankDraftProduct()) {
                is OpResult.Success -> intakeMessage = result.message
                is OpResult.Error -> intakeMessage = result.message
            }
        }
    }

    fun setStaffEnabled(staffId: String, enabled: Boolean) {
        viewModelScope.launch {
            when (val result = repository.setStaffEnabled(staffId, enabled)) {
                is OpResult.Success -> accountMessage = result.message
                is OpResult.Error -> accountMessage = result.message
            }
        }
    }

    fun resetStaffPassword(staffId: String, newPassword: String) {
        viewModelScope.launch {
            when (val result = repository.resetStaffPassword(staffId, newPassword)) {
                is OpResult.Success -> accountMessage = result.message
                is OpResult.Error -> accountMessage = result.message
            }
        }
    }

    fun exportSync() {
        viewModelScope.launch {
            try {
                lastSyncExport = repository.exportSyncJson()
                accountMessage = "Sync file ready to share."
            } catch (t: Throwable) {
                accountMessage = t.message ?: "Export failed."
                lastSyncExport = null
            }
        }
    }

    fun clearSyncExport() {
        lastSyncExport = null
    }

    fun checkForAppUpdate(force: Boolean = false) {
        viewModelScope.launch {
            updateState = UpdateUiState.Checking
            when (val result = updateChecker.checkForUpdate(force)) {
                is UpdateCheckResult.Available ->
                    updateState = UpdateUiState.Available(result.info)
                UpdateCheckResult.UpToDate ->
                    updateState = UpdateUiState.UpToDate
                is UpdateCheckResult.Failed ->
                    updateState = UpdateUiState.Error(result.message)
            }
        }
    }

    fun autoCheckForAppUpdate() {
        if (autoUpdateChecked) return
        autoUpdateChecked = true
        checkForAppUpdate(force = false)
    }

    fun dismissAppUpdate() {
        val available = updateState as? UpdateUiState.Available ?: return
        updateChecker.dismiss(available.info)
        updateState = UpdateUiState.Idle
    }

    fun downloadAppUpdate() {
        val info = when (val state = updateState) {
            is UpdateUiState.Available -> state.info
            is UpdateUiState.ReadyToInstall -> state.info
            else -> return
        }
        viewModelScope.launch {
            updateState = UpdateUiState.Downloading(null)
            try {
                val file = updateChecker.downloadApk(info) { progress ->
                    updateState = UpdateUiState.Downloading(progress)
                }
                downloadedApk = file
                updateState = UpdateUiState.ReadyToInstall(info)
            } catch (t: Throwable) {
                updateState = UpdateUiState.Error(t.message ?: "Download failed.")
            }
        }
    }

    fun clearUpdateMessage() {
        if (updateState is UpdateUiState.UpToDate || updateState is UpdateUiState.Error) {
            updateState = UpdateUiState.Idle
        }
    }

    fun submitProductRequest(productName: String, notes: String) {
        viewModelScope.launch {
            when (val result = repository.submitProductRequest(productName, notes)) {
                is OpResult.Success -> requestMessage = result.message
                is OpResult.Error -> requestMessage = result.message
            }
        }
    }

    fun markProductRequestFulfilled(requestId: String) {
        viewModelScope.launch {
            when (val result = repository.setProductRequestStatus(requestId, "Fulfilled")) {
                is OpResult.Success -> requestMessage = result.message
                is OpResult.Error -> requestMessage = result.message
            }
        }
    }

    fun clearRequestMessage() {
        requestMessage = null
    }

    fun importSync(json: String) {
        viewModelScope.launch {
            when (val result = repository.importSyncJson(json)) {
                is OpResult.Success -> accountMessage = result.message
                is OpResult.Error -> accountMessage = result.message
            }
        }
    }

    fun analyzeLabelBitmap(bitmap: Bitmap) {
        viewModelScope.launch {
            scanBusy = true
            scanError = null
            try {
                val catalog = repository.catalogSnapshot()
                scanResult = scanner.analyze(bitmap, catalog)
            } catch (t: Throwable) {
                scanError = t.message ?: "Could not read the label. Try again with better lighting."
                scanResult = null
            } finally {
                scanBusy = false
            }
        }
    }

    fun confirmScanIntake(quantity: Int) {
        val result = scanResult ?: return
        viewModelScope.launch {
            val intake = repository.applyScanIntake(result, quantity, createIfMissing = true)
            if (intake != null) {
                intakeMessage = "Added ${intake.quantityAdded} × ${intake.productName} to inventory."
                scanResult = null
            } else {
                scanError = "Could not add inventory from this scan."
            }
        }
    }

    override fun onCleared() {
        super.onCleared()
        scanner.close()
    }
}

class DispensaryViewModelFactory(
    private val repository: DispensaryRepository,
    private val updateChecker: AppUpdateChecker
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(DispensaryViewModel::class.java)) {
            return DispensaryViewModel(repository, updateChecker) as T
        }
        throw IllegalArgumentException("Unknown ViewModel: ${modelClass.name}")
    }
}
