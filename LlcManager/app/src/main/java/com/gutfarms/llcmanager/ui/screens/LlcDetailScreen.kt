package com.gutfarms.llcmanager.ui.screens

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Remove
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.ui.components.AddFab
import com.gutfarms.llcmanager.ui.components.AtmosphereBackground
import com.gutfarms.llcmanager.ui.components.ConfirmDialog
import com.gutfarms.llcmanager.ui.components.EmptyHint
import com.gutfarms.llcmanager.ui.components.FormField
import com.gutfarms.llcmanager.ui.components.MetricChip
import com.gutfarms.llcmanager.ui.components.SimpleFormDialog
import com.gutfarms.llcmanager.ui.components.formatMoney
import com.gutfarms.llcmanager.ui.components.formatQty
import com.gutfarms.llcmanager.ui.theme.DeepTeal
import com.gutfarms.llcmanager.ui.viewmodel.LlcViewModel
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

private enum class DetailTab { Deductions, Inventory }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LlcDetailScreen(
    llcId: Long,
    viewModel: LlcViewModel,
    onBack: () -> Unit
) {
    LaunchedEffect(llcId) { viewModel.selectLlc(llcId) }

    val llc by viewModel.selectedLlc.collectAsStateWithLifecycle()
    val deductions by viewModel.deductions.collectAsStateWithLifecycle()
    val inventory by viewModel.inventory.collectAsStateWithLifecycle()

    var tab by remember { mutableStateOf(DetailTab.Deductions) }
    var showEditLlc by remember { mutableStateOf(false) }
    var showAddDeduction by remember { mutableStateOf(false) }
    var showAddInventory by remember { mutableStateOf(false) }
    var pendingDeductionDelete by remember { mutableStateOf<Deduction?>(null) }
    var pendingInventoryDelete by remember { mutableStateOf<InventoryItem?>(null) }

    val current = llc
    AtmosphereBackground(modifier = Modifier.fillMaxSize()) {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                TopAppBar(
                    title = {
                        Column {
                            Text(current?.name ?: "LLC", style = MaterialTheme.typography.titleLarge)
                            val subtitle = listOfNotNull(
                                current?.state?.takeIf { it.isNotBlank() },
                                current?.ein?.takeIf { it.isNotBlank() }?.let { "EIN $it" }
                            ).joinToString(" · ")
                            if (subtitle.isNotBlank()) {
                                Text(
                                    subtitle,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
                        }
                    },
                    actions = {
                        IconButton(onClick = { showEditLlc = true }) {
                            Icon(Icons.Outlined.Edit, contentDescription = "Edit LLC")
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
                )
            },
            floatingActionButton = {
                AddFab(
                    onClick = {
                        if (tab == DetailTab.Deductions) showAddDeduction = true
                        else showAddInventory = true
                    },
                    contentDescription = if (tab == DetailTab.Deductions) "Add deduction" else "Add inventory item"
                )
            }
        ) { padding ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(horizontal = 20.dp)
            ) {
                Row(horizontalArrangement = Arrangement.spacedBy(28.dp)) {
                    MetricChip("Deductions", formatMoney(deductions.sumOf { it.amount }))
                    MetricChip("Inventory", formatMoney(inventory.sumOf { it.totalValue }))
                    MetricChip("SKUs", inventory.size.toString())
                }
                Spacer(Modifier.height(14.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = tab == DetailTab.Deductions,
                        onClick = { tab = DetailTab.Deductions },
                        label = { Text("Deductions") }
                    )
                    FilterChip(
                        selected = tab == DetailTab.Inventory,
                        onClick = { tab = DetailTab.Inventory },
                        label = { Text("Inventory") }
                    )
                }
                Spacer(Modifier.height(10.dp))

                AnimatedContent(
                    targetState = tab,
                    transitionSpec = { fadeIn() togetherWith fadeOut() },
                    label = "detail-tab"
                ) { currentTab ->
                    when (currentTab) {
                        DetailTab.Deductions -> DeductionList(
                            deductions = deductions,
                            onRemove = { pendingDeductionDelete = it }
                        )
                        DetailTab.Inventory -> InventoryList(
                            items = inventory,
                            onAdjust = { item, delta -> viewModel.adjustInventory(item, delta) },
                            onRemove = { pendingInventoryDelete = it }
                        )
                    }
                }
            }
        }
    }

    if (showEditLlc && current != null) {
        LlcEditorDialog(
            title = "Edit LLC",
            initialName = current.name,
            initialEin = current.ein,
            initialState = current.state,
            initialNotes = current.notes,
            onDismiss = { showEditLlc = false },
            onSave = { name, ein, state, notes ->
                viewModel.saveLlc(
                    id = current.id,
                    name = name,
                    ein = ein,
                    state = state,
                    notes = notes
                ) { showEditLlc = false }
            }
        )
    }

    if (showAddDeduction) {
        DeductionEditorDialog(
            onDismiss = { showAddDeduction = false },
            onSave = { name, category, amount, notes ->
                viewModel.addDeduction(llcId, name, category, amount, notes) {
                    showAddDeduction = false
                }
            }
        )
    }

    if (showAddInventory) {
        InventoryEditorDialog(
            onDismiss = { showAddInventory = false },
            onSave = { name, sku, qty, unit, cost, location, notes ->
                viewModel.saveInventoryItem(
                    llcId = llcId,
                    name = name,
                    sku = sku,
                    quantity = qty,
                    unit = unit,
                    unitCost = cost,
                    location = location,
                    notes = notes
                ) { showAddInventory = false }
            }
        )
    }

    pendingDeductionDelete?.let { item ->
        ConfirmDialog(
            title = "Remove deduction?",
            message = "Remove \"${item.name}\" (${formatMoney(item.amount)})?",
            onConfirm = {
                viewModel.removeDeduction(item.id)
                pendingDeductionDelete = null
            },
            onDismiss = { pendingDeductionDelete = null }
        )
    }

    pendingInventoryDelete?.let { item ->
        ConfirmDialog(
            title = "Remove inventory item?",
            message = "Remove \"${item.name}\" from inventory?",
            onConfirm = {
                viewModel.removeInventoryItem(item.id)
                pendingInventoryDelete = null
            },
            onDismiss = { pendingInventoryDelete = null }
        )
    }
}

@Composable
private fun DeductionList(
    deductions: List<Deduction>,
    onRemove: (Deduction) -> Unit
) {
    if (deductions.isEmpty()) {
        EmptyHint("No deductions yet. Tap + to add one.")
        return
    }
    val dateFmt = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.fillMaxSize()
    ) {
        items(deductions, key = { it.id }) { deduction ->
            Surface(
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.78f),
                shape = RoundedCornerShape(6.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(deduction.name, style = MaterialTheme.typography.titleMedium)
                        Text(
                            "${deduction.category} · ${dateFmt.format(Date(deduction.dateEpochMs))}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        if (deduction.notes.isNotBlank()) {
                            Text(
                                deduction.notes,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                    Text(
                        formatMoney(deduction.amount),
                        style = MaterialTheme.typography.titleMedium,
                        color = DeepTeal,
                        modifier = Modifier.padding(end = 4.dp)
                    )
                    IconButton(onClick = { onRemove(deduction) }) {
                        Icon(Icons.Outlined.Delete, contentDescription = "Remove deduction")
                    }
                }
            }
        }
        item { Spacer(Modifier.height(72.dp)) }
    }
}

@Composable
private fun InventoryList(
    items: List<InventoryItem>,
    onAdjust: (InventoryItem, Double) -> Unit,
    onRemove: (InventoryItem) -> Unit
) {
    if (items.isEmpty()) {
        EmptyHint("Inventory is empty. Tap + to add an item.")
        return
    }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.fillMaxSize()
    ) {
        items(items, key = { it.id }) { item ->
            Surface(
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.78f),
                shape = RoundedCornerShape(6.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(item.name, style = MaterialTheme.typography.titleMedium)
                            val meta = listOfNotNull(
                                item.sku.takeIf { it.isNotBlank() }?.let { "SKU $it" },
                                item.location.takeIf { it.isNotBlank() }
                            ).joinToString(" · ")
                            if (meta.isNotBlank()) {
                                Text(
                                    meta,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                        IconButton(onClick = { onRemove(item) }) {
                            Icon(Icons.Outlined.Delete, contentDescription = "Remove item")
                        }
                    }
                    Spacer(Modifier.height(6.dp))
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        IconButton(onClick = { onAdjust(item, -1.0) }) {
                            Icon(Icons.Outlined.Remove, contentDescription = "Decrease quantity")
                        }
                        Text(
                            "${formatQty(item.quantity)} ${item.unit}",
                            style = MaterialTheme.typography.titleMedium,
                            color = DeepTeal
                        )
                        IconButton(onClick = { onAdjust(item, 1.0) }) {
                            Icon(Icons.Outlined.Add, contentDescription = "Increase quantity")
                        }
                        Spacer(Modifier.weight(1f))
                        Text(
                            formatMoney(item.totalValue),
                            style = MaterialTheme.typography.titleMedium
                        )
                    }
                }
            }
        }
        item { Spacer(Modifier.height(72.dp)) }
    }
}

@Composable
private fun DeductionEditorDialog(
    onDismiss: () -> Unit,
    onSave: (name: String, category: String, amount: Double, notes: String) -> Unit
) {
    var name by remember { mutableStateOf("") }
    var category by remember { mutableStateOf("General") }
    var amountText by remember { mutableStateOf("") }
    var notes by remember { mutableStateOf("") }
    val amount = amountText.toDoubleOrNull()

    SimpleFormDialog(
        title = "Add deduction",
        confirmLabel = "Add",
        confirmEnabled = name.isNotBlank() && amount != null && amount >= 0,
        onDismiss = onDismiss,
        onConfirm = { onSave(name, category, amount ?: 0.0, notes) }
    ) {
        FormField(name, { name = it }, "Description")
        FormField(category, { category = it }, "Category")
        FormField(
            amountText,
            { amountText = it.filter { ch -> ch.isDigit() || ch == '.' } },
            "Amount",
            keyboardType = KeyboardType.Decimal
        )
        FormField(notes, { notes = it }, "Notes", singleLine = false)
    }
}

@Composable
private fun InventoryEditorDialog(
    onDismiss: () -> Unit,
    onSave: (
        name: String,
        sku: String,
        qty: Double,
        unit: String,
        cost: Double,
        location: String,
        notes: String
    ) -> Unit
) {
    var name by remember { mutableStateOf("") }
    var sku by remember { mutableStateOf("") }
    var qtyText by remember { mutableStateOf("1") }
    var unit by remember { mutableStateOf("ea") }
    var costText by remember { mutableStateOf("") }
    var location by remember { mutableStateOf("") }
    var notes by remember { mutableStateOf("") }
    val qty = qtyText.toDoubleOrNull()
    val cost = costText.toDoubleOrNull() ?: 0.0

    SimpleFormDialog(
        title = "Add inventory item",
        confirmLabel = "Add",
        confirmEnabled = name.isNotBlank() && qty != null && qty >= 0,
        onDismiss = onDismiss,
        onConfirm = {
            onSave(name, sku, qty ?: 0.0, unit, cost, location, notes)
        }
    ) {
        FormField(name, { name = it }, "Item name")
        FormField(sku, { sku = it }, "SKU (optional)")
        FormField(
            qtyText,
            { qtyText = it.filter { ch -> ch.isDigit() || ch == '.' } },
            "Quantity",
            keyboardType = KeyboardType.Decimal
        )
        FormField(unit, { unit = it }, "Unit (ea, box, lb…)")
        FormField(
            costText,
            { costText = it.filter { ch -> ch.isDigit() || ch == '.' } },
            "Unit cost",
            keyboardType = KeyboardType.Decimal
        )
        FormField(location, { location = it }, "Location (optional)")
        FormField(notes, { notes = it }, "Notes", singleLine = false)
    }
}
