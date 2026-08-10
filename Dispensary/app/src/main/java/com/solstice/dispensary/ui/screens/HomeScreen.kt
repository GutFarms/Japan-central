package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ShoppingBag
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.ui.components.BrandLogo
import com.solstice.dispensary.ui.components.HeroBackdrop
import com.solstice.dispensary.ui.components.ProductTile
import com.solstice.dispensary.ui.components.SectionHeader
import com.solstice.dispensary.ui.theme.Amber
import com.solstice.dispensary.ui.theme.Charcoal
import com.solstice.dispensary.ui.theme.Sage

@Composable
fun HomeScreen(
    featured: List<Product>,
    cartCount: Int,
    showInventory: Boolean = false,
    onOpenMenu: () -> Unit,
    onOpenProduct: (String) -> Unit,
    onOpenCart: () -> Unit,
    onOpenStore: () -> Unit,
    onOpenInventory: () -> Unit,
    onSelectCategory: (ProductCategory) -> Unit
) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 24.dp)
    ) {
        item {
            HeroBackdrop {
                Column {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.Top
                    ) {
                        BrandLogo(size = 96.dp)
                        if (cartCount > 0) {
                            Surface(
                                shape = RoundedCornerShape(20.dp),
                                color = Amber,
                                modifier = Modifier.clickable(onClick = onOpenCart)
                            ) {
                                Row(
                                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    Icon(
                                        Icons.Outlined.ShoppingBag,
                                        contentDescription = "Cart",
                                        tint = Charcoal
                                    )
                                    Spacer(Modifier.width(6.dp))
                                    Text(
                                        text = cartCount.toString(),
                                        color = Charcoal,
                                        style = MaterialTheme.typography.titleMedium
                                    )
                                }
                            }
                        }
                    }
                    Spacer(Modifier.height(18.dp))
                    Text(
                        text = "Curated flower, edibles, and concentrates for pickup.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = Sage
                    )
                    Spacer(Modifier.height(18.dp))
                    Button(
                        onClick = onOpenMenu,
                        shape = RoundedCornerShape(12.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = Amber,
                            contentColor = Charcoal
                        )
                    ) {
                        Text("Browse menu")
                    }
                }
            }
        }

        item {
            Column(modifier = Modifier.padding(20.dp)) {
                SectionHeader(
                    title = "Shop by type",
                    subtitle = "Flower, edibles, vapes, and more"
                )
                Row(
                    modifier = Modifier.horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    ProductCategory.entries.forEach { category ->
                        Surface(
                            shape = RoundedCornerShape(14.dp),
                            color = MaterialTheme.colorScheme.surfaceVariant,
                            modifier = Modifier
                                .width(120.dp)
                                .clickable {
                                    onSelectCategory(category)
                                    onOpenMenu()
                                }
                        ) {
                            Column(modifier = Modifier.padding(14.dp)) {
                                Text(
                                    text = category.label,
                                    style = MaterialTheme.typography.titleMedium
                                )
                                Text(
                                    text = "View",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.primary
                                )
                            }
                        }
                    }
                }
            }
        }

        item {
            Column(modifier = Modifier.padding(horizontal = 20.dp)) {
                SectionHeader(
                    title = "Featured",
                    subtitle = "Staff picks this week"
                )
            }
        }

        items(featured, key = { it.id }) { product ->
            ProductTile(
                product = product,
                onClick = { onOpenProduct(product.id) },
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 6.dp)
            )
        }

        if (showInventory) {
            item {
                Surface(
                    modifier = Modifier
                        .padding(horizontal = 20.dp)
                        .fillMaxWidth()
                        .clickable(onClick = onOpenInventory),
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.secondaryContainer
                ) {
                    Column(modifier = Modifier.padding(18.dp)) {
                        Text("AI inventory scanner", style = MaterialTheme.typography.titleLarge)
                        Text(
                            text = "Scan labels or barcodes to add stock",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.8f)
                        )
                    }
                }
            }
        }

        item {
            Surface(
                modifier = Modifier
                    .padding(20.dp)
                    .fillMaxWidth()
                    .clickable(onClick = onOpenStore),
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.primaryContainer
            ) {
                Column(modifier = Modifier.padding(18.dp)) {
                    Text("Visit the shop", style = MaterialTheme.typography.titleLarge)
                    Text(
                        text = "Hours, address, and pickup details",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.8f)
                    )
                }
            }
        }
    }
}
