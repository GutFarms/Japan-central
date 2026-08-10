package com.solstice.dispensary.data.repository

import android.content.Context
import com.solstice.dispensary.data.db.DispensaryDatabase
import com.solstice.dispensary.data.db.SeedCatalog
import com.solstice.dispensary.data.model.CartItem
import com.solstice.dispensary.data.model.CartLine
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import java.util.UUID

class DispensaryRepository(context: Context) {
    private val db = DispensaryDatabase.get(context)
    private val prefs = context.getSharedPreferences("solstice_prefs", Context.MODE_PRIVATE)

    val products: Flow<List<Product>> = db.productDao().observeAll()
    val featured: Flow<List<Product>> = db.productDao().observeFeatured()
    val cartItems: Flow<List<CartItem>> = db.cartDao().observeAll()
    val orders: Flow<List<Order>> = db.orderDao().observeAll()

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

    fun isAgeVerified(): Boolean = prefs.getBoolean(KEY_AGE, false)

    fun setAgeVerified(verified: Boolean) {
        prefs.edit().putBoolean(KEY_AGE, verified).apply()
    }

    suspend fun ensureSeeded() {
        if (db.productDao().count() == 0) {
            db.productDao().insertAll(SeedCatalog.products)
        }
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

        val subtotal = lines.sumOf { it.lineTotal }
        val tax = subtotal * TAX_RATE
        val order = Order(
            id = UUID.randomUUID().toString().take(8).uppercase(),
            createdAt = System.currentTimeMillis(),
            total = subtotal + tax,
            itemCount = lines.sumOf { it.quantity },
            status = "Ready for pickup",
            pickupName = pickupName.ifBlank { "Guest" },
            notes = notes.trim()
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
        db.cartDao().clear()
        return order
    }

    companion object {
        const val TAX_RATE = 0.08
        private const val KEY_AGE = "age_verified"
    }
}
