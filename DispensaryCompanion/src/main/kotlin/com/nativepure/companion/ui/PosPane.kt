package com.nativepure.companion.ui

import androidx.compose.foundation.VerticalScrollbar
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollbarAdapter
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.nativepure.companion.data.CompanionRepository
import com.nativepure.companion.data.CustomerProfile
import com.nativepure.companion.data.LoyaltyPoints
import com.nativepure.companion.data.OpResult
import com.nativepure.companion.data.OrderStatus
import com.nativepure.companion.data.PaymentMethod
import com.nativepure.companion.data.PosSaleRequest
import com.nativepure.companion.data.Product
import java.awt.Toolkit
import java.awt.datatransfer.StringSelection
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun PosPane(
    repository: CompanionRepository,
    cashier: CustomerProfile,
    onRefresh: () -> Unit,
    onMessage: (String?) -> Unit
) {
    var search by remember { mutableStateOf("") }
    var customerQuery by remember { mutableStateOf("") }
    var loyaltyCustomer by remember { mutableStateOf<CustomerProfile?>(null) }
    var walkInName by remember { mutableStateOf("") }
    var notes by remember { mutableStateOf("") }
    var paymentMethod by remember { mutableStateOf(PaymentMethod.CASH) }
    var cashTenderedText by remember { mutableStateOf("") }
    var redeemPoints by remember { mutableStateOf(0) }
    var categoryFilter by remember { mutableStateOf<String?>(null) }

    val cart = repository.cartSummary()
    val products = repository.posSearchProducts(search)
        .let { list ->
            if (categoryFilter == null) list
            else list.filter { it.category.label == categoryFilter }
        }
    val categories = remember(products) {
        repository.posSearchProducts("").map { it.category.label }.distinct().sorted()
    }
    val queue = repository.openPickupQueue()
    val todaysSales = repository.todaysPosSales()
    val todayTotal = todaysSales.sumOf { it.total }

    val maxRedeem = loyaltyCustomer?.let {
        LoyaltyPoints.maxRedeemablePoints(it.loyaltyPoints, cart.total)
    } ?: 0
    val safeRedeem = redeemPoints.coerceAtMost(maxRedeem)
        .let { it - (it % LoyaltyPoints.REDEEM_POINTS_PER_DOLLAR) }
    val discount = LoyaltyPoints.discountForPoints(safeRedeem)
    val payable = (cart.total - discount).coerceAtLeast(0.0)
    val cashTendered = cashTenderedText.toDoubleOrNull() ?: 0.0
    val changeDue = if (paymentMethod == PaymentMethod.CASH) {
        (cashTendered - payable).coerceAtLeast(0.0)
    } else {
        0.0
    }
    val customerHits = if (customerQuery.length >= 2 && loyaltyCustomer == null) {
        repository.findCustomersForPos(customerQuery)
    } else {
        emptyList()
    }

    Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        // —— Product browser ——
        Column(Modifier.weight(1.35f).fillMaxHeight()) {
            Text("Register", style = MaterialTheme.typography.headlineLarge)
            Text(
                "Cashier ${cashier.fullName} · Today ${todaysSales.size} sales · $${"%.2f".format(todayTotal)}",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(10.dp))
            OutlinedTextField(
                value = search,
                onValueChange = { search = it },
                label = { Text("Search name, SKU, or brand") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(10.dp)
            )
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                FilterChip(
                    selected = categoryFilter == null,
                    onClick = { categoryFilter = null },
                    label = { Text("All") }
                )
                categories.take(8).forEach { cat ->
                    FilterChip(
                        selected = categoryFilter == cat,
                        onClick = { categoryFilter = if (categoryFilter == cat) null else cat },
                        label = { Text(cat) }
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
            if (products.isEmpty()) {
                Text("No matching in-stock products.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            } else {
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(160.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    contentPadding = PaddingValues(bottom = 12.dp),
                    modifier = Modifier.fillMaxSize()
                ) {
                    items(products, key = { it.id }) { product ->
                        PosProductTile(product) {
                            repository.addToCart(product.id)
                            onRefresh()
                        }
                    }
                }
            }
        }

        // —— Ticket + tender ——
        Column(Modifier.weight(1f).fillMaxHeight()) {
            Surface(
                shape = RoundedCornerShape(14.dp),
                tonalElevation = 2.dp,
                modifier = Modifier.fillMaxWidth().weight(1f)
            ) {
                Column(Modifier.padding(14.dp).fillMaxSize()) {
                    Text("Ticket", style = MaterialTheme.typography.headlineMedium)
                    Spacer(Modifier.height(8.dp))
                    if (cart.lines.isEmpty()) {
                        Text("Scan or tap products to start a sale.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    } else {
                        val listState = rememberLazyListState()
                        Box(Modifier.weight(1f, fill = true)) {
                            LazyColumn(
                                state = listState,
                                verticalArrangement = Arrangement.spacedBy(6.dp),
                                modifier = Modifier.fillMaxSize().padding(end = 10.dp)
                            ) {
                                items(cart.lines, key = { it.product.id }) { line ->
                                    Row(
                                        Modifier.fillMaxWidth(),
                                        verticalAlignment = Alignment.CenterVertically
                                    ) {
                                        Column(Modifier.weight(1f)) {
                                            Text(line.product.name, style = MaterialTheme.typography.titleMedium)
                                            Text(
                                                "$${ "%.2f".format(line.unitPrice)} · stock ${line.product.stockQuantity}",
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                                style = MaterialTheme.typography.bodyMedium
                                            )
                                        }
                                        OutlinedButton(
                                            onClick = {
                                                repository.setCartQuantity(line.product.id, line.quantity - 1)
                                                onRefresh()
                                            },
                                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)
                                        ) { Text("−") }
                                        Text(
                                            "${line.quantity}",
                                            modifier = Modifier.padding(horizontal = 8.dp),
                                            style = MaterialTheme.typography.titleLarge
                                        )
                                        OutlinedButton(
                                            onClick = {
                                                repository.setCartQuantity(line.product.id, line.quantity + 1)
                                                onRefresh()
                                            },
                                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)
                                        ) { Text("+") }
                                        Spacer(Modifier.width(8.dp))
                                        Text(
                                            "$${"%.2f".format(line.lineTotal)}",
                                            style = MaterialTheme.typography.titleMedium
                                        )
                                    }
                                }
                            }
                            VerticalScrollbar(
                                adapter = rememberScrollbarAdapter(listState),
                                modifier = Modifier.align(Alignment.CenterEnd).fillMaxHeight()
                            )
                        }
                    }

                    HorizontalDivider(Modifier.padding(vertical = 8.dp))
                    Text("Subtotal $${"%.2f".format(cart.subtotal)}")
                    Text("Tax $${"%.2f".format(cart.tax)}")
                    if (discount > 0) Text("Loyalty −$${"%.2f".format(discount)}")
                    Text(
                        "Total $${"%.2f".format(payable)}",
                        style = MaterialTheme.typography.headlineMedium
                    )

                    Spacer(Modifier.height(8.dp))
                    Text("Customer (optional)", style = MaterialTheme.typography.titleMedium)
                    if (loyaltyCustomer != null) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(loyaltyCustomer!!.fullName, style = MaterialTheme.typography.titleLarge)
                                Text(
                                    "${loyaltyCustomer!!.email} · ${loyaltyCustomer!!.loyaltyPoints} pts",
                                    color = MaterialTheme.colorScheme.secondary
                                )
                            }
                            TextButton(onClick = {
                                loyaltyCustomer = null
                                redeemPoints = 0
                                customerQuery = ""
                            }) { Text("Clear") }
                        }
                        if (maxRedeem >= 100) {
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                FilterChip(
                                    selected = safeRedeem == 0,
                                    onClick = { redeemPoints = 0 },
                                    label = { Text("No redeem") }
                                )
                                listOf(100, 200, 500, maxRedeem).distinct().filter { it in 100..maxRedeem }.forEach { pts ->
                                    FilterChip(
                                        selected = safeRedeem == pts,
                                        onClick = { redeemPoints = pts },
                                        label = { Text("$pts pts") }
                                    )
                                }
                            }
                        }
                    } else {
                        OutlinedTextField(
                            value = customerQuery,
                            onValueChange = { customerQuery = it },
                            label = { Text("Lookup name, email, or phone") },
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(10.dp)
                        )
                        customerHits.forEach { hit ->
                            TextButton(onClick = {
                                loyaltyCustomer = hit
                                walkInName = hit.fullName
                                customerQuery = ""
                            }) {
                                Text("${hit.fullName} · ${hit.loyaltyPoints} pts")
                            }
                        }
                        OutlinedTextField(
                            value = walkInName,
                            onValueChange = { walkInName = it },
                            label = { Text("Walk-in name") },
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(10.dp)
                        )
                    }

                    Spacer(Modifier.height(8.dp))
                    Text("Tender", style = MaterialTheme.typography.titleMedium)
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        PaymentMethod.entries.forEach { method ->
                            FilterChip(
                                selected = paymentMethod == method,
                                onClick = { paymentMethod = method },
                                label = { Text(method.label) }
                            )
                        }
                    }
                    if (paymentMethod == PaymentMethod.CASH) {
                        OutlinedTextField(
                            value = cashTenderedText,
                            onValueChange = { cashTenderedText = it.filter { ch -> ch.isDigit() || ch == '.' } },
                            label = { Text("Cash tendered") },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(10.dp),
                            supportingText = {
                                if (cart.lines.isNotEmpty() && cashTendered >= payable) {
                                    Text("Change $${"%.2f".format(changeDue)}")
                                }
                            }
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            listOf(payable, 20.0, 50.0, 100.0).distinct().forEach { amt ->
                                OutlinedButton(onClick = {
                                    cashTenderedText = "%.2f".format(amt)
                                }) { Text("$${"%.0f".format(amt)}") }
                            }
                        }
                    }

                    OutlinedTextField(
                        value = notes,
                        onValueChange = { notes = it },
                        label = { Text("Notes") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(10.dp)
                    )

                    Spacer(Modifier.height(10.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            onClick = {
                                val tender = when (paymentMethod) {
                                    PaymentMethod.CASH -> cashTendered
                                    else -> payable
                                }
                                when (
                                    val result = repository.completePosSale(
                                        PosSaleRequest(
                                            customerName = walkInName.ifBlank { loyaltyCustomer?.fullName.orEmpty() },
                                            paymentMethod = paymentMethod,
                                            amountTendered = tender,
                                            notes = notes,
                                            loyaltyCustomerId = loyaltyCustomer?.id,
                                            redeemPoints = safeRedeem
                                        )
                                    )
                                ) {
                                    is OpResult.Success -> {
                                        onMessage(result.message)
                                        cashTenderedText = ""
                                        notes = ""
                                        redeemPoints = 0
                                        walkInName = ""
                                        // Keep loyalty customer for next sale? Clear for privacy.
                                        loyaltyCustomer = null
                                        customerQuery = ""
                                        onRefresh()
                                    }
                                    is OpResult.Error -> onMessage(result.message)
                                }
                            },
                            enabled = cart.lines.isNotEmpty() &&
                                (paymentMethod != PaymentMethod.CASH || cashTendered + 0.001 >= payable),
                            modifier = Modifier.weight(1f),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = MaterialTheme.colorScheme.primary
                            )
                        ) { Text("Complete sale") }
                        OutlinedButton(onClick = {
                            repository.clearCart()
                            onRefresh()
                        }) { Text("Void") }
                    }
                }
            }

            Spacer(Modifier.height(12.dp))
            PickupQueueStrip(repository, queue, onRefresh, onMessage)
        }
    }
}

@Composable
private fun PosProductTile(product: Product, onAdd: () -> Unit) {
    Surface(
        onClick = onAdd,
        shape = RoundedCornerShape(12.dp),
        tonalElevation = 1.dp,
        modifier = Modifier.fillMaxWidth().height(110.dp)
    ) {
        Column(
            Modifier.padding(10.dp).fillMaxSize(),
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            Column {
                Text(product.name, style = MaterialTheme.typography.titleMedium, maxLines = 2)
                Text(
                    product.sku.ifBlank { product.category.label },
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.Bottom
            ) {
                Text(
                    "$${"%.2f".format(product.effectivePrice)}",
                    style = MaterialTheme.typography.titleLarge
                )
                Text(
                    "×${product.stockQuantity}",
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun PickupQueueStrip(
    repository: CompanionRepository,
    queue: List<com.nativepure.companion.data.Order>,
    onRefresh: () -> Unit,
    onMessage: (String?) -> Unit
) {
    val dateFormat = remember { SimpleDateFormat("h:mm a", Locale.US) }
    Surface(
        shape = RoundedCornerShape(12.dp),
        tonalElevation = 1.dp,
        modifier = Modifier.fillMaxWidth().height(220.dp)
    ) {
        Column(Modifier.padding(12.dp).fillMaxSize()) {
            Text(
                "Pickup queue (${queue.size})",
                style = MaterialTheme.typography.titleLarge
            )
            Spacer(Modifier.height(6.dp))
            if (queue.isEmpty()) {
                OrderBackgroundFeed(
                    compact = true,
                    modifier = Modifier.fillMaxWidth().weight(1f, fill = true)
                )
            } else {
                LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.weight(1f)) {
                    items(queue, key = { it.id }) { order ->
                        Row(
                            Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(
                                    "#${order.id} · ${order.pickupName}",
                                    style = MaterialTheme.typography.titleMedium
                                )
                                Text(
                                    "${order.itemCount} items · $${"%.2f".format(order.total)} · ${dateFormat.format(Date(order.createdAt))}",
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    style = MaterialTheme.typography.bodyMedium
                                )
                            }
                            TextButton(onClick = {
                                val text = repository.orderReceiptText(order.id)
                                if (text != null) {
                                    Toolkit.getDefaultToolkit().systemClipboard
                                        .setContents(StringSelection(text), null)
                                }
                                onMessage(text ?: "No receipt.")
                            }) { Text("Receipt") }
                            Button(
                                onClick = {
                                    when (val result = repository.updateOrderStatus(order.id, OrderStatus.PICKED_UP)) {
                                        is OpResult.Success -> {
                                            onMessage(result.message)
                                            onRefresh()
                                        }
                                        is OpResult.Error -> onMessage(result.message)
                                    }
                                },
                                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp)
                            ) { Text("Hand off") }
                        }
                    }
                }
            }
        }
    }
}
