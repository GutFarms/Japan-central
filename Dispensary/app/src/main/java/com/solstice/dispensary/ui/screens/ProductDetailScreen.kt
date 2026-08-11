package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductSize
import com.solstice.dispensary.data.model.StrainType
import com.solstice.dispensary.ui.components.MetaPill
import com.solstice.dispensary.ui.components.ProductSwatch
import com.solstice.dispensary.ui.components.QuantityStepper
import com.solstice.dispensary.ui.components.money
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ProductDetailScreen(
    product: Product?,
    onBack: () -> Unit,
    onAddToCart: (String, Int, ProductSize?) -> Unit
) {
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    var quantity by remember { mutableIntStateOf(1) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(product?.name ?: "Product") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        },
        snackbarHost = { SnackbarHost(snackbar) }
    ) { padding ->
        if (product == null) {
            Text(
                text = "This product isn’t available on the menu. It may be unpublished or removed.",
                modifier = Modifier.padding(padding).padding(20.dp),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            return@Scaffold
        }

        val sizeOffers = remember(product) { product.offeredSizes() }
        var selectedSize by remember(product.id) {
            mutableStateOf(
                sizeOffers.firstOrNull { it.inStock }?.size
                    ?: sizeOffers.firstOrNull()?.size
            )
        }
        val unitPrice = product.effectivePriceFor(selectedSize)
        val shelfPrice = product.shelfPriceFor(selectedSize)
        val sizeInStock = if (product.sizeInventoryEnabled) {
            selectedSize != null && product.stockFor(selectedSize) > 0
        } else {
            product.inStock
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(20.dp)
        ) {
            ProductSwatch(
                product = product,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(120.dp)
            )
            Spacer(Modifier.height(18.dp))
            Text(product.name, style = MaterialTheme.typography.headlineLarge)
            Text(
                text = "${product.brand} · ${product.category.label}",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (product.strainType != StrainType.NONE) {
                    MetaPill(product.strainType.label)
                }
                if (product.thcPercent > 0) MetaPill("THC ${product.thcPercent}%")
                if (product.cbdPercent > 0) MetaPill("CBD ${product.cbdPercent}%")
                MetaPill(product.displayUnitLabel(selectedSize))
            }

            if (product.sizeInventoryEnabled && sizeOffers.isNotEmpty()) {
                Spacer(Modifier.height(16.dp))
                Text("Size", style = MaterialTheme.typography.titleLarge)
                Spacer(Modifier.height(8.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    sizeOffers.forEach { offer ->
                        FilterChip(
                            selected = selectedSize == offer.size,
                            onClick = { selectedSize = offer.size },
                            enabled = offer.inStock || selectedSize == offer.size,
                            label = {
                                Text(
                                    if (offer.inStock) {
                                        "${offer.size.label} · ${money(product.effectivePriceFor(offer.size))}"
                                    } else {
                                        "${offer.size.label} · sold out"
                                    }
                                )
                            }
                        )
                    }
                }
            }

            Spacer(Modifier.height(16.dp))
            if (product.hasActiveDeal) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    MetaPill(product.dealLabel.ifBlank { "Deal" })
                    MetaPill("−${product.dealPercent}%")
                }
                Spacer(Modifier.height(8.dp))
                Text(
                    text = money(shelfPrice),
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textDecoration = TextDecoration.LineThrough
                )
                Text(
                    text = money(unitPrice),
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.primary
                )
            } else {
                Text(
                    text = money(unitPrice),
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.primary
                )
            }
            Spacer(Modifier.height(16.dp))
            Text("About", style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(6.dp))
            Text(product.description, style = MaterialTheme.typography.bodyLarge)
            Spacer(Modifier.height(14.dp))
            Text("Effects", style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(6.dp))
            Text(
                text = product.effects,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(24.dp))
            QuantityStepper(
                quantity = quantity,
                onDecrease = { if (quantity > 1) quantity -= 1 },
                onIncrease = { quantity += 1 }
            )
            Spacer(Modifier.height(16.dp))
            Button(
                onClick = {
                    onAddToCart(product.id, quantity, selectedSize)
                    scope.launch {
                        val sizeNote = selectedSize?.let { " (${it.label})" }.orEmpty()
                        snackbar.showSnackbar("Added ${product.name}$sizeNote to bag")
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp),
                enabled = sizeInStock
            ) {
                Text(
                    text = if (sizeInStock) {
                        "Add to bag · ${money(unitPrice * quantity)}"
                    } else {
                        "Out of stock"
                    },
                    modifier = Modifier.padding(vertical = 4.dp)
                )
            }
        }
    }
}
