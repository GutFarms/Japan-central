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
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.DocumentScanner
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.InventoryIntake
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.StrainType
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
    onSetPublished: (String, Boolean) -> Unit,
    onSaveProduct: (Product) -> Unit,
    onCreateDraft: () -> Unit
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
    var editing by remember { mutableStateOf<Product?>(null) }

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
                .imePadding()
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
                    text = "Admin and staff can edit name, price, SKU, and stock. Drafts stay hidden until published.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(
                        onClick = onCreateDraft,
                        modifier = Modifier.weight(1f),
                        shape = RoundedCornerShape(14.dp)
                    ) {
                        Text("Add product")
                    }
                    OutlinedButton(
                        onClick = onOpenScanner,
                        modifier = Modifier.weight(1f),
                        shape = RoundedCornerShape(14.dp)
                    ) {
                        Icon(Icons.Outlined.DocumentScanner, contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text("AI scan")
                    }
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

            editing?.let { product ->
                item {
                    ProductEditorCard(
                        product = product,
                        onCancel = { editing = null },
                        onSave = { updated ->
                            onSaveProduct(updated)
                            editing = null
                        }
                    )
                }
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
                                    text = "${product.brand} · ${product.sku.ifBlank { "no SKU" }} · $${"%.2f".format(product.price)}",
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
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            TextButton(onClick = { editing = product }) {
                                Text("Edit")
                            }
                            TextButton(
                                onClick = { onSetPublished(product.id, !product.published) }
                            ) {
                                Text(if (product.published) "Unpublish" else "Publish")
                            }
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ProductEditorCard(
    product: Product,
    onCancel: () -> Unit,
    onSave: (Product) -> Unit
) {
    var name by remember(product.id) { mutableStateOf(product.name) }
    var brand by remember(product.id) { mutableStateOf(product.brand) }
    var sku by remember(product.id) { mutableStateOf(product.sku) }
    var price by remember(product.id) { mutableStateOf(if (product.price > 0) product.price.toString() else "") }
    var stock by remember(product.id) { mutableStateOf(product.stockQuantity.toString()) }
    var unit by remember(product.id) { mutableStateOf(product.unitLabel) }
    var thc by remember(product.id) { mutableStateOf(product.thcPercent.toString()) }
    var cbd by remember(product.id) { mutableStateOf(product.cbdPercent.toString()) }
    var description by remember(product.id) { mutableStateOf(product.description) }
    var effects by remember(product.id) { mutableStateOf(product.effects) }
    var category by remember(product.id) { mutableStateOf(product.category) }
    var strain by remember(product.id) { mutableStateOf(product.strainType) }
    var published by remember(product.id) { mutableStateOf(product.published) }
    var featured by remember(product.id) { mutableStateOf(product.featured) }

    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                "Edit product",
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSecondaryContainer
            )
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Name") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            OutlinedTextField(
                value = brand,
                onValueChange = { brand = it },
                label = { Text("Brand") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            OutlinedTextField(
                value = sku,
                onValueChange = { sku = it },
                label = { Text("SKU (required to publish)") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = price,
                    onValueChange = { price = it.filter { ch -> ch.isDigit() || ch == '.' } },
                    label = { Text("Price") },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
                )
                OutlinedTextField(
                    value = stock,
                    onValueChange = { stock = it.filter { ch -> ch.isDigit() } },
                    label = { Text("Stock") },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number)
                )
            }
            OutlinedTextField(
                value = unit,
                onValueChange = { unit = it },
                label = { Text("Unit (e.g. 3.5g, each)") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = thc,
                    onValueChange = { thc = it.filter { ch -> ch.isDigit() || ch == '.' } },
                    label = { Text("THC %") },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
                )
                OutlinedTextField(
                    value = cbd,
                    onValueChange = { cbd = it.filter { ch -> ch.isDigit() || ch == '.' } },
                    label = { Text("CBD %") },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
                )
            }
            Text("Category", style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ProductCategory.entries.forEach { cat ->
                    FilterChip(
                        selected = category == cat,
                        onClick = { category = cat },
                        label = { Text(cat.label) }
                    )
                }
            }
            Text("Strain", style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                StrainType.entries.forEach { type ->
                    FilterChip(
                        selected = strain == type,
                        onClick = { strain = type },
                        label = { Text(type.label) }
                    )
                }
            }
            OutlinedTextField(
                value = description,
                onValueChange = { description = it },
                label = { Text("Description") },
                modifier = Modifier.fillMaxWidth(),
                minLines = 2
            )
            OutlinedTextField(
                value = effects,
                onValueChange = { effects = it },
                label = { Text("Effects") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = published,
                    onClick = { published = !published },
                    label = { Text(if (published) "Published" else "Draft") }
                )
                FilterChip(
                    selected = featured,
                    onClick = { featured = !featured },
                    label = { Text(if (featured) "Featured" else "Not featured") }
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = onCancel, modifier = Modifier.weight(1f)) {
                    Text("Cancel")
                }
                Button(
                    onClick = {
                        onSave(
                            product.copy(
                                name = name,
                                brand = brand,
                                sku = sku,
                                price = price.toDoubleOrNull() ?: 0.0,
                                stockQuantity = stock.toIntOrNull() ?: 0,
                                unitLabel = unit,
                                thcPercent = thc.toDoubleOrNull() ?: 0.0,
                                cbdPercent = cbd.toDoubleOrNull() ?: 0.0,
                                description = description,
                                effects = effects,
                                category = category,
                                strainType = strain,
                                published = published,
                                featured = featured
                            )
                        )
                    },
                    modifier = Modifier.weight(1f),
                    enabled = name.trim().length >= 2
                ) {
                    Text("Save changes")
                }
            }
        }
    }
}
