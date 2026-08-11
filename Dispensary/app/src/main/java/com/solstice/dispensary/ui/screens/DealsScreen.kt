package com.solstice.dispensary.ui.screens

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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.ui.components.BotanicalScreenBackground
import com.solstice.dispensary.ui.components.LeafEmptyState
import com.solstice.dispensary.ui.components.MetaPill
import com.solstice.dispensary.ui.components.ProductSwatch
import com.solstice.dispensary.ui.components.SectionHeader
import com.solstice.dispensary.ui.components.money

@Composable
fun DealsScreen(
    deals: List<Product>,
    onOpenProduct: (String) -> Unit
) {
    BotanicalScreenBackground {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            item {
                SectionHeader(
                    title = "Deals",
                    subtitle = if (deals.isEmpty()) {
                        "No active promos right now"
                    } else {
                        "${deals.size} active ${if (deals.size == 1) "deal" else "deals"}"
                    }
                )
                Text(
                    text = "Limited-time savings on select menu items. Deal prices apply at checkout.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            if (deals.isEmpty()) {
                item {
                    LeafEmptyState(
                        title = "Quiet on the specials board",
                        subtitle = "Check back soon — staff posts new deals from Stock."
                    )
                }
            }

            items(deals, key = { it.id }) { product ->
                Surface(
                    onClick = { onOpenProduct(product.id) },
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Row(
                        modifier = Modifier.padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        ProductSwatch(product = product)
                        Column(
                            modifier = Modifier
                                .weight(1f)
                                .padding(horizontal = 12.dp),
                            verticalArrangement = Arrangement.spacedBy(4.dp)
                        ) {
                            Text(product.name, style = MaterialTheme.typography.titleMedium)
                            Text(
                                "${product.brand} · ${product.unitLabel}",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                MetaPill(
                                    product.dealLabel.ifBlank { "${product.dealPercent}% off" }
                                )
                                MetaPill("−${product.dealPercent}%")
                            }
                        }
                        Column(horizontalAlignment = Alignment.End) {
                            Text(
                                money(product.price),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                textDecoration = TextDecoration.LineThrough
                            )
                            Text(
                                money(product.effectivePrice),
                                style = MaterialTheme.typography.titleMedium,
                                color = MaterialTheme.colorScheme.primary
                            )
                        }
                    }
                }
            }

            item { Spacer(Modifier.height(8.dp)) }
        }
    }
}
