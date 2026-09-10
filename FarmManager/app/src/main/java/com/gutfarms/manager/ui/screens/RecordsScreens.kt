package com.gutfarms.manager.ui.screens

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.ContactRole
import com.gutfarms.manager.data.model.FarmContact
import com.gutfarms.manager.data.model.FarmProfile
import com.gutfarms.manager.data.model.HealthRecord
import com.gutfarms.manager.data.model.HealthRecordType
import com.gutfarms.manager.data.model.InventoryCategory
import com.gutfarms.manager.data.model.InventoryItem
import com.gutfarms.manager.data.model.JournalCategory
import com.gutfarms.manager.data.model.JournalEntry
import com.gutfarms.manager.ui.components.EmptyHint
import com.gutfarms.manager.ui.components.FormSheet
import com.gutfarms.manager.ui.components.MoneyField
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.SimpleDropdown
import com.gutfarms.manager.ui.components.formatDate
import com.gutfarms.manager.ui.components.formatDateInput
import com.gutfarms.manager.ui.components.formatMoney
import com.gutfarms.manager.ui.components.parseDateInput
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Forest
import com.gutfarms.manager.ui.theme.Mist
import com.gutfarms.manager.ui.theme.SoftTeal
import kotlinx.coroutines.flow.StateFlow

@Composable
fun RecordsHubScreen(
    farmName: StateFlow<String>,
    healthCount: Int,
    inventoryCount: Int,
    journalCount: Int,
    contactCount: Int,
    lowStockCount: Int,
    onOpenFarmInfo: () -> Unit,
    onOpenHealth: () -> Unit,
    onOpenInventory: () -> Unit,
    onOpenJournal: () -> Unit,
    onOpenContacts: () -> Unit,
    onBack: () -> Unit
) {
    val brand by farmName.collectAsState()
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
            .verticalScroll(rememberScrollState())
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(start = 4.dp, top = 8.dp)
        ) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
            }
        }
        ScreenHeader(
            brand = brand,
            title = "Farm records",
            subtitle = "Gather health, inventory, journal, and contact details."
        )
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            RecordNavCard("Farm info", "Location, owner, phone, notes", onOpenFarmInfo)
            RecordNavCard("Health log", "$healthCount records", onOpenHealth)
            RecordNavCard(
                "Inventory",
                if (lowStockCount > 0) "$inventoryCount items · $lowStockCount low stock"
                else "$inventoryCount items",
                onOpenInventory
            )
            RecordNavCard("Farm journal", "$journalCount entries", onOpenJournal)
            RecordNavCard("Contacts", "$contactCount people & vendors", onOpenContacts)
        }
    }
}

@Composable
private fun RecordNavCard(title: String, detail: String, onClick: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surface)
            .clickable(onClick = onClick)
            .padding(16.dp)
    ) {
        Text(title, style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(4.dp))
        Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun FarmInfoScreen(
    farmName: StateFlow<String>,
    farmProfile: StateFlow<FarmProfile>,
    onSave: (FarmProfile) -> Unit,
    onBack: () -> Unit
) {
    val brand by farmName.collectAsState()
    val profile by farmProfile.collectAsState()
    var name by remember(profile) { mutableStateOf(profile.farmName) }
    var location by remember(profile) { mutableStateOf(profile.location) }
    var owner by remember(profile) { mutableStateOf(profile.ownerName) }
    var phone by remember(profile) { mutableStateOf(profile.phone) }
    var notes by remember(profile) { mutableStateOf(profile.notes) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
            .verticalScroll(rememberScrollState())
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, top = 8.dp)) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
            }
        }
        ScreenHeader(brand = brand, title = "Farm information", subtitle = "Core details for this operation.")
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedTextField(value = name, onValueChange = { name = it }, label = { Text("Farm name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = location, onValueChange = { location = it }, label = { Text("Location") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = owner, onValueChange = { owner = it }, label = { Text("Owner / manager") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = phone, onValueChange = { phone = it }, label = { Text("Phone") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = notes, onValueChange = { notes = it }, label = { Text("Notes") }, modifier = Modifier.fillMaxWidth())
            Button(
                onClick = {
                    onSave(
                        FarmProfile(
                            id = 1,
                            farmName = name.trim(),
                            location = location.trim(),
                            ownerName = owner.trim(),
                            phone = phone.trim(),
                            notes = notes.trim()
                        )
                    )
                    onBack()
                },
                enabled = name.isNotBlank(),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Save farm info")
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HealthScreen(
    farmName: StateFlow<String>,
    animals: StateFlow<List<Animal>>,
    records: StateFlow<List<HealthRecord>>,
    onSave: (HealthRecord) -> Unit,
    onDelete: (HealthRecord) -> Unit,
    onBack: () -> Unit
) {
    val brand by farmName.collectAsState()
    val animalList by animals.collectAsState()
    val recordList by records.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<HealthRecord?>(null) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = { editing = null; showSheet = true }, containerColor = SoftTeal) {
                Icon(Icons.Filled.Add, contentDescription = "Add health record")
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, top = 8.dp)) {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                }
            }
            ScreenHeader(brand = brand, title = "Health records", subtitle = "Vaccinations, treatments, and checkups.")
            LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (recordList.isEmpty()) item { EmptyHint("Log vaccines, illnesses, and vet visits.") }
                items(recordList, key = { it.id }) { record ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .animateContentSize()
                            .padding(16.dp)
                    ) {
                        Text(record.title, style = MaterialTheme.typography.titleLarge)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            "${prettyEnum(record.type.name)} · ${formatDate(record.dateMillis)}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        val who = record.animalLabel.ifBlank {
                            animalList.firstOrNull { it.id == record.animalId }?.name.orEmpty()
                        }
                        if (who.isNotBlank()) {
                            Text(who, style = MaterialTheme.typography.bodyMedium)
                        }
                        if (record.provider.isNotBlank() || record.cost > 0) {
                            Text(
                                listOfNotNull(
                                    record.provider.takeIf { it.isNotBlank() },
                                    record.cost.takeIf { it > 0 }?.let { formatMoney(it) }
                                ).joinToString(" · "),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.secondary
                            )
                        }
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                            TextButton(onClick = { editing = record; showSheet = true }) { Text("Edit") }
                            IconButton(onClick = { onDelete(record) }) {
                                Icon(Icons.Outlined.Delete, contentDescription = "Delete")
                            }
                        }
                    }
                }
            }
        }
    }

    if (showSheet) {
        ModalBottomSheet(onDismissRequest = { showSheet = false }, sheetState = sheetState) {
            HealthForm(
                animals = animalList,
                initial = editing,
                onDismiss = { showSheet = false },
                onSave = { onSave(it); showSheet = false }
            )
        }
    }
}

@Composable
private fun HealthForm(
    animals: List<Animal>,
    initial: HealthRecord?,
    onDismiss: () -> Unit,
    onSave: (HealthRecord) -> Unit
) {
    var type by remember { mutableStateOf(initial?.type ?: HealthRecordType.CHECKUP) }
    var title by remember { mutableStateOf(initial?.title.orEmpty()) }
    var animalId by remember { mutableStateOf(initial?.animalId) }
    var animalLabel by remember { mutableStateOf(initial?.animalLabel.orEmpty()) }
    var dateText by remember { mutableStateOf(formatDateInput(initial?.dateMillis ?: System.currentTimeMillis())) }
    var provider by remember { mutableStateOf(initial?.provider.orEmpty()) }
    var cost by remember { mutableStateOf(initial?.cost?.takeIf { it > 0 }?.toString().orEmpty()) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }
    val noneOption = Animal(id = -1, name = "No group", type = com.gutfarms.manager.data.model.AnimalType.OTHER, count = 0)

    FormSheet(
        title = if (initial == null) "Add health record" else "Edit health record",
        onDismiss = onDismiss,
        onSave = {
            onSave(
                HealthRecord(
                    id = initial?.id ?: 0,
                    animalId = animalId,
                    animalLabel = animalLabel.trim(),
                    type = type,
                    title = title.trim(),
                    dateMillis = parseDateInput(dateText) ?: System.currentTimeMillis(),
                    provider = provider.trim(),
                    cost = cost.toDoubleOrNull() ?: 0.0,
                    notes = notes.trim()
                )
            )
        },
        saveEnabled = title.isNotBlank()
    ) {
        SimpleDropdown(
            label = "Type",
            options = HealthRecordType.entries,
            selected = type,
            onSelected = { type = it },
            optionLabel = { prettyEnum(it.name) }
        )
        OutlinedTextField(value = title, onValueChange = { title = it }, label = { Text("Title") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        SimpleDropdown(
            label = "Livestock group (optional)",
            options = listOf(noneOption) + animals,
            selected = animals.firstOrNull { it.id == animalId } ?: noneOption,
            onSelected = { animalId = it.id.takeIf { id -> id > 0 } },
            optionLabel = { if (it.id < 0) "None" else "${it.name} (${it.type.name.lowercase()})" }
        )
        OutlinedTextField(value = animalLabel, onValueChange = { animalLabel = it }, label = { Text("Animal / tag label") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = dateText, onValueChange = { dateText = it }, label = { Text("Date (yyyy-MM-dd)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = provider, onValueChange = { provider = it }, label = { Text("Provider") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        MoneyField(label = "Cost", value = cost, onValueChange = { cost = it })
        OutlinedTextField(value = notes, onValueChange = { notes = it }, label = { Text("Notes") }, modifier = Modifier.fillMaxWidth())
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InventoryScreen(
    farmName: StateFlow<String>,
    items: StateFlow<List<InventoryItem>>,
    onSave: (InventoryItem) -> Unit,
    onDelete: (InventoryItem) -> Unit,
    onBack: () -> Unit
) {
    val brand by farmName.collectAsState()
    val itemList by items.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<InventoryItem?>(null) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = { editing = null; showSheet = true }, containerColor = Forest) {
                Icon(Icons.Filled.Add, contentDescription = "Add inventory item")
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, top = 8.dp)) {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                }
            }
            ScreenHeader(brand = brand, title = "Inventory", subtitle = "Stock levels for feed, meds, and supplies.")
            LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (itemList.isEmpty()) item { EmptyHint("Track feed bags, medicine, and equipment.") }
                items(itemList, key = { it.id }) { item ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .padding(16.dp)
                    ) {
                        Text(item.name, style = MaterialTheme.typography.titleLarge)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            "${prettyEnum(item.category.name)} · ${trimQty(item.quantity)} ${item.unit}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        if (item.needsReorder) {
                            Text("Reorder soon", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.labelLarge)
                        }
                        if (item.location.isNotBlank()) {
                            Text(item.location, style = MaterialTheme.typography.bodySmall)
                        }
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                            TextButton(onClick = { editing = item; showSheet = true }) { Text("Edit") }
                            IconButton(onClick = { onDelete(item) }) {
                                Icon(Icons.Outlined.Delete, contentDescription = "Delete")
                            }
                        }
                    }
                }
            }
        }
    }

    if (showSheet) {
        ModalBottomSheet(onDismissRequest = { showSheet = false }, sheetState = sheetState) {
            InventoryForm(initial = editing, onDismiss = { showSheet = false }, onSave = { onSave(it); showSheet = false })
        }
    }
}

@Composable
private fun InventoryForm(
    initial: InventoryItem?,
    onDismiss: () -> Unit,
    onSave: (InventoryItem) -> Unit
) {
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var category by remember { mutableStateOf(initial?.category ?: InventoryCategory.SUPPLIES) }
    var quantity by remember { mutableStateOf(initial?.quantity?.toString().orEmpty()) }
    var unit by remember { mutableStateOf(initial?.unit ?: "units") }
    var reorder by remember { mutableStateOf(initial?.reorderLevel?.takeIf { it > 0 }?.toString().orEmpty()) }
    var unitCost by remember { mutableStateOf(initial?.unitCost?.takeIf { it > 0 }?.toString().orEmpty()) }
    var location by remember { mutableStateOf(initial?.location.orEmpty()) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }

    FormSheet(
        title = if (initial == null) "Add inventory" else "Edit inventory",
        onDismiss = onDismiss,
        onSave = {
            onSave(
                InventoryItem(
                    id = initial?.id ?: 0,
                    name = name.trim(),
                    category = category,
                    quantity = quantity.toDoubleOrNull() ?: 0.0,
                    unit = unit.trim().ifBlank { "units" },
                    reorderLevel = reorder.toDoubleOrNull() ?: 0.0,
                    unitCost = unitCost.toDoubleOrNull() ?: 0.0,
                    location = location.trim(),
                    notes = notes.trim()
                )
            )
        },
        saveEnabled = name.isNotBlank()
    ) {
        OutlinedTextField(value = name, onValueChange = { name = it }, label = { Text("Item name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        SimpleDropdown(
            label = "Category",
            options = InventoryCategory.entries,
            selected = category,
            onSelected = { category = it },
            optionLabel = { prettyEnum(it.name) }
        )
        OutlinedTextField(
            value = quantity,
            onValueChange = { if (it.isEmpty() || it.matches(Regex("^\\d*\\.?\\d{0,2}$"))) quantity = it },
            label = { Text("Quantity") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(value = unit, onValueChange = { unit = it }, label = { Text("Unit") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(
            value = reorder,
            onValueChange = { if (it.isEmpty() || it.matches(Regex("^\\d*\\.?\\d{0,2}$"))) reorder = it },
            label = { Text("Reorder level") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        MoneyField(label = "Unit cost", value = unitCost, onValueChange = { unitCost = it })
        OutlinedTextField(value = location, onValueChange = { location = it }, label = { Text("Location") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = notes, onValueChange = { notes = it }, label = { Text("Notes") }, modifier = Modifier.fillMaxWidth())
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun JournalScreen(
    farmName: StateFlow<String>,
    entries: StateFlow<List<JournalEntry>>,
    onSave: (JournalEntry) -> Unit,
    onDelete: (JournalEntry) -> Unit,
    onBack: () -> Unit
) {
    val brand by farmName.collectAsState()
    val entryList by entries.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<JournalEntry?>(null) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = { editing = null; showSheet = true }, containerColor = SoftTeal) {
                Icon(Icons.Filled.Add, contentDescription = "Add journal entry")
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, top = 8.dp)) {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                }
            }
            ScreenHeader(brand = brand, title = "Farm journal", subtitle = "Daily logs, weather, pasture notes, and tasks.")
            LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (entryList.isEmpty()) item { EmptyHint("Capture observations and day-to-day farm notes.") }
                items(entryList, key = { it.id }) { entry ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .padding(16.dp)
                    ) {
                        Text(entry.title, style = MaterialTheme.typography.titleLarge)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            "${prettyEnum(entry.category.name)} · ${formatDate(entry.dateMillis)}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        if (entry.body.isNotBlank()) {
                            Text(entry.body, style = MaterialTheme.typography.bodyMedium)
                        }
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                            TextButton(onClick = { editing = entry; showSheet = true }) { Text("Edit") }
                            IconButton(onClick = { onDelete(entry) }) {
                                Icon(Icons.Outlined.Delete, contentDescription = "Delete")
                            }
                        }
                    }
                }
            }
        }
    }

    if (showSheet) {
        ModalBottomSheet(onDismissRequest = { showSheet = false }, sheetState = sheetState) {
            JournalForm(initial = editing, onDismiss = { showSheet = false }, onSave = { onSave(it); showSheet = false })
        }
    }
}

@Composable
private fun JournalForm(
    initial: JournalEntry?,
    onDismiss: () -> Unit,
    onSave: (JournalEntry) -> Unit
) {
    var title by remember { mutableStateOf(initial?.title.orEmpty()) }
    var category by remember { mutableStateOf(initial?.category ?: JournalCategory.DAILY_LOG) }
    var body by remember { mutableStateOf(initial?.body.orEmpty()) }
    var dateText by remember { mutableStateOf(formatDateInput(initial?.dateMillis ?: System.currentTimeMillis())) }
    var tags by remember { mutableStateOf(initial?.tags.orEmpty()) }

    FormSheet(
        title = if (initial == null) "Add journal entry" else "Edit journal entry",
        onDismiss = onDismiss,
        onSave = {
            onSave(
                JournalEntry(
                    id = initial?.id ?: 0,
                    title = title.trim(),
                    category = category,
                    body = body.trim(),
                    dateMillis = parseDateInput(dateText) ?: System.currentTimeMillis(),
                    tags = tags.trim()
                )
            )
        },
        saveEnabled = title.isNotBlank()
    ) {
        OutlinedTextField(value = title, onValueChange = { title = it }, label = { Text("Title") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        SimpleDropdown(
            label = "Category",
            options = JournalCategory.entries,
            selected = category,
            onSelected = { category = it },
            optionLabel = { prettyEnum(it.name) }
        )
        OutlinedTextField(value = dateText, onValueChange = { dateText = it }, label = { Text("Date (yyyy-MM-dd)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = body, onValueChange = { body = it }, label = { Text("Notes") }, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = tags, onValueChange = { tags = it }, label = { Text("Tags") }, singleLine = true, modifier = Modifier.fillMaxWidth())
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ContactsScreen(
    farmName: StateFlow<String>,
    contacts: StateFlow<List<FarmContact>>,
    onSave: (FarmContact) -> Unit,
    onDelete: (FarmContact) -> Unit,
    onBack: () -> Unit
) {
    val brand by farmName.collectAsState()
    val contactList by contacts.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<FarmContact?>(null) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = { editing = null; showSheet = true }, containerColor = Forest) {
                Icon(Icons.Filled.Add, contentDescription = "Add contact")
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, top = 8.dp)) {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                }
            }
            ScreenHeader(brand = brand, title = "Contacts", subtitle = "Vets, suppliers, buyers, and workers.")
            LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (contactList.isEmpty()) item { EmptyHint("Save people and vendors you work with.") }
                items(contactList, key = { it.id }) { contact ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .padding(16.dp)
                    ) {
                        Text(contact.name, style = MaterialTheme.typography.titleLarge)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            prettyEnum(contact.role.name) +
                                (contact.organization.takeIf { it.isNotBlank() }?.let { " · $it" } ?: ""),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        if (contact.phone.isNotBlank() || contact.email.isNotBlank()) {
                            Text(
                                listOfNotNull(
                                    contact.phone.takeIf { it.isNotBlank() },
                                    contact.email.takeIf { it.isNotBlank() }
                                ).joinToString(" · "),
                                style = MaterialTheme.typography.bodySmall
                            )
                        }
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                            TextButton(onClick = { editing = contact; showSheet = true }) { Text("Edit") }
                            IconButton(onClick = { onDelete(contact) }) {
                                Icon(Icons.Outlined.Delete, contentDescription = "Delete")
                            }
                        }
                    }
                }
            }
        }
    }

    if (showSheet) {
        ModalBottomSheet(onDismissRequest = { showSheet = false }, sheetState = sheetState) {
            ContactForm(initial = editing, onDismiss = { showSheet = false }, onSave = { onSave(it); showSheet = false })
        }
    }
}

@Composable
private fun ContactForm(
    initial: FarmContact?,
    onDismiss: () -> Unit,
    onSave: (FarmContact) -> Unit
) {
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var role by remember { mutableStateOf(initial?.role ?: ContactRole.OTHER) }
    var phone by remember { mutableStateOf(initial?.phone.orEmpty()) }
    var email by remember { mutableStateOf(initial?.email.orEmpty()) }
    var organization by remember { mutableStateOf(initial?.organization.orEmpty()) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }

    FormSheet(
        title = if (initial == null) "Add contact" else "Edit contact",
        onDismiss = onDismiss,
        onSave = {
            onSave(
                FarmContact(
                    id = initial?.id ?: 0,
                    name = name.trim(),
                    role = role,
                    phone = phone.trim(),
                    email = email.trim(),
                    organization = organization.trim(),
                    notes = notes.trim()
                )
            )
        },
        saveEnabled = name.isNotBlank()
    ) {
        OutlinedTextField(value = name, onValueChange = { name = it }, label = { Text("Name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        SimpleDropdown(
            label = "Role",
            options = ContactRole.entries,
            selected = role,
            onSelected = { role = it },
            optionLabel = { prettyEnum(it.name) }
        )
        OutlinedTextField(value = organization, onValueChange = { organization = it }, label = { Text("Organization") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = phone, onValueChange = { phone = it }, label = { Text("Phone") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = email, onValueChange = { email = it }, label = { Text("Email") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(value = notes, onValueChange = { notes = it }, label = { Text("Notes") }, modifier = Modifier.fillMaxWidth())
    }
}

private fun prettyEnum(name: String): String =
    name.lowercase().replace('_', ' ').replaceFirstChar { it.titlecase() }

private fun trimQty(value: Double): String =
    if (value % 1.0 == 0.0) value.toInt().toString() else String.format("%.2f", value)
