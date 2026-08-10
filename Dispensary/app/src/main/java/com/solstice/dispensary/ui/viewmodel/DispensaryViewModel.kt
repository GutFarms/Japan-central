package com.solstice.dispensary.ui.viewmodel

import android.graphics.Bitmap
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.solstice.dispensary.data.model.AuthResult
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.data.model.CustomerProfile
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.LabelScanResult
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.ThemeMode
import com.solstice.dispensary.data.repository.DispensaryRepository
import com.solstice.dispensary.scan.LabelAiScanner
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

@OptIn(ExperimentalCoroutinesApi::class)
class DispensaryViewModel(
    private val repository: DispensaryRepository
) : ViewModel() {

    private val scanner = LabelAiScanner()

    var ageVerified by mutableStateOf(repository.isAgeVerified())
        private set

    var themeMode by mutableStateOf(repository.getThemeMode())
        private set

    var currentCustomer by mutableStateOf<CustomerProfile?>(null)
        private set

    var authBusy by mutableStateOf(false)
        private set

    var authError by mutableStateOf<String?>(null)
        private set

    var accountMessage by mutableStateOf<String?>(null)
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

    val isLoggedIn: Boolean get() = currentCustomer != null

    val canViewSensitiveInfo: Boolean
        get() = currentCustomer?.role?.canViewSensitiveInfo == true

    val canManageStaff: Boolean
        get() = currentCustomer?.role?.canManageStaff == true

    val canManageInventory: Boolean
        get() = currentCustomer?.role?.canManageInventory == true

    init {
        viewModelScope.launch {
            currentCustomer = repository.currentCustomer()
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

    fun clearAuthError() {
        authError = null
    }

    fun login(email: String, password: String) {
        viewModelScope.launch {
            authBusy = true
            authError = null
            when (val result = repository.login(email, password)) {
                is AuthResult.Success -> currentCustomer = result.customer
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
                is AuthResult.Success -> currentCustomer = result.customer
                is AuthResult.Error -> authError = result.message
            }
            authBusy = false
        }
    }

    fun logout() {
        repository.logout()
        currentCustomer = null
        accountMessage = null
    }

    fun createStaffSubAccount(email: String, password: String, fullName: String) {
        viewModelScope.launch {
            when (val result = repository.createStaffSubAccount(email, password, fullName)) {
                is AuthResult.Success ->
                    accountMessage = "Staff account created for ${result.customer.email}."
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

    fun placeOrder(pickupName: String, notes: String) {
        viewModelScope.launch {
            val order = repository.placePickupOrder(pickupName, notes)
            if (order == null) {
                checkoutMessage = "Your cart is empty."
            } else {
                lastPlacedOrder = order
                checkoutMessage = "Order ${order.id} is ready for pickup."
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
    private val repository: DispensaryRepository
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(DispensaryViewModel::class.java)) {
            return DispensaryViewModel(repository) as T
        }
        throw IllegalArgumentException("Unknown ViewModel: ${modelClass.name}")
    }
}
