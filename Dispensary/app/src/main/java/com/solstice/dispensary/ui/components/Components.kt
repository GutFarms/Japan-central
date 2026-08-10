package com.solstice.dispensary.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Remove
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.R
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.StrainType
import com.solstice.dispensary.ui.theme.Amber
import com.solstice.dispensary.ui.theme.Charcoal
import com.solstice.dispensary.ui.theme.Ivory
import com.solstice.dispensary.ui.theme.Sage
import com.solstice.dispensary.ui.theme.SageDeep
import java.text.NumberFormat
import java.util.Locale

fun money(amount: Double): String =
    NumberFormat.getCurrencyInstance(Locale.US).format(amount)

@Composable
fun BrandLogo(
    modifier: Modifier = Modifier,
    size: Dp = 120.dp,
    contentDescription: String = "Native Pure"
) {
    Image(
        painter = painterResource(id = R.drawable.logo_main),
        contentDescription = contentDescription,
        modifier = modifier
            .size(size)
            .clip(RoundedCornerShape(size / 8)),
        contentScale = ContentScale.Crop
    )
}

@Composable
fun BrandMark(
    modifier: Modifier = Modifier,
    compact: Boolean = false,
    showWordmark: Boolean = false
) {
    val logoSize = if (compact) 56.dp else 112.dp
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.Start
    ) {
        BrandLogo(size = logoSize)
        if (showWordmark) {
            Spacer(Modifier.height(8.dp))
            Text(
                text = "NATIVE PURE",
                style = if (compact) MaterialTheme.typography.titleLarge else MaterialTheme.typography.displayMedium,
                color = MaterialTheme.colorScheme.onBackground,
                fontWeight = FontWeight.Bold
            )
            Text(
                text = "DISPENSARY",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.secondary
            )
        }
    }
}

@Composable
fun CategoryChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit
) {
    Surface(
        shape = RoundedCornerShape(20.dp),
        color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant,
        contentColor = if (selected) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.clickable(onClick = onClick)
    ) {
        Text(
            text = label,
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
            style = MaterialTheme.typography.labelLarge
        )
    }
}

@Composable
fun ProductTile(
    product: Product,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 1.dp,
        shadowElevation = 0.dp
    ) {
        Row(
            modifier = Modifier.padding(14.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            ProductSwatch(product = product)
            Spacer(Modifier.width(14.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = product.name,
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Text(
                    text = "${product.brand} · ${product.unitLabel}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(4.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (!product.published) {
                        MetaPill("Draft")
                    }
                    if (product.strainType != StrainType.NONE) {
                        MetaPill(product.strainType.label)
                    }
                    if (product.thcPercent > 0) {
                        MetaPill("THC ${product.thcPercent}%")
                    }
                }
            }
            Text(
                text = money(product.price),
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.primary
            )
        }
    }
}

@Composable
fun ProductSwatch(product: Product, modifier: Modifier = Modifier.size(56.dp)) {
    val colors = swatchColors(product.category)
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(14.dp))
            .background(Brush.linearGradient(colors)),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = product.category.label.take(1),
            style = MaterialTheme.typography.titleLarge,
            color = Ivory,
            fontWeight = FontWeight.Bold
        )
    }
}

fun swatchColors(category: ProductCategory): List<Color> = when (category) {
    ProductCategory.FLOWER -> listOf(SageDeep, Sage)
    ProductCategory.PREROLL -> listOf(Color(0xFF3A4A3C), Amber)
    ProductCategory.EDIBLE -> listOf(Color(0xFF5C4033), Amber)
    ProductCategory.CONCENTRATE -> listOf(Color(0xFF2C3A45), Color(0xFF6B8FA3))
    ProductCategory.VAPE -> listOf(Charcoal, SageDeep)
    ProductCategory.TOPICAL -> listOf(Color(0xFF4A5C52), MistSoft)
    ProductCategory.ACCESSORY -> listOf(Color(0xFF2F3431), Color(0xFF6E7671))
}

private val MistSoft = Color(0xFFA8C0B0)

@Composable
fun MetaPill(text: String) {
    Surface(
        shape = RoundedCornerShape(6.dp),
        color = MaterialTheme.colorScheme.surfaceVariant
    ) {
        Text(
            text = text,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

@Composable
fun QuantityStepper(
    quantity: Int,
    onDecrease: () -> Unit,
    onIncrease: () -> Unit
) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        IconButton(
            onClick = onDecrease,
            modifier = Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
        ) {
            Icon(Icons.Outlined.Remove, contentDescription = "Decrease")
        }
        Text(
            text = quantity.toString(),
            modifier = Modifier.padding(horizontal = 12.dp),
            style = MaterialTheme.typography.titleMedium
        )
        IconButton(
            onClick = onIncrease,
            modifier = Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.primary)
        ) {
            Icon(
                Icons.Outlined.Add,
                contentDescription = "Increase",
                tint = MaterialTheme.colorScheme.onPrimary
            )
        }
    }
}

@Composable
fun SectionHeader(title: String, subtitle: String? = null) {
    Column(modifier = Modifier.padding(bottom = 12.dp)) {
        Text(text = title, style = MaterialTheme.typography.headlineMedium)
        if (subtitle != null) {
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
fun HeroBackdrop(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val isDark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    val gradient = if (isDark) {
        listOf(Charcoal, Color(0xFF1A1410), Color(0xFF2A1C14))
    } else {
        listOf(Color(0xFF1A1410), Color(0xFF2E2316), Color(0xFF3A2A1C))
    }
    Box(
        modifier = modifier
            .fillMaxWidth()
            .background(Brush.verticalGradient(gradient))
            .padding(20.dp)
    ) {
        content()
    }
}
