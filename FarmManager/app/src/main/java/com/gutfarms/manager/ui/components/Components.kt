package com.gutfarms.manager.ui.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Forest
import com.gutfarms.manager.ui.theme.ForestDeep
import com.gutfarms.manager.ui.theme.Mist
import com.gutfarms.manager.ui.theme.Wheat
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.launch
import java.text.NumberFormat
import java.text.SimpleDateFormat
import java.util.Currency
import java.util.Date
import java.util.Locale
import kotlin.math.abs
import kotlin.math.min

fun formatMoney(amount: Double): String {
    val format = NumberFormat.getCurrencyInstance(Locale.US)
    format.currency = Currency.getInstance("USD")
    return format.format(amount)
}

fun formatPercent(value: Double): String = String.format(Locale.US, "%.1f%%", value)

fun formatDate(millis: Long): String =
    SimpleDateFormat("MMM d, yyyy", Locale.US).format(Date(millis))

fun parseDateInput(value: String): Long? {
    return try {
        SimpleDateFormat("yyyy-MM-dd", Locale.US).parse(value.trim())?.time
    } catch (_: Exception) {
        null
    }
}

fun formatDateInput(millis: Long): String =
    SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date(millis))

@Composable
fun ScreenHeader(
    brand: String,
    title: String,
    subtitle: String,
    modifier: Modifier = Modifier,
    onBrandClick: (() -> Unit)? = null
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(bottomStart = 28.dp, bottomEnd = 28.dp))
            .background(
                Brush.verticalGradient(
                    colors = listOf(ForestDeep, Forest, Color(0xFF3F7A4D))
                )
            )
            .padding(horizontal = 20.dp, vertical = 28.dp)
    ) {
        Column {
            Text(
                text = brand,
                style = MaterialTheme.typography.displayMedium,
                color = Wheat,
                modifier = if (onBrandClick != null) {
                    Modifier.clickable(onClick = onBrandClick)
                } else {
                    Modifier
                }
            )
            if (onBrandClick != null) {
                Text(
                    text = "Tap name to rename farm",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Mist.copy(alpha = 0.85f),
                    modifier = Modifier
                        .padding(top = 2.dp)
                        .clickable(onClick = onBrandClick)
                )
            }
            Spacer(Modifier.height(8.dp))
            Text(
                text = title,
                style = MaterialTheme.typography.titleLarge,
                color = CreamLeaf
            )
            Spacer(Modifier.height(4.dp))
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = Mist.copy(alpha = 0.92f)
            )
        }
    }
}

@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onBackground,
        modifier = modifier.padding(horizontal = 4.dp)
    )
}

@Composable
fun MetricTile(
    label: String,
    value: String,
    accent: Color = MaterialTheme.colorScheme.primary,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(18.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(16.dp)
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(6.dp))
        Text(
            text = value,
            style = MaterialTheme.typography.headlineMedium,
            color = accent
        )
    }
}

@Composable
fun EmptyHint(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(vertical = 12.dp)
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun <T> SimpleDropdown(
    label: String,
    options: List<T>,
    selected: T,
    onSelected: (T) -> Unit,
    optionLabel: (T) -> String,
    modifier: Modifier = Modifier
) {
    if (options.size > 5) {
        BubbleScrollPicker(
            label = label,
            options = options,
            selected = selected,
            onSelected = onSelected,
            optionLabel = optionLabel,
            modifier = modifier
        )
        return
    }

    var expanded by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { expanded = !expanded },
        modifier = modifier.fillMaxWidth()
    ) {
        OutlinedTextField(
            value = optionLabel(selected),
            onValueChange = {},
            readOnly = true,
            label = { Text(label) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded) },
            modifier = Modifier
                .menuAnchor()
                .fillMaxWidth()
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            options.forEach { option ->
                DropdownMenuItem(
                    text = { Text(optionLabel(option)) },
                    onClick = {
                        onSelected(option)
                        expanded = false
                    }
                )
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun <T> BubbleScrollPicker(
    label: String,
    options: List<T>,
    selected: T,
    onSelected: (T) -> Unit,
    optionLabel: (T) -> String,
    modifier: Modifier = Modifier,
    visibleCount: Int = 5,
    itemHeight: Dp = 44.dp
) {
    if (options.isEmpty()) return

    val density = LocalDensity.current
    val itemHeightPx = with(density) { itemHeight.toPx() }
    val pickerHeight = itemHeight * visibleCount
    val sidePad = visibleCount / 2
    val selectedIndex = options.indexOf(selected).coerceAtLeast(0)
    val listState = rememberLazyListState(initialFirstVisibleItemIndex = selectedIndex)
    val fling = rememberSnapFlingBehavior(lazyListState = listState)
    val scope = rememberCoroutineScope()

    LaunchedEffect(selectedIndex, options) {
        if (!listState.isScrollInProgress) {
            listState.scrollToItem(selectedIndex)
        }
    }

    LaunchedEffect(listState, options) {
        snapshotFlow {
            Triple(
                listState.isScrollInProgress,
                listState.firstVisibleItemIndex,
                listState.firstVisibleItemScrollOffset
            )
        }
            .filter { (scrolling, _, _) -> !scrolling }
            .distinctUntilChanged()
            .collect { (_, first, offset) ->
                val centered = if (offset > itemHeightPx / 2f) first + 1 else first
                val clamped = centered.coerceIn(0, options.lastIndex)
                if (options[clamped] != selected) {
                    onSelected(options[clamped])
                }
            }
    }

    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(bottom = 6.dp, start = 4.dp)
        )
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(pickerHeight)
                .clip(RoundedCornerShape(20.dp))
                .background(
                    Brush.verticalGradient(
                        colors = listOf(
                            Mist.copy(alpha = 0.55f),
                            Color.White,
                            Mist.copy(alpha = 0.55f)
                        )
                    )
                )
        ) {
            // Center selection bubble
            Box(
                modifier = Modifier
                    .align(Alignment.Center)
                    .fillMaxWidth()
                    .height(itemHeight)
                    .padding(horizontal = 10.dp)
                    .clip(RoundedCornerShape(999.dp))
                    .background(Forest.copy(alpha = 0.14f))
            )

            LazyColumn(
                state = listState,
                flingBehavior = fling,
                modifier = Modifier.fillMaxWidth(),
                contentPadding = PaddingValues(vertical = itemHeight * sidePad),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                itemsIndexed(options, key = { index, _ -> index }) { index, option ->
                    // Subscribe to scroll so bubble scale/alpha updates while dragging.
                    listState.firstVisibleItemIndex
                    listState.firstVisibleItemScrollOffset

                    val layoutInfo = listState.layoutInfo
                    val itemInfo = layoutInfo.visibleItemsInfo.firstOrNull { it.index == index }
                    val viewportCenter = layoutInfo.viewportStartOffset +
                        (layoutInfo.viewportEndOffset - layoutInfo.viewportStartOffset) / 2f
                    val itemCenter = itemInfo?.let { it.offset + it.size / 2f } ?: viewportCenter
                    val distance = abs(itemCenter - viewportCenter)
                    val maxDistance = itemHeightPx * sidePad
                    val fraction = if (maxDistance == 0f) 0f else min(1f, distance / maxDistance)
                    val scale = 1.12f - (0.32f * fraction)
                    val alpha = 1f - (0.55f * fraction)
                    val isCenter = fraction < 0.35f

                    Text(
                        text = optionLabel(option),
                        style = if (isCenter) {
                            MaterialTheme.typography.titleLarge
                        } else {
                            MaterialTheme.typography.bodyLarge
                        },
                        fontWeight = if (isCenter) FontWeight.SemiBold else FontWeight.Normal,
                        color = if (isCenter) ForestDeep else MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(itemHeight)
                            .graphicsLayer {
                                scaleX = scale
                                scaleY = scale
                                this.alpha = alpha
                            }
                            .clickable {
                                onSelected(option)
                                scope.launch { listState.animateScrollToItem(index) }
                            }
                            .padding(horizontal = 16.dp, vertical = 10.dp)
                    )
                }
            }
        }
    }
}

@Composable
fun FormSheet(
    title: String,
    onDismiss: () -> Unit,
    onSave: () -> Unit,
    saveEnabled: Boolean = true,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(title, style = MaterialTheme.typography.headlineMedium)
        content()
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End,
            verticalAlignment = Alignment.CenterVertically
        ) {
            TextButton(onClick = onDismiss) { Text("Cancel") }
            Button(onClick = onSave, enabled = saveEnabled) { Text("Save") }
        }
    }
}

@Composable
fun MoneyField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    OutlinedTextField(
        value = value,
        onValueChange = { input ->
            if (input.isEmpty() || input.matches(Regex("^\\d*\\.?\\d{0,2}$"))) {
                onValueChange(input)
            }
        },
        label = { Text(label) },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
        singleLine = true,
        modifier = modifier.fillMaxWidth()
    )
}
