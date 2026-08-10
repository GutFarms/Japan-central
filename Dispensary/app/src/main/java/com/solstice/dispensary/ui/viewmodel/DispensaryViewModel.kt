package com.solstice.dispensary.ui.viewmodel

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.repository.DispensaryRepository
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

    var ageVerified by mutableStateOf(repository.isAgeVerified())
        private set

    var selectedCategory by mutableStateOf<ProductCategory?>(null)
        private set

    var searchQuery by mutableStateOf("")
        private set

    var lastPlacedOrder by mutableStateOf<Order?>(null)
        private set

    var checkoutMessage by mutableStateOf<String?>(null)
        private set

    private val categoryFilter = MutableStateFlow<ProductCategory?>(null)

    val products: StateFlow<List<Product>> = categoryFilter
        .flatMapLatest { repository.productsByCategory(it) }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val featured: StateFlow<List<Product>> = repository.featured
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val cart: StateFlow<CartSummary> = repository.cartSummary
        .stateIn(
            viewModelScope,
            SharingStarted.WhileSubscribed(5_000),
            CartSummary(emptyList(), 0.0, 0.0, 0.0, 0)
        )

    val orders: StateFlow<List<Order>> = repository.orders
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun filteredProducts(): List<Product> {
        val q = searchQuery.trim().lowercase()
        val list = products.value
        if (q.isEmpty()) return list
        return list.filter {
            it.name.lowercase().contains(q) ||
                it.brand.lowercase().contains(q) ||
                it.effects.lowercase().contains(q) ||
                it.category.label.lowercase().contains(q)
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
