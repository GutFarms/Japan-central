package com.gutfarms.llcmanager.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import android.app.DatePickerDialog
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.FilterChip
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.gutfarms.llcmanager.ui.theme.DeepTeal
import com.gutfarms.llcmanager.ui.theme.Paper
import com.gutfarms.llcmanager.ui.theme.Sand
import com.gutfarms.llcmanager.ui.theme.Seafoam
import java.text.NumberFormat
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Currency
import java.util.Date
import java.util.Locale

private val moneyFormat: NumberFormat = NumberFormat.getCurrencyInstance(Locale.US).apply {
    currency = Currency.getInstance("USD")
}

fun formatMoney(value: Double): String = moneyFormat.format(value)

fun formatQty(value: Double): String {
    return if (value % 1.0 == 0.0) value.toInt().toString() else String.format(Locale.US, "%.2f", value)
}

@Composable
fun AtmosphereBackground(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Box(
        modifier = modifier.background(
            Brush.verticalGradient(
                colors = listOf(
                    Sand.copy(alpha = 0.95f),
                    Paper,
                    Seafoam.copy(alpha = 0.18f)
                )
            )
        )
    ) {
        content()
    }
}

@Composable
fun SectionHeader(
    title: String,
    subtitle: String? = null,
    modifier: Modifier = Modifier,
    action: @Composable (() -> Unit)? = null
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.headlineMedium, color = DeepTeal)
            if (!subtitle.isNullOrBlank()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
        if (action != null) action()
    }
}

@Composable
fun EmptyHint(message: String, modifier: Modifier = Modifier) {
    AnimatedVisibility(
        visible = true,
        enter = fadeIn() + slideInVertically { it / 4 },
        exit = fadeOut(),
        modifier = modifier
    ) {
        Surface(
            color = MaterialTheme.colorScheme.surface.copy(alpha = 0.55f),
            shape = RoundedCornerShape(4.dp),
            modifier = Modifier.fillMaxWidth()
        ) {
            Text(
                text = message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 20.dp)
            )
        }
    }
}

@Composable
fun FormField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    singleLine: Boolean = true,
    keyboardType: KeyboardType = KeyboardType.Text,
    trailing: @Composable (() -> Unit)? = null
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        singleLine = singleLine,
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
        trailingIcon = trailing,
        modifier = modifier.fillMaxWidth()
    )
}

@Composable
fun ConfirmDialog(
    title: String,
    message: String,
    confirmLabel: String = "Remove",
    onConfirm: () -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(message) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(confirmLabel, color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SimpleFormDialog(
    title: String,
    confirmLabel: String = "Save",
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
    confirmEnabled: Boolean = true,
    content: @Composable ColumnScope.() -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(title, style = MaterialTheme.typography.titleLarge)
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Outlined.Close, contentDescription = "Close")
                }
            }
        },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(10.dp),
                content = content
            )
        },
        confirmButton = {
            Button(onClick = onConfirm, enabled = confirmEnabled) {
                Text(confirmLabel)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}

@Composable
fun AddFab(onClick: () -> Unit, contentDescription: String = "Add") {
    FloatingActionButton(onClick = onClick) {
        Icon(Icons.Outlined.Add, contentDescription = contentDescription)
    }
}

@Composable
fun MetricChip(label: String, value: String, modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        Text(
            label.uppercase(Locale.US),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(2.dp))
        Text(
            value,
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.SemiBold,
            color = DeepTeal
        )
    }
}

@Composable
fun ScreenPadding(
    padding: PaddingValues,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier = Modifier
            .padding(padding)
            .padding(horizontal = 20.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        content = content
    )
}

@Composable
fun TinySpacer(width: Int = 8) {
    Spacer(modifier = Modifier.width(width.dp).size(width.dp))
}

@Composable
fun DateField(
    epochMs: Long,
    onDateChange: (Long) -> Unit,
    label: String = "Date",
    modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    val fmt = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }
    val display = fmt.format(Date(epochMs))
    fun openPicker() {
        val cal = Calendar.getInstance().apply { timeInMillis = epochMs }
        DatePickerDialog(
            context,
            { _, y, m, d ->
                val picked = Calendar.getInstance().apply {
                    set(Calendar.YEAR, y)
                    set(Calendar.MONTH, m)
                    set(Calendar.DAY_OF_MONTH, d)
                    set(Calendar.HOUR_OF_DAY, 12)
                    set(Calendar.MINUTE, 0)
                    set(Calendar.SECOND, 0)
                    set(Calendar.MILLISECOND, 0)
                }
                onDateChange(picked.timeInMillis)
            },
            cal.get(Calendar.YEAR),
            cal.get(Calendar.MONTH),
            cal.get(Calendar.DAY_OF_MONTH)
        ).show()
    }
    OutlinedTextField(
        value = display,
        onValueChange = {},
        readOnly = true,
        label = { Text(label) },
        trailingIcon = {
            TextButton(onClick = { openPicker() }) { Text("Change") }
        },
        modifier = modifier.fillMaxWidth()
    )
}

@Composable
fun CategoryChipRow(
    categories: List<String>,
    selected: String,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        categories.forEach { cat ->
            FilterChip(
                selected = selected.equals(cat, ignoreCase = true),
                onClick = { onSelect(cat) },
                label = { Text(cat) }
            )
        }
    }
}
