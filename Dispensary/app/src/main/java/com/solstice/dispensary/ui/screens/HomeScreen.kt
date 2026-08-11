package com.solstice.dispensary.ui.screens

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ShoppingBag
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.update.UpdateUiState
import com.solstice.dispensary.ui.components.AppUpdateBanner
import com.solstice.dispensary.ui.components.BotanicalScreenBackground
import com.solstice.dispensary.ui.components.BrandLogo
import com.solstice.dispensary.ui.components.HeroBackdrop
import com.solstice.dispensary.ui.components.LeafEmptyState
import com.solstice.dispensary.ui.components.ProductTile
import com.solstice.dispensary.ui.components.SectionHeader
import com.solstice.dispensary.ui.theme.Amber
import com.solstice.dispensary.ui.theme.CanopyLight
import com.solstice.dispensary.ui.theme.Charcoal
import com.solstice.dispensary.ui.theme.Stem

@Composable
fun HomeScreen(
    featured: List<Product>,
    cartCount: Int,
    showInventory: Boolean = false,
    updateState: UpdateUiState = UpdateUiState.Idle,
    needsInstallPermission: Boolean = false,
    onDownloadUpdate: () -> Unit = {},
    onInstallUpdate: () -> Unit = {},
    onDismissUpdate: () -> Unit = {},
    onOpenInstallPermission: () -> Unit = {},
    onOpenMenu: () -> Unit,
    onOpenProduct: (String) -> Unit,
    onOpenCart: () -> Unit,
    onOpenStore: () -> Unit,
    onOpenInventory: () -> Unit,
    onSelectCategory: (ProductCategory) -> Unit
) {
    var visible by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { visible = true }
    val fade by animateFloatAsState(
        targetValue = if (visible) 1f else 0f,
        animationSpec = tween(700),
        label = "homeFade"
    )

    BotanicalScreenBackground {
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .alpha(fade),
            contentPadding = PaddingValues(bottom = 28.dp)
        ) {
            item {
                AppUpdateBanner(
                    state = updateState,
                    onDownload = onDownloadUpdate,
                    onInstall = onInstallUpdate,
                    onDismiss = onDismissUpdate,
                    onOpenInstallPermission = onOpenInstallPermission,
                    needsInstallPermission = needsInstallPermission
                )
            }
            item {
                HeroBackdrop {
                    Column {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.Top
                        ) {
                            BrandLogo(size = 92.dp)
                            if (cartCount > 0) {
                                TextButton(onClick = onOpenCart) {
                                    Icon(
                                        Icons.Outlined.ShoppingBag,
                                        contentDescription = "Cart",
                                        tint = CanopyLight
                                    )
                                    Spacer(Modifier.width(6.dp))
                                    Text(
                                        text = cartCount.toString(),
                                        color = CanopyLight,
                                        style = MaterialTheme.typography.titleMedium
                                    )
                                }
                            }
                        }
                        Spacer(Modifier.height(22.dp))
                        Text(
                            text = "Native Pure",
                            style = MaterialTheme.typography.displayMedium,
                            color = CanopyLight
                        )
                        Spacer(Modifier.height(8.dp))
                        Text(
                            text = "A living menu for calm evenings and clear mornings.",
                            style = MaterialTheme.typography.bodyLarge,
                            color = Stem
                        )
                        Spacer(Modifier.height(20.dp))
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
                Column(modifier = Modifier.padding(horizontal = 20.dp, vertical = 18.dp)) {
                    SectionHeader(
                        title = "Shop by type",
                        subtitle = "Wander the canopy by category"
                    )
                    Row(
                        modifier = Modifier.horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(12.dp)
                    ) {
                        ProductCategory.entries.forEach { category ->
                            Column(
                                modifier = Modifier
                                    .width(118.dp)
                                    .clickable {
                                        onSelectCategory(category)
                                        onOpenMenu()
                                    }
                                    .padding(vertical = 4.dp)
                            ) {
                                Text(
                                    text = category.label,
                                    style = MaterialTheme.typography.titleMedium
                                )
                                Text(
                                    text = "Open shelf",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.primary
                                )
                            }
                        }
                    }
                }
            }

            item {
                Column(modifier = Modifier.padding(horizontal = 20.dp)) {
                    SectionHeader(
                        title = "Featured blooms",
                        subtitle = "Staff picks from this week’s harvest"
                    )
                }
            }

            if (featured.isEmpty()) {
                item {
                    LeafEmptyState(
                        title = "Nothing flowering yet",
                        subtitle = "Published picks will appear here."
                    )
                }
            }

            itemsIndexed(featured, key = { _, product -> product.id }) { index, product ->
                val itemFade by animateFloatAsState(
                    targetValue = if (visible) 1f else 0f,
                    animationSpec = tween(500, delayMillis = 80 * index),
                    label = "featureFade$index"
                )
                ProductTile(
                    product = product,
                    onClick = { onOpenProduct(product.id) },
                    modifier = Modifier
                        .padding(horizontal = 20.dp, vertical = 6.dp)
                        .graphicsLayer { alpha = itemFade }
                )
            }

            if (showInventory) {
                item {
                    Column(
                        modifier = Modifier
                            .padding(horizontal = 20.dp, vertical = 8.dp)
                            .fillMaxWidth()
                            .clickable(onClick = onOpenInventory)
                    ) {
                        Text("AI inventory scanner", style = MaterialTheme.typography.titleLarge)
                        Text(
                            text = "Scan labels or barcodes to add draft stock",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }

            item {
                Column(
                    modifier = Modifier
                        .padding(20.dp)
                        .fillMaxWidth()
                        .clickable(onClick = onOpenStore)
                ) {
                    Text("Visit the shop", style = MaterialTheme.typography.titleLarge)
                    Text(
                        text = "Hours, address, and pickup under the greenhouse lights",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}
