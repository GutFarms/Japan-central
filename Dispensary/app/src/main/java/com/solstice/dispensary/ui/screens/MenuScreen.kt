package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.ui.components.CategoryChip
import com.solstice.dispensary.ui.components.ProductTile
import com.solstice.dispensary.ui.components.SectionHeader

@Composable
fun MenuScreen(
    products: List<Product>,
    selectedCategory: ProductCategory?,
    searchQuery: String,
    onSearchChange: (String) -> Unit,
    onSelectCategory: (ProductCategory?) -> Unit,
    onOpenProduct: (String) -> Unit
) {
    Column(modifier = Modifier.fillMaxSize()) {
        Column(modifier = Modifier.padding(horizontal = 20.dp, vertical = 16.dp)) {
            SectionHeader(
                title = "Menu",
                subtitle = "${products.size} products"
            )
            OutlinedTextField(
                value = searchQuery,
                onValueChange = onSearchChange,
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                placeholder = { Text("Search strains, brands, effects") },
                leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null) }
            )
            Row(
                modifier = Modifier
                    .padding(top = 12.dp)
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                CategoryChip(
                    category = null,
                    label = "All",
                    selected = selectedCategory == null,
                    onClick = { onSelectCategory(null) }
                )
                ProductCategory.entries.forEach { category ->
                    CategoryChip(
                        category = category,
                        label = category.label,
                        selected = selectedCategory == category,
                        onClick = { onSelectCategory(category) }
                    )
                }
            }
        }

        LazyColumn(
            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            items(products, key = { it.id }) { product ->
                ProductTile(
                    product = product,
                    onClick = { onOpenProduct(product.id) }
                )
            }
            if (products.isEmpty()) {
                item {
                    Text(
                        text = "No products match your search.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 24.dp)
                    )
                }
            }
        }
    }
}
