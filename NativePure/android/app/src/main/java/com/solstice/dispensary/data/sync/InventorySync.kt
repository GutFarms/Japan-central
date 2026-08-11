package com.solstice.dispensary.data.sync

import com.solstice.dispensary.data.auth.PasswordHasher
import com.solstice.dispensary.data.model.AccountRole
import com.solstice.dispensary.data.model.Customer
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.ProductSize
import com.solstice.dispensary.data.model.StrainType
import org.json.JSONArray
import org.json.JSONObject

object InventorySync {
    const val FORMAT = "nativepure-sync-v1"

    fun exportJson(
        products: List<Product>,
        orders: List<Order>,
        orderLines: List<OrderLine>,
        customers: List<Customer> = emptyList()
    ): String {
        val root = JSONObject()
        root.put("format", FORMAT)
        root.put("exportedAt", System.currentTimeMillis())
        root.put("products", JSONArray().also { arr ->
            products.forEach { p ->
                arr.put(
                    JSONObject()
                        .put("id", p.id)
                        .put("name", p.name)
                        .put("brand", p.brand)
                        .put("category", p.category.name)
                        .put("strainType", p.strainType.name)
                        .put("thcPercent", p.thcPercent)
                        .put("cbdPercent", p.cbdPercent)
                        .put("price", p.price)
                        .put("unitLabel", p.unitLabel)
                        .put("description", p.description)
                        .put("effects", p.effects)
                        .put("featured", p.featured)
                        .put("inStock", p.inStock)
                        .put("stockQuantity", p.stockQuantity)
                        .put("sku", p.sku)
                        .put("published", p.published)
                        .put("publishedAt", p.publishedAt)
                        .put("publishedBy", p.publishedBy)
                        .put("onDeal", p.onDeal)
                        .put("dealPercent", p.dealPercent)
                        .put("dealLabel", p.dealLabel)
                        .put("sizeInventoryEnabled", p.sizeInventoryEnabled)
                        .put("stockGram", p.stockGram)
                        .put("priceGram", p.priceGram)
                        .put("stockEighth", p.stockEighth)
                        .put("priceEighth", p.priceEighth)
                        .put("stockQuarter", p.stockQuarter)
                        .put("priceQuarter", p.priceQuarter)
                        .put("stockOunce", p.stockOunce)
                        .put("priceOunce", p.priceOunce)
                        .put("imagePath", p.imagePath)
                )
            }
        })
        root.put("customers", JSONArray().also { arr ->
            customers.forEach { c ->
                // Never export password hashes/salts — credentials stay on-device.
                arr.put(
                    JSONObject()
                        .put("id", c.id)
                        .put("email", c.email)
                        .put("username", c.username)
                        .put("fullName", c.fullName)
                        .put("phone", c.phone)
                        .put("dateOfBirth", c.dateOfBirth)
                        .put("createdAt", c.createdAt)
                        .put("lastLoginAt", c.lastLoginAt)
                        .put("notes", c.notes)
                        .put("marketingOptIn", c.marketingOptIn)
                        .put("role", c.role.name)
                        .put("createdByAdminId", c.createdByAdminId)
                        .put("mustChangePassword", c.mustChangePassword)
                        .put("enabled", c.enabled)
                        .put("emailVerified", c.emailVerified)
                        .put("loyaltyPoints", c.loyaltyPoints)
                        .put("lifetimeSpend", c.lifetimeSpend)
                )
            }
        })
        root.put("orders", JSONArray().also { arr ->
            orders.forEach { o ->
                arr.put(
                    JSONObject()
                        .put("id", o.id)
                        .put("createdAt", o.createdAt)
                        .put("total", o.total)
                        .put("itemCount", o.itemCount)
                        .put("status", o.status)
                        .put("pickupName", o.pickupName)
                        .put("notes", o.notes)
                        .put("customerId", o.customerId)
                        .put("customerEmail", o.customerEmail)
                        .put("pointsEarned", o.pointsEarned)
                        .put("pointsRedeemed", o.pointsRedeemed)
                        .put("discount", o.discount)
                )
            }
        })
        root.put("orderLines", JSONArray().also { arr ->
            orderLines.forEach { line ->
                arr.put(
                    JSONObject()
                        .put("orderId", line.orderId)
                        .put("productId", line.productId)
                        .put("productName", line.productName)
                        .put("unitPrice", line.unitPrice)
                        .put("quantity", line.quantity)
                        .put("sizeKey", line.sizeKey)
                        .put("sizeLabel", line.sizeLabel)
                )
            }
        })
        return root.toString(2)
    }

    fun parseProducts(json: String): List<Product> {
        val root = JSONObject(json)
        require(root.optString("format") == FORMAT) { "Unsupported sync file format." }
        val arr = root.getJSONArray("products")
        return buildList {
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                val product = Product(
                    id = o.getString("id"),
                    name = o.getString("name"),
                    brand = o.getString("brand"),
                    category = ProductCategory.valueOf(o.getString("category")),
                    strainType = StrainType.valueOf(o.getString("strainType")),
                    thcPercent = o.getDouble("thcPercent"),
                    cbdPercent = o.getDouble("cbdPercent"),
                    price = o.getDouble("price"),
                    unitLabel = o.getString("unitLabel"),
                    description = o.getString("description"),
                    effects = o.getString("effects"),
                    featured = o.optBoolean("featured", false),
                    inStock = o.optBoolean("inStock", true),
                    stockQuantity = o.optInt("stockQuantity", 0),
                    sku = o.optString("sku", ""),
                    published = o.optBoolean("published", true),
                    publishedAt = o.optLong("publishedAt", 0L),
                    publishedBy = o.optString("publishedBy", ""),
                    onDeal = o.optBoolean("onDeal", false),
                    dealPercent = o.optInt("dealPercent", 0),
                    dealLabel = o.optString("dealLabel", ""),
                    sizeInventoryEnabled = o.optBoolean("sizeInventoryEnabled", false),
                    stockGram = o.optInt("stockGram", 0),
                    priceGram = o.optDouble("priceGram", 0.0),
                    stockEighth = o.optInt("stockEighth", 0),
                    priceEighth = o.optDouble("priceEighth", 0.0),
                    stockQuarter = o.optInt("stockQuarter", 0),
                    priceQuarter = o.optDouble("priceQuarter", 0.0),
                    stockOunce = o.optInt("stockOunce", 0),
                    priceOunce = o.optDouble("priceOunce", 0.0),
                    imagePath = o.optString("imagePath", "")
                )
                add(
                    if (product.sizeInventoryEnabled) product.normalizedSizeInventory() else product
                )
            }
        }
    }

    fun parseCustomers(json: String): List<Customer> {
        val root = JSONObject(json)
        require(root.optString("format") == FORMAT) { "Unsupported sync file format." }
        val arr = root.optJSONArray("customers") ?: return emptyList()
        return buildList {
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                // Ignore any legacy passwordHash/passwordSalt — credentials never trusted from sync.
                val salt = PasswordHasher.newSalt()
                val hash = PasswordHasher.hash(PasswordHasher.randomUnusableSecret(), salt)
                add(
                    Customer(
                        id = o.getString("id"),
                        email = o.getString("email"),
                        username = o.optString("username", ""),
                        passwordHash = hash,
                        passwordSalt = salt,
                        fullName = o.optString("fullName", ""),
                        phone = o.optString("phone", ""),
                        dateOfBirth = o.optString("dateOfBirth", ""),
                        createdAt = o.optLong("createdAt", System.currentTimeMillis()),
                        lastLoginAt = o.optLong("lastLoginAt", 0L),
                        notes = o.optString("notes", ""),
                        marketingOptIn = o.optBoolean("marketingOptIn", false),
                        role = AccountRole.CUSTOMER, // never escalate from sync
                        createdByAdminId = "",
                        mustChangePassword = true,
                        enabled = o.optBoolean("enabled", true),
                        emailVerified = o.optBoolean("emailVerified", false),
                        loyaltyPoints = o.optInt("loyaltyPoints", 0),
                        lifetimeSpend = o.optDouble("lifetimeSpend", 0.0)
                    )
                )
            }
        }
    }

    fun parseOrders(json: String): Pair<List<Order>, List<OrderLine>> {
        val root = JSONObject(json)
        require(root.optString("format") == FORMAT) { "Unsupported sync file format." }
        val orders = buildList {
            val arr = root.optJSONArray("orders") ?: JSONArray()
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                add(
                    Order(
                        id = o.getString("id"),
                        createdAt = o.getLong("createdAt"),
                        total = o.getDouble("total"),
                        itemCount = o.getInt("itemCount"),
                        status = o.getString("status"),
                        pickupName = o.getString("pickupName"),
                        notes = o.optString("notes", ""),
                        customerId = o.optString("customerId", ""),
                        customerEmail = o.optString("customerEmail", ""),
                        pointsEarned = o.optInt("pointsEarned", 0),
                        pointsRedeemed = o.optInt("pointsRedeemed", 0),
                        discount = o.optDouble("discount", 0.0)
                    )
                )
            }
        }
        val lines = buildList {
            val arr = root.optJSONArray("orderLines") ?: JSONArray()
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                add(
                    OrderLine(
                        orderId = o.getString("orderId"),
                        productId = o.getString("productId"),
                        productName = o.getString("productName"),
                        unitPrice = o.getDouble("unitPrice"),
                        quantity = o.getInt("quantity"),
                        sizeKey = o.optString("sizeKey", ProductSize.UNIT_KEY),
                        sizeLabel = o.optString("sizeLabel", "")
                    )
                )
            }
        }
        return orders to lines
    }
}
