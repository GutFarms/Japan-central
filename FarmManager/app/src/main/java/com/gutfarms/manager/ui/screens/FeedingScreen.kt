package com.gutfarms.manager.ui.screens

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
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
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
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
import com.gutfarms.manager.data.model.FeedFrequency
import com.gutfarms.manager.data.model.FeedUnit
import com.gutfarms.manager.data.model.FeedingSchedule
import com.gutfarms.manager.data.model.FeedingScheduleWithAnimal
import com.gutfarms.manager.ui.components.EmptyHint
import com.gutfarms.manager.ui.components.FormSheet
import com.gutfarms.manager.ui.components.MoneyField
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.SimpleDropdown
import com.gutfarms.manager.ui.components.formatMoney
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Mist
import kotlinx.coroutines.flow.StateFlow

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FeedingScreen(
    farmName: StateFlow<String>,
    animals: StateFlow<List<Animal>>,
    schedules: StateFlow<List<FeedingScheduleWithAnimal>>,
    onSave: (FeedingSchedule) -> Unit,
    onDelete: (FeedingSchedule) -> Unit,
    onToggle: (FeedingSchedule) -> Unit
) {
    val brand by farmName.collectAsState()
    val animalList by animals.collectAsState()
    val scheduleList by schedules.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<FeedingSchedule?>(null) }
    var onlyActive by remember { mutableStateOf(false) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    val visible = if (onlyActive) scheduleList.filter { it.schedule.active } else scheduleList
    val monthlyFeed = scheduleList.filter { it.schedule.active }.sumOf { it.schedule.monthlyCost }

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(
                onClick = {
                    editing = null
                    showSheet = true
                },
                containerColor = MaterialTheme.colorScheme.secondary
            ) {
                Icon(Icons.Filled.Add, contentDescription = "Add feeding schedule")
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
        ) {
            ScreenHeader(
                brand = brand,
                title = "Feeding schedules",
                subtitle = "Track feed quantity, stock, and projected cost."
            )

            LazyColumn(
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                item {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column {
                            Text("Projected monthly feed", style = MaterialTheme.typography.titleMedium)
                            Text(
                                formatMoney(monthlyFeed),
                                style = MaterialTheme.typography.headlineMedium,
                                color = MaterialTheme.colorScheme.primary
                            )
                        }
                        FilterChip(
                            selected = onlyActive,
                            onClick = { onlyActive = !onlyActive },
                            label = { Text(if (onlyActive) "Active only" else "All") }
                        )
                    }
                }

                if (visible.isEmpty()) {
                    item { EmptyHint("Create a feeding schedule for your livestock.") }
                }

                items(visible, key = { it.schedule.id }) { item ->
                    val schedule = item.schedule
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .animateContentSize()
                            .padding(16.dp)
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    "${schedule.timeOfDay} · ${schedule.feedName}",
                                    style = MaterialTheme.typography.titleLarge
                                )
                                Spacer(Modifier.height(4.dp))
                                Text(
                                    "${item.animalName} · ${schedule.quantityLabel} · ${
                                        schedule.frequency.name.lowercase().replace('_', ' ')
                                    }",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Text(
                                    "${schedule.animalsFed} animals · ${schedule.perHeadLabel} · ${schedule.stockLabel}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Text(
                                    "${formatMoney(schedule.dailyCost)}/day · ${formatMoney(schedule.monthlyCost)}/mo",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.secondary
                                )
                            }
                            Switch(
                                checked = schedule.active,
                                onCheckedChange = { onToggle(schedule) }
                            )
                        }
                        Row(horizontalArrangement = Arrangement.End, modifier = Modifier.fillMaxWidth()) {
                            IconButton(onClick = {
                                editing = schedule
                                showSheet = true
                            }) {
                                Text("Edit", style = MaterialTheme.typography.labelLarge)
                            }
                            IconButton(onClick = { onDelete(schedule) }) {
                                Icon(Icons.Outlined.Delete, contentDescription = "Delete")
                            }
                        }
                    }
                }
            }
        }
    }

    if (showSheet) {
        ModalBottomSheet(
            onDismissRequest = { showSheet = false },
            sheetState = sheetState
        ) {
            FeedingForm(
                animals = animalList,
                initial = editing,
                onDismiss = { showSheet = false },
                onSave = {
                    onSave(it)
                    showSheet = false
                }
            )
        }
    }
}

@Composable
private fun FeedingForm(
    animals: List<Animal>,
    initial: FeedingSchedule?,
    onDismiss: () -> Unit,
    onSave: (FeedingSchedule) -> Unit
) {
    if (animals.isEmpty()) {
        Column(modifier = Modifier.padding(20.dp)) {
            Text("Add livestock first", style = MaterialTheme.typography.headlineMedium)
            Spacer(Modifier.height(8.dp))
            Text("Feeding schedules need an animal group.")
        }
        return
    }

    var animalId by remember {
        mutableStateOf(initial?.animalId ?: animals.first().id)
    }
    var feedName by remember { mutableStateOf(initial?.feedName.orEmpty()) }
    var quantity by remember { mutableStateOf(initial?.feedQuantity?.toString().orEmpty()) }
    var unit by remember { mutableStateOf(initial?.quantityUnit ?: FeedUnit.KG) }
    var costPerUnit by remember { mutableStateOf(initial?.costPerUnit?.toString().orEmpty()) }
    var animalsFed by remember {
        mutableStateOf(
            initial?.animalsFed?.toString()
                ?: animals.firstOrNull { it.id == (initial?.animalId ?: animals.first().id) }?.count?.toString()
                ?: "1"
        )
    }
    var stockOnHand by remember { mutableStateOf(initial?.stockOnHand?.toString().orEmpty()) }
    var frequency by remember { mutableStateOf(initial?.frequency ?: FeedFrequency.DAILY) }
    var timeOfDay by remember { mutableStateOf(initial?.timeOfDay ?: "08:00") }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }

    FormSheet(
        title = if (initial == null) "Add feeding" else "Edit feeding",
        onDismiss = onDismiss,
        onSave = {
            onSave(
                FeedingSchedule(
                    id = initial?.id ?: 0,
                    animalId = animalId,
                    feedName = feedName.trim(),
                    feedQuantity = quantity.toDoubleOrNull() ?: 0.0,
                    quantityUnit = unit,
                    costPerUnit = costPerUnit.toDoubleOrNull() ?: 0.0,
                    animalsFed = animalsFed.toIntOrNull()?.coerceAtLeast(1) ?: 1,
                    stockOnHand = stockOnHand.toDoubleOrNull() ?: 0.0,
                    frequency = frequency,
                    timeOfDay = timeOfDay.trim(),
                    notes = notes.trim(),
                    active = initial?.active ?: true
                )
            )
        },
        saveEnabled = feedName.isNotBlank() &&
            (quantity.toDoubleOrNull() ?: 0.0) > 0 &&
            timeOfDay.isNotBlank()
    ) {
        SimpleDropdown(
            label = "Livestock group",
            options = animals,
            selected = animals.firstOrNull { it.id == animalId } ?: animals.first(),
            onSelected = {
                animalId = it.id
                if (initial == null) animalsFed = it.count.toString()
            },
            optionLabel = { "${it.name} (${it.type.name.lowercase()})" }
        )
        OutlinedTextField(
            value = feedName,
            onValueChange = { feedName = it },
            label = { Text("Feed name") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(
            value = quantity,
            onValueChange = { if (it.isEmpty() || it.matches(Regex("^\\d*\\.?\\d{0,2}$"))) quantity = it },
            label = { Text("Quantity per feeding") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        SimpleDropdown(
            label = "Unit",
            options = FeedUnit.entries,
            selected = unit,
            onSelected = { unit = it },
            optionLabel = { it.label }
        )
        MoneyField(
            label = "Cost per ${unit.label}",
            value = costPerUnit,
            onValueChange = { costPerUnit = it }
        )
        OutlinedTextField(
            value = animalsFed,
            onValueChange = { if (it.isEmpty() || it.matches(Regex("^\\d{0,5}$"))) animalsFed = it },
            label = { Text("Animals fed") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(
            value = stockOnHand,
            onValueChange = {
                if (it.isEmpty() || it.matches(Regex("^\\d*\\.?\\d{0,2}$"))) stockOnHand = it
            },
            label = { Text("Stock on hand (${unit.label})") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        SimpleDropdown(
            label = "Frequency",
            options = FeedFrequency.entries,
            selected = frequency,
            onSelected = { frequency = it },
            optionLabel = { it.name.lowercase().replace('_', ' ').replaceFirstChar { c -> c.titlecase() } }
        )
        OutlinedTextField(
            value = timeOfDay,
            onValueChange = { timeOfDay = it },
            label = { Text("Time (HH:mm)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(
            value = notes,
            onValueChange = { notes = it },
            label = { Text("Notes") },
            modifier = Modifier.fillMaxWidth()
        )
    }
}
