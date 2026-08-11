package com.solstice.dispensary.data.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import androidx.room.Update
import com.solstice.dispensary.data.model.AccountRole
import com.solstice.dispensary.data.model.CartItem
import com.solstice.dispensary.data.model.Customer
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.ProductRequest
import kotlinx.coroutines.flow.Flow

@Dao
interface ProductDao {
    @Query("SELECT * FROM products ORDER BY featured DESC, name ASC")
    fun observeAll(): Flow<List<Product>>

    @Query("SELECT * FROM products WHERE published = 1 ORDER BY featured DESC, name ASC")
    fun observePublished(): Flow<List<Product>>

    @Query("SELECT * FROM products ORDER BY published ASC, stockQuantity ASC, name ASC")
    fun observeInventory(): Flow<List<Product>>

    @Query("SELECT * FROM products WHERE category = :category AND published = 1 ORDER BY name ASC")
    fun observePublishedByCategory(category: ProductCategory): Flow<List<Product>>

    @Query("SELECT * FROM products WHERE category = :category ORDER BY name ASC")
    fun observeByCategory(category: ProductCategory): Flow<List<Product>>

    @Query("SELECT * FROM products WHERE id = :id LIMIT 1")
    fun observeById(id: String): Flow<Product?>

    @Query("SELECT * FROM products WHERE id = :id LIMIT 1")
    suspend fun getById(id: String): Product?

    @Query("SELECT * FROM products WHERE sku = :sku LIMIT 1")
    suspend fun getBySku(sku: String): Product?

    @Query("SELECT * FROM products WHERE featured = 1 AND published = 1 ORDER BY name ASC")
    fun observeFeaturedPublished(): Flow<List<Product>>

    @Query("SELECT * FROM products WHERE featured = 1 ORDER BY name ASC")
    fun observeFeatured(): Flow<List<Product>>

    @Query(
        "SELECT * FROM products WHERE published = 1 AND onDeal = 1 AND dealPercent > 0 " +
            "ORDER BY dealPercent DESC, name ASC"
    )
    fun observePublishedDeals(): Flow<List<Product>>

    @Query("SELECT COUNT(*) FROM products")
    suspend fun count(): Int

    @Query("SELECT * FROM products")
    suspend fun getAll(): List<Product>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(products: List<Product>)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(product: Product)

    @Update
    suspend fun update(product: Product)

    @Query("UPDATE products SET stockQuantity = :quantity, inStock = :inStock WHERE id = :id")
    suspend fun setStock(id: String, quantity: Int, inStock: Boolean)

    @Query("UPDATE products SET published = :published WHERE id = :id")
    suspend fun setPublished(id: String, published: Boolean)
}

@Dao
interface CartDao {
    @Query("SELECT * FROM cart_items")
    fun observeAll(): Flow<List<CartItem>>

    @Query("SELECT * FROM cart_items")
    suspend fun getAll(): List<CartItem>

    @Query(
        "SELECT * FROM cart_items WHERE productId = :productId AND sizeKey = :sizeKey LIMIT 1"
    )
    suspend fun get(productId: String, sizeKey: String): CartItem?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(item: CartItem)

    @Update
    suspend fun update(item: CartItem)

    @Query("DELETE FROM cart_items WHERE productId = :productId AND sizeKey = :sizeKey")
    suspend fun delete(productId: String, sizeKey: String)

    @Query("DELETE FROM cart_items")
    suspend fun clear()
}

@Dao
interface OrderDao {
    @Query("SELECT * FROM orders ORDER BY createdAt DESC")
    fun observeAll(): Flow<List<Order>>

    @Query("SELECT * FROM orders WHERE customerId = :customerId ORDER BY createdAt DESC")
    fun observeForCustomer(customerId: String): Flow<List<Order>>

    @Query("SELECT * FROM order_lines WHERE orderId = :orderId")
    fun observeLines(orderId: String): Flow<List<OrderLine>>

    @Query("SELECT * FROM orders WHERE id = :id LIMIT 1")
    suspend fun getById(id: String): Order?

    @Query("SELECT * FROM order_lines WHERE orderId = :orderId")
    suspend fun getLines(orderId: String): List<OrderLine>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrder(order: Order)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertLines(lines: List<OrderLine>)

    @Update
    suspend fun update(order: Order)

    @Transaction
    suspend fun placeOrder(order: Order, lines: List<OrderLine>) {
        insertOrder(order)
        insertLines(lines)
    }
}

@Dao
interface InventoryDao {
    @Query("SELECT * FROM inventory_intakes ORDER BY createdAt DESC")
    fun observeAll(): Flow<List<InventoryIntake>>

    @Insert
    suspend fun insert(intake: InventoryIntake)
}

@Dao
interface ProductRequestDao {
    @Query("SELECT * FROM product_requests ORDER BY createdAt DESC")
    fun observeAll(): Flow<List<ProductRequest>>

    @Query("SELECT * FROM product_requests WHERE customerId = :customerId ORDER BY createdAt DESC")
    fun observeForCustomer(customerId: String): Flow<List<ProductRequest>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(request: ProductRequest)

    @Query("SELECT * FROM product_requests WHERE id = :id LIMIT 1")
    suspend fun getById(id: String): ProductRequest?

    @Query("UPDATE product_requests SET status = :status WHERE id = :id")
    suspend fun setStatus(id: String, status: String)

    @Query("SELECT COUNT(*) FROM product_requests")
    suspend fun count(): Int

    @Query("SELECT COUNT(DISTINCT customerId) FROM product_requests")
    suspend fun countDistinctCustomers(): Int
}

@Dao
interface CustomerDao {
    @Query("SELECT * FROM customers ORDER BY role DESC, createdAt DESC")
    fun observeAll(): Flow<List<Customer>>

    @Query("SELECT * FROM customers WHERE id = :id LIMIT 1")
    fun observeById(id: String): Flow<Customer?>

    @Query("SELECT * FROM customers WHERE id = :id LIMIT 1")
    suspend fun getById(id: String): Customer?

    @Query("SELECT * FROM customers WHERE lower(email) = lower(:email) LIMIT 1")
    suspend fun getByEmail(email: String): Customer?

    @Query("SELECT * FROM customers WHERE lower(username) = lower(:username) AND username != '' LIMIT 1")
    suspend fun getByUsername(username: String): Customer?

    @Query("SELECT * FROM customers WHERE role = :role ORDER BY createdAt DESC")
    fun observeByRole(role: AccountRole): Flow<List<Customer>>

    @Query("SELECT * FROM customers WHERE role = :role ORDER BY createdAt DESC")
    suspend fun getByRole(role: AccountRole): List<Customer>

    @Query("SELECT COUNT(*) FROM customers")
    suspend fun count(): Int

    @Query("SELECT COUNT(*) FROM customers WHERE role = :role")
    suspend fun countByRole(role: AccountRole): Int

    @Insert(onConflict = OnConflictStrategy.ABORT)
    suspend fun insert(customer: Customer)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(customer: Customer)

    @Update
    suspend fun update(customer: Customer)

    @Query("DELETE FROM customers WHERE id = :id")
    suspend fun delete(id: String)
}

