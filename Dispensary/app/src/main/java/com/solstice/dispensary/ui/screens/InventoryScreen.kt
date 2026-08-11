package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.DocumentScanner
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
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
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.ui.components.MetaPill
import com.solstice.dispensary.ui.components.ProductSwatch
import com.solstice.dispensary.ui.components.SectionHeader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun InventoryScreen(
    products: List<Product>,
    intakes: List<InventoryIntake>,
    intakeMessage: String?,
    onClearMessage: () -> Unit,
    onOpenScanner: () -> Unit,
    onAdjustStock: (String, Int) -> Unit,
    onSetPublished: (String, Boolean) -> Unit
) {
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(intakeMessage) {
        if (intakeMessage != null) {
            snackbar.showSnackbar(intakeMessage)
            onClearMessage()
        }
    }

    var query by remember { mutableStateOf("") }
    var filterCategory by remember { mutableStateOf<ProductCategory?>(null) }
    var filterPublished by remember { mutableStateOf("All") }

    val filtered = remember(products, query, filterCategory, filterPublished) {
        products.filter { product ->
            val q = query.trim()
            val matchesQuery = q.isEmpty() ||
                product.name.contains(q, ignoreCase = true) ||
                product.brand.contains(q, ignoreCase = true) ||
                product.sku.contains(q, ignoreCase = true) ||
                product.category.label.contains(q, ignoreCase = true)
            val matchesCategory = filterCategory == null || product.category == filterCategory
            val matchesPublished = when (filterPublished) {
                "Live" -> product.published
                "Draft" -> !product.published
                else -> true
            }
            matchesQuery && matchesCategory && matchesPublished
        }
    }

    val lowStock = products.count { it.stockQuantity <= 5 }
    val draftCount = products.count { !it.published }
    val dateFormat = remember { SimpleDateFormat("MMM d · h:mm a", Locale.US) }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        floatingActionButton = {
            FloatingActionButton(onClick = onOpenScanner) {
                Icon(Icons.Outlined.DocumentScanner, contentDescription = "Scan inventory")
            }
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            contentPadding = PaddingValues(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            item {
                SectionHeader(
                    title = "Inventory",
                    subtitle = "${products.size} SKUs · $lowStock low stock" +
                        if (draftCount > 0) " · $draftCount unpublished" else ""
                )
                Text(
                    text = "Drafts stay in Stock until they have a SKU and price, then publish to the customer menu.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(8.dp))
                Button(
                    onClick = onOpenScanner,
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(14.dp)
                ) {
                    Icon(Icons.Outlined.DocumentScanner, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text("AI camera scan to add stock")
                }
            }

            item {
                Text("Stock levels", style = MaterialTheme.typography.titleLarge)
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    label = { Text("Search name, brand, or SKU") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    shape = RoundedCornerShape(14.dp)
                )
                Spacer(Modifier.height(8.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = filterCategory == null,
                        onClick = { filterCategory = null },
                        label = { Text("All categories") }
                    )
                    ProductCategory.entries.forEach { cat ->
                        FilterChip(
                            selected = filterCategory == cat,
                            onClick = { filterCategory = cat },
                            label = { Text(cat.label) }
                        )
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("All", "Live", "Draft").forEach { status ->
                        FilterChip(
                            selected = filterPublished == status,
                            onClick = { filterPublished = status },
                            label = { Text(status) }
                        )
                    }
                }
                Text(
                    text = "Showing ${filtered.size} of ${products.size}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            items(filtered, key = { it.id }) { product ->
                Surface(
                    shape = RoundedCornerShape(14.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            ProductSwatch(product = product)
                            Column(
                                modifier = Modifier
                                    .weight(1f)
                                    .padding(horizontal = 12.dp)
                            ) {
                                Text(product.name, style = MaterialTheme.typography.titleMedium)
                                Text(
                                    text = "${product.brand} · ${product.sku}",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                    MetaPill(product.category.label)
                                    MetaPill("${product.stockQuantity} in stock")
                                    MetaPill(if (product.published) "Published" else "Draft")
                                }
                            }
                            Column(horizontalAlignment = Alignment.End) {
                                TextButton(onClick = { onAdjustStock(product.id, 1) }) {
                                    Text("+1")
                                }
                                TextButton(onClick = { onAdjustStock(product.id, -1) }) {
                                    Text("−1")
                                }
                            }
                        }
                        TextButton(
                            onClick = { onSetPublished(product.id, !product.published) }
                        ) {
                            Text(if (product.published) "Unpublish from menu" else "Publish to customers")
                        }
                    }
                }
            }

            if (intakes.isNotEmpty()) {
                item {
                    Spacer(Modifier.height(8.dp))
                    Text("Recent AI intakes", style = MaterialTheme.typography.titleLarge)
                }
                items(intakes.take(12), key = { it.id }) { intake ->
                    Surface(
                        shape = RoundedCornerShape(12.dp),
                        color = MaterialTheme.colorScheme.surfaceVariant,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Column(modifier = Modifier.padding(14.dp)) {
                            Text(
                                "+${intake.quantityAdded} × ${intake.productName}",
                                style = MaterialTheme.typography.titleMedium
                            )
                            Text(
                                text = "${dateFormat.format(Date(intake.createdAt))} · " +
                                    "${(intake.confidence * 100).toInt()}% confidence",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }
            }
        }
    }
}
