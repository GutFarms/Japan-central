package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.CartSummary
import com.solstice.dispensary.ui.components.ProductSwatch
import com.solstice.dispensary.ui.components.QuantityStepper
import com.solstice.dispensary.ui.components.SectionHeader
import com.solstice.dispensary.ui.components.money

@Composable
fun CartScreen(
    cart: CartSummary,
    defaultPickupName: String = "",
    checkoutMessage: String?,
    onClearMessage: () -> Unit,
    onSetQuantity: (String, Int) -> Unit,
    onRemove: (String) -> Unit,
    onClear: () -> Unit,
    onPlaceOrder: (String, String) -> Unit,
    onBrowseMenu: () -> Unit
) {
    var name by remember(defaultPickupName) { mutableStateOf(defaultPickupName) }
    var notes by remember { mutableStateOf("") }

    LaunchedEffect(checkoutMessage) {
        if (checkoutMessage != null) {
            name = defaultPickupName
            notes = ""
        }
    }

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        item {
            Spacer(Modifier.height(8.dp))
            SectionHeader(
                title = "Your bag",
                subtitle = if (cart.itemCount == 0) "Empty" else "${cart.itemCount} items"
            )
        }

        if (cart.lines.isEmpty()) {
            item {
                Text(
                    text = "Add products from the menu to start a pickup order.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(12.dp))
                Button(onClick = onBrowseMenu, shape = RoundedCornerShape(12.dp)) {
                    Text("Browse menu")
                }
            }
        } else {
            items(cart.lines, key = { it.product.id }) { line ->
                Surface(
                    shape = RoundedCornerShape(14.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        ProductSwatch(product = line.product)
                        Column(
                            modifier = Modifier
                                .weight(1f)
                                .padding(horizontal = 12.dp)
                        ) {
                            Text(line.product.name, style = MaterialTheme.typography.titleMedium)
                            Text(
                                text = "${money(line.product.price)} · ${line.product.unitLabel}",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            QuantityStepper(
                                quantity = line.quantity,
                                onDecrease = {
                                    onSetQuantity(line.product.id, line.quantity - 1)
                                },
                                onIncrease = {
                                    onSetQuantity(line.product.id, line.quantity + 1)
                                }
                            )
                        }
                        Column(horizontalAlignment = Alignment.End) {
                            Text(money(line.lineTotal), style = MaterialTheme.typography.titleMedium)
                            TextButton(onClick = { onRemove(line.product.id) }) {
                                Text("Remove")
                            }
                        }
                    }
                }
            }

            item {
                HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text("Subtotal", style = MaterialTheme.typography.bodyLarge)
                    Text(money(cart.subtotal), style = MaterialTheme.typography.bodyLarge)
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text("Tax (8%)", style = MaterialTheme.typography.bodyMedium)
                    Text(money(cart.tax), style = MaterialTheme.typography.bodyMedium)
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text("Total", style = MaterialTheme.typography.titleLarge)
                    Text(money(cart.total), style = MaterialTheme.typography.titleLarge)
                }
            }

            item {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Pickup name") },
                    singleLine = true
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = notes,
                    onValueChange = { notes = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Order notes (optional)") }
                )
                Spacer(Modifier.height(12.dp))
                Button(
                    onClick = { onPlaceOrder(name, notes) },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(14.dp)
                ) {
                    Text("Place pickup order")
                }
                TextButton(onClick = onClear) {
                    Text("Clear bag")
                }
                if (checkoutMessage != null) {
                    Surface(
                        shape = RoundedCornerShape(12.dp),
                        color = MaterialTheme.colorScheme.primaryContainer,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Column(modifier = Modifier.padding(14.dp)) {
                            Text(checkoutMessage, style = MaterialTheme.typography.titleMedium)
                            TextButton(onClick = onClearMessage) { Text("Dismiss") }
                        }
                    }
                }
                Spacer(Modifier.height(24.dp))
            }
        }
    }
}
