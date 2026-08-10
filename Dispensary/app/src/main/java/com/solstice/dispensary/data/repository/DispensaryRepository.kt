package com.solstice.dispensary.data.repository

import android.content.Context
import com.solstice.dispensary.data.db.DispensaryDatabase
import com.solstice.dispensary.data.db.SeedCatalog
import com.solstice.dispensary.data.model.CartItem
import com.solstice.dispensary.data.model.CartLine
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.LabelScanResult
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.scan.NewProductFactory
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
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

    /**
     * Confirms an AI scan: matches existing product or creates a new one, then adds stock.
     */
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

        // Decrement inventory for fulfilled pickup
        lines.forEach { line ->
            adjustStock(line.product.id, -line.quantity)
        }

        db.cartDao().clear()
        return order
    }

    companion object {
        const val TAX_RATE = 0.08
        private const val KEY_AGE = "age_verified"
    }
}
