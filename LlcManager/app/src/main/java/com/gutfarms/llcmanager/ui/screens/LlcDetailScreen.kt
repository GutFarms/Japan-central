package com.gutfarms.llcmanager.ui.screens

import android.content.Intent
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Remove
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material.icons.outlined.SwapHoriz
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.DeductionCategories
import com.gutfarms.llcmanager.data.model.Income
import com.gutfarms.llcmanager.data.model.IncomeCategories
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.data.model.Llc
import com.gutfarms.llcmanager.ui.components.AddFab
import com.gutfarms.llcmanager.ui.components.AtmosphereBackground
import com.gutfarms.llcmanager.ui.components.CategoryChipRow
import com.gutfarms.llcmanager.ui.components.ConfirmDialog
import com.gutfarms.llcmanager.ui.components.DateField
import com.gutfarms.llcmanager.ui.components.EmptyHint
import com.gutfarms.llcmanager.ui.components.FormField
import com.gutfarms.llcmanager.ui.components.MetricChip
import com.gutfarms.llcmanager.ui.components.SimpleFormDialog
import com.gutfarms.llcmanager.ui.components.formatMoney
import com.gutfarms.llcmanager.ui.components.formatQty
import com.gutfarms.llcmanager.ui.theme.DeepTeal
import com.gutfarms.llcmanager.ui.theme.SoftCoral
import com.gutfarms.llcmanager.ui.viewmodel.LlcViewModel
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

private enum class DetailTab { Income, Deductions, Inventory }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LlcDetailScreen(
    llcId: Long,
    viewModel: LlcViewModel,
    onBack: () -> Unit
) {
    LaunchedEffect(llcId) { viewModel.selectLlc(llcId) }

    val context = LocalContext.current
    val llc by viewModel.selectedLlc.collectAsStateWithLifecycle()
    val deductions by viewModel.deductions.collectAsStateWithLifecycle()
    val incomes by viewModel.incomes.collectAsStateWithLifecycle()
    val inventory by viewModel.inventory.collectAsStateWithLifecycle()
    val year by viewModel.yearFilter.collectAsState()
    val years by viewModel.availableYears.collectAsStateWithLifecycle()
    val allLlcs by viewModel.llcsForMove.collectAsStateWithLifecycle()

    var tab by remember { mutableStateOf(DetailTab.Income) }
    var showEditLlc by remember { mutableStateOf(false) }
    var editingDeduction by remember { mutableStateOf<Deduction?>(null) }
    var showAddDeduction by remember { mutableStateOf(false) }
    var editingIncome by remember { mutableStateOf<Income?>(null) }
    var showAddIncome by remember { mutableStateOf(false) }
    var editingInventory by remember { mutableStateOf<InventoryItem?>(null) }
    var showAddInventory by remember { mutableStateOf(false) }
    var pendingDeductionDelete by remember { mutableStateOf<Deduction?>(null) }
    var pendingIncomeDelete by remember { mutableStateOf<Income?>(null) }
    var pendingInventoryDelete by remember { mutableStateOf<InventoryItem?>(null) }
    var movingItem by remember { mutableStateOf<InventoryItem?>(null) }

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
                        IconButton(
                            onClick = {
                                val entity = current ?: return@IconButton
                                val text = viewModel.buildShareText(
                                    entity, deductions, incomes, inventory
                                )
                                val send = Intent(Intent.ACTION_SEND).apply {
                                    type = "text/plain"
                                    putExtra(Intent.EXTRA_SUBJECT, "${entity.name} report")
                                    putExtra(Intent.EXTRA_TEXT, text)
                                }
                                context.startActivity(Intent.createChooser(send, "Share report"))
                            }
                        ) {
                            Icon(Icons.Outlined.Share, contentDescription = "Share report")
                        }
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
                        when (tab) {
                            DetailTab.Income -> showAddIncome = true
                            DetailTab.Deductions -> showAddDeduction = true
                            DetailTab.Inventory -> showAddInventory = true
                        }
                    },
                    contentDescription = "Add"
                )
            }
        ) { padding ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(horizontal = 20.dp)
            ) {
                val incomeTotal = incomes.sumOf { it.amount }
                val deductionTotal = deductions.sumOf { it.amount }
                Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                    MetricChip("Net $year", formatMoney(incomeTotal - deductionTotal))
                    MetricChip("Income", formatMoney(incomeTotal))
                    MetricChip("Deductions", formatMoney(deductionTotal))
                }
                Spacer(Modifier.height(10.dp))
                Row(
                    modifier = Modifier.horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    years.forEach { y ->
                        FilterChip(
                            selected = year == y,
                            onClick = { viewModel.setYearFilter(y) },
                            label = { Text(y.toString()) }
                        )
                    }
                }
                Spacer(Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = tab == DetailTab.Income,
                        onClick = { tab = DetailTab.Income },
                        label = { Text("Income") }
                    )
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
                        DetailTab.Income -> IncomeList(
                            incomes = incomes,
                            onEdit = { editingIncome = it },
                            onRemove = { pendingIncomeDelete = it }
                        )
                        DetailTab.Deductions -> DeductionList(
                            deductions = deductions,
                            onEdit = { editingDeduction = it },
                            onRemove = { pendingDeductionDelete = it }
                        )
                        DetailTab.Inventory -> InventoryList(
                            items = inventory,
                            onAdjust = { item, delta -> viewModel.adjustInventory(item, delta) },
                            onEdit = { editingInventory = it },
                            onMove = { movingItem = it },
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

    if (showAddDeduction || editingDeduction != null) {
        val existing = editingDeduction
        DeductionEditorDialog(
            title = if (existing == null) "Add deduction" else "Edit deduction",
            confirmLabel = if (existing == null) "Add" else "Save",
            initial = existing,
            onDismiss = {
                showAddDeduction = false
                editingDeduction = null
            },
            onSave = { name, category, vendor, amount, date, notes ->
                viewModel.saveDeduction(
                    id = existing?.id ?: 0L,
                    llcId = llcId,
                    name = name,
                    category = category,
                    vendor = vendor,
                    amount = amount,
                    dateEpochMs = date,
                    notes = notes
                ) {
                    showAddDeduction = false
                    editingDeduction = null
                }
            }
        )
    }

    if (showAddIncome || editingIncome != null) {
        val existing = editingIncome
        IncomeEditorDialog(
            title = if (existing == null) "Add income" else "Edit income",
            confirmLabel = if (existing == null) "Add" else "Save",
            initial = existing,
            onDismiss = {
                showAddIncome = false
                editingIncome = null
            },
            onSave = { name, category, source, amount, date, notes ->
                viewModel.saveIncome(
                    id = existing?.id ?: 0L,
                    llcId = llcId,
                    name = name,
                    category = category,
                    source = source,
                    amount = amount,
                    dateEpochMs = date,
                    notes = notes
                ) {
                    showAddIncome = false
                    editingIncome = null
                }
            }
        )
    }

    if (showAddInventory || editingInventory != null) {
        val existing = editingInventory
        InventoryEditorDialog(
            title = if (existing == null) "Add inventory item" else "Edit inventory item",
            confirmLabel = if (existing == null) "Add" else "Save",
            initial = existing,
            onDismiss = {
                showAddInventory = false
                editingInventory = null
            },
            onSave = { name, sku, qty, unit, cost, reorder, location, notes ->
                viewModel.saveInventoryItem(
                    id = existing?.id ?: 0L,
                    llcId = llcId,
                    name = name,
                    sku = sku,
                    quantity = qty,
                    unit = unit,
                    unitCost = cost,
                    reorderLevel = reorder,
                    location = location,
                    notes = notes
                ) {
                    showAddInventory = false
                    editingInventory = null
                }
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

    pendingIncomeDelete?.let { item ->
        ConfirmDialog(
            title = "Remove income?",
            message = "Remove \"${item.name}\" (${formatMoney(item.amount)})?",
            onConfirm = {
                viewModel.removeIncome(item.id)
                pendingIncomeDelete = null
            },
            onDismiss = { pendingIncomeDelete = null }
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

    movingItem?.let { item ->
        MoveInventoryDialog(
            item = item,
            llcs = allLlcs.filter { it.id != llcId },
            onDismiss = { movingItem = null },
            onMove = { target ->
                viewModel.moveInventory(item, target.id) { movingItem = null }
            }
        )
    }
}

@Composable
private fun IncomeList(
    incomes: List<Income>,
    onEdit: (Income) -> Unit,
    onRemove: (Income) -> Unit
) {
    if (incomes.isEmpty()) {
        EmptyHint("No income for this year. Tap + to add.")
        return
    }
    val dateFmt = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.fillMaxSize()
    ) {
        items(incomes, key = { it.id }) { income ->
            Surface(
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.78f),
                shape = RoundedCornerShape(6.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onEdit(income) }
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(income.name, style = MaterialTheme.typography.titleMedium)
                        Text(
                            listOfNotNull(
                                income.category,
                                income.source.takeIf { it.isNotBlank() },
                                dateFmt.format(Date(income.dateEpochMs))
                            ).joinToString(" · "),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Text(
                        formatMoney(income.amount),
                        style = MaterialTheme.typography.titleMedium,
                        color = DeepTeal,
                        modifier = Modifier.padding(end = 4.dp)
                    )
                    IconButton(onClick = { onRemove(income) }) {
                        Icon(Icons.Outlined.Delete, contentDescription = "Remove income")
                    }
                }
            }
        }
        item { Spacer(Modifier.height(72.dp)) }
    }
}

@Composable
private fun DeductionList(
    deductions: List<Deduction>,
    onEdit: (Deduction) -> Unit,
    onRemove: (Deduction) -> Unit
) {
    if (deductions.isEmpty()) {
        EmptyHint("No deductions for this year. Tap + to add one.")
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
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onEdit(deduction) }
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(deduction.name, style = MaterialTheme.typography.titleMedium)
                        Text(
                            listOfNotNull(
                                deduction.category,
                                deduction.vendor.takeIf { it.isNotBlank() },
                                dateFmt.format(Date(deduction.dateEpochMs))
                            ).joinToString(" · "),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
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
    onEdit: (InventoryItem) -> Unit,
    onMove: (InventoryItem) -> Unit,
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
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onEdit(item) }
            ) {
                Column(modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(modifier = Modifier.weight(1f)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(item.name, style = MaterialTheme.typography.titleMedium)
                                if (item.isLowStock) {
                                    Icon(
                                        Icons.Outlined.WarningAmber,
                                        contentDescription = "Low stock",
                                        tint = SoftCoral,
                                        modifier = Modifier.padding(start = 6.dp)
                                    )
                                }
                            }
                            val meta = listOfNotNull(
                                item.sku.takeIf { it.isNotBlank() }?.let { "SKU $it" },
                                item.location.takeIf { it.isNotBlank() },
                                if (item.reorderLevel > 0) "Reorder ≤ ${formatQty(item.reorderLevel)}" else null
                            ).joinToString(" · ")
                            if (meta.isNotBlank()) {
                                Text(
                                    meta,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                        IconButton(onClick = { onMove(item) }) {
                            Icon(Icons.Outlined.SwapHoriz, contentDescription = "Move to another LLC")
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
                            color = if (item.isLowStock) SoftCoral else DeepTeal
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
    title: String,
    confirmLabel: String,
    initial: Deduction?,
    onDismiss: () -> Unit,
    onSave: (
        name: String,
        category: String,
        vendor: String,
        amount: Double,
        dateEpochMs: Long,
        notes: String
    ) -> Unit
) {
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var category by remember { mutableStateOf(initial?.category ?: "General") }
    var vendor by remember { mutableStateOf(initial?.vendor.orEmpty()) }
    var amountText by remember {
        mutableStateOf(initial?.amount?.let { if (it == 0.0) "" else it.toString() }.orEmpty())
    }
    var dateMs by remember { mutableStateOf(initial?.dateEpochMs ?: System.currentTimeMillis()) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }
    val amount = amountText.toDoubleOrNull()

    SimpleFormDialog(
        title = title,
        confirmLabel = confirmLabel,
        confirmEnabled = name.isNotBlank() && amount != null && amount >= 0,
        onDismiss = onDismiss,
        onConfirm = { onSave(name, category, vendor, amount ?: 0.0, dateMs, notes) }
    ) {
        FormField(name, { name = it }, "Description")
        CategoryChipRow(
            categories = DeductionCategories.defaults,
            selected = category,
            onSelect = { category = it }
        )
        FormField(category, { category = it }, "Category")
        FormField(vendor, { vendor = it }, "Vendor (optional)")
        FormField(
            amountText,
            { amountText = it.filter { ch -> ch.isDigit() || ch == '.' } },
            "Amount",
            keyboardType = KeyboardType.Decimal
        )
        DateField(dateMs, onDateChange = { dateMs = it })
        FormField(notes, { notes = it }, "Notes", singleLine = false)
    }
}

@Composable
private fun IncomeEditorDialog(
    title: String,
    confirmLabel: String,
    initial: Income?,
    onDismiss: () -> Unit,
    onSave: (
        name: String,
        category: String,
        source: String,
        amount: Double,
        dateEpochMs: Long,
        notes: String
    ) -> Unit
) {
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var category by remember { mutableStateOf(initial?.category ?: "Sales") }
    var source by remember { mutableStateOf(initial?.source.orEmpty()) }
    var amountText by remember {
        mutableStateOf(initial?.amount?.let { if (it == 0.0) "" else it.toString() }.orEmpty())
    }
    var dateMs by remember { mutableStateOf(initial?.dateEpochMs ?: System.currentTimeMillis()) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }
    val amount = amountText.toDoubleOrNull()

    SimpleFormDialog(
        title = title,
        confirmLabel = confirmLabel,
        confirmEnabled = name.isNotBlank() && amount != null && amount >= 0,
        onDismiss = onDismiss,
        onConfirm = { onSave(name, category, source, amount ?: 0.0, dateMs, notes) }
    ) {
        FormField(name, { name = it }, "Description")
        CategoryChipRow(
            categories = IncomeCategories.defaults,
            selected = category,
            onSelect = { category = it }
        )
        FormField(category, { category = it }, "Category")
        FormField(source, { source = it }, "Source / customer (optional)")
        FormField(
            amountText,
            { amountText = it.filter { ch -> ch.isDigit() || ch == '.' } },
            "Amount",
            keyboardType = KeyboardType.Decimal
        )
        DateField(dateMs, onDateChange = { dateMs = it })
        FormField(notes, { notes = it }, "Notes", singleLine = false)
    }
}

@Composable
private fun InventoryEditorDialog(
    title: String,
    confirmLabel: String,
    initial: InventoryItem?,
    onDismiss: () -> Unit,
    onSave: (
        name: String,
        sku: String,
        qty: Double,
        unit: String,
        cost: Double,
        reorder: Double,
        location: String,
        notes: String
    ) -> Unit
) {
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var sku by remember { mutableStateOf(initial?.sku.orEmpty()) }
    var qtyText by remember {
        mutableStateOf(initial?.quantity?.let { formatQty(it) } ?: "1")
    }
    var unit by remember { mutableStateOf(initial?.unit ?: "ea") }
    var costText by remember {
        mutableStateOf(initial?.unitCost?.takeIf { it > 0 }?.toString().orEmpty())
    }
    var reorderText by remember {
        mutableStateOf(initial?.reorderLevel?.takeIf { it > 0 }?.let { formatQty(it) }.orEmpty())
    }
    var location by remember { mutableStateOf(initial?.location.orEmpty()) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }
    val qty = qtyText.toDoubleOrNull()
    val cost = costText.toDoubleOrNull() ?: 0.0
    val reorder = reorderText.toDoubleOrNull() ?: 0.0

    SimpleFormDialog(
        title = title,
        confirmLabel = confirmLabel,
        confirmEnabled = name.isNotBlank() && qty != null && qty >= 0,
        onDismiss = onDismiss,
        onConfirm = {
            onSave(name, sku, qty ?: 0.0, unit, cost, reorder, location, notes)
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
        FormField(
            reorderText,
            { reorderText = it.filter { ch -> ch.isDigit() || ch == '.' } },
            "Reorder level (optional)",
            keyboardType = KeyboardType.Decimal
        )
        FormField(location, { location = it }, "Location (optional)")
        FormField(notes, { notes = it }, "Notes", singleLine = false)
    }
}

@Composable
private fun MoveInventoryDialog(
    item: InventoryItem,
    llcs: List<Llc>,
    onDismiss: () -> Unit,
    onMove: (Llc) -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Move \"${item.name}\"") },
        text = {
            if (llcs.isEmpty()) {
                Text("Add another LLC first to move inventory between entities.")
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("Choose destination LLC:")
                    llcs.forEach { dest ->
                        TextButton(onClick = { onMove(dest) }) {
                            Text(dest.name, modifier = Modifier.fillMaxWidth())
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}
