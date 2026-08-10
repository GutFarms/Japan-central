package com.solstice.dispensary.data.sync

import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderLine
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.StrainType
import org.json.JSONArray
import org.json.JSONObject

object InventorySync {
    const val FORMAT = "nativepure-sync-v1"

    fun exportJson(
        products: List<Product>,
        orders: List<Order>,
        orderLines: List<OrderLine>
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
                add(
                    Product(
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
                        publishedBy = o.optString("publishedBy", "")
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
                        customerEmail = o.optString("customerEmail", "")
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
                        quantity = o.getInt("quantity")
                    )
                )
            }
        }
        return orders to lines
    }
}
