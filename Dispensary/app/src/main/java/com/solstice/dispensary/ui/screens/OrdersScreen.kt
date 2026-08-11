package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.Order
import com.solstice.dispensary.data.model.OrderStatus
import com.solstice.dispensary.ui.components.SectionHeader
import com.solstice.dispensary.ui.components.money
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun OrdersScreen(
    orders: List<Order>,
    canManageOrders: Boolean = false,
    onUpdateStatus: (orderId: String, status: String) -> Unit = { _, _ -> },
    onShareReceipt: (orderId: String) -> Unit = {}
) {
    val dateFormat = SimpleDateFormat("MMM d · h:mm a", Locale.US)

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        item {
            SectionHeader(
                title = "Orders",
                subtitle = "Pickup history on this device"
            )
        }

        if (orders.isEmpty()) {
            item {
                Text(
                    text = "No orders yet. Place a pickup order from your bag.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }

        items(orders, key = { it.id }) { order ->
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text("#${order.id}", style = MaterialTheme.typography.titleLarge)
                        Text(
                            money(order.total),
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.primary
                        )
                    }
                    Spacer(Modifier.height(4.dp))
                    Text(
                        text = dateFormat.format(Date(order.createdAt)),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = "${order.status} · ${order.itemCount} items · ${order.pickupName}",
                        style = MaterialTheme.typography.bodyMedium
                    )
                    if (order.discount > 0) {
                        Text(
                            "Redeemed ${order.pointsRedeemed} pts (−${money(order.discount)})",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.secondary
                        )
                    }
                    if (order.pointsEarned > 0) {
                        Text(
                            text = "+${order.pointsEarned} loyalty points",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.secondary
                        )
                    }
                    if (order.customerEmail.isNotBlank()) {
                        Text(
                            text = order.customerEmail,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    if (order.notes.isNotBlank()) {
                        Text(
                            text = order.notes,
                            style = MaterialTheme.typography.bodyMedium
                        )
                    }
                    TextButton(onClick = { onShareReceipt(order.id) }) {
                        Text("Share receipt")
                    }
                    if (canManageOrders) {
                        Spacer(Modifier.height(4.dp))
                        Row(
                            modifier = Modifier.horizontalScroll(rememberScrollState()),
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            OrderStatus.staffActions.forEach { status ->
                                FilterChip(
                                    selected = order.status == status,
                                    onClick = { onUpdateStatus(order.id, status) },
                                    label = { Text(status) }
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
