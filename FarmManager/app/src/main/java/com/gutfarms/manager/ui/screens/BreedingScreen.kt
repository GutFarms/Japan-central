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
import androidx.compose.runtime.LaunchedEffect
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
import com.gutfarms.manager.data.model.BreedingMethod
import com.gutfarms.manager.data.model.BreedingSchedule
import com.gutfarms.manager.data.model.BreedingScheduleWithAnimal
import com.gutfarms.manager.data.model.BreedingStatus
import com.gutfarms.manager.ui.components.EmptyHint
import com.gutfarms.manager.ui.components.FormSheet
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.SimpleDropdown
import com.gutfarms.manager.ui.components.formatDate
import com.gutfarms.manager.ui.components.formatDateInput
import com.gutfarms.manager.ui.components.parseDateInput
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Forest
import com.gutfarms.manager.ui.theme.Mist
import com.gutfarms.manager.ui.theme.SoftTeal
import kotlinx.coroutines.flow.StateFlow

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BreedingScreen(
    farmName: StateFlow<String>,
    animals: StateFlow<List<Animal>>,
    breedingSchedules: StateFlow<List<BreedingScheduleWithAnimal>>,
    onSave: (BreedingSchedule) -> Unit,
    onDelete: (BreedingSchedule) -> Unit,
    onToggle: (BreedingSchedule) -> Unit
) {
    val brand by farmName.collectAsState()
    val animalList by animals.collectAsState()
    val scheduleList by breedingSchedules.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<BreedingSchedule?>(null) }
    var onlyActive by remember { mutableStateOf(true) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    val visible = if (onlyActive) scheduleList.filter { it.schedule.active } else scheduleList
    val dueSoon = scheduleList.count {
        it.schedule.active && it.schedule.daysUntilDue in 0..14
    }

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(
                onClick = {
                    editing = null
                    showSheet = true
                },
                containerColor = SoftTeal
            ) {
                Icon(Icons.Filled.Add, contentDescription = "Add breeding schedule")
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
                title = "Breeding schedules",
                subtitle = "Plan matings and track expected due dates."
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
                            Text("Due within 2 weeks", style = MaterialTheme.typography.titleMedium)
                            Text(
                                "$dueSoon",
                                style = MaterialTheme.typography.headlineMedium,
                                color = Forest
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
                    item { EmptyHint("Add a breeding record to track gestation and due dates.") }
                }

                items(visible, key = { it.schedule.id }) { item ->
                    val schedule = item.schedule
                    val dueLabel = when {
                        schedule.daysUntilDue < 0 -> "Overdue by ${-schedule.daysUntilDue} days"
                        schedule.daysUntilDue == 0L -> "Due today"
                        else -> "Due in ${schedule.daysUntilDue} days"
                    }
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
                                    "${schedule.femaleLabel} · ${item.animalName}",
                                    style = MaterialTheme.typography.titleLarge
                                )
                                Spacer(Modifier.height(4.dp))
                                Text(
                                    "${prettyEnum(schedule.status.name)} · ${prettyEnum(schedule.method.name)}",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Text(
                                    "Bred ${formatDate(schedule.breedingDateMillis)} · Expected ${formatDate(schedule.expectedDueDateMillis)}",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Text(
                                    "$dueLabel · ${schedule.expectedOffspring} expected",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = SoftTeal
                                )
                                if (schedule.sireName.isNotBlank()) {
                                    Text(
                                        "Sire: ${schedule.sireName}",
                                        style = MaterialTheme.typography.bodyMedium
                                    )
                                }
                            }
                            Switch(
                                checked = schedule.active,
                                onCheckedChange = { onToggle(schedule) }
                            )
                        }
                        Row(
                            horizontalArrangement = Arrangement.End,
                            modifier = Modifier.fillMaxWidth()
                        ) {
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
            BreedingForm(
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
private fun BreedingForm(
    animals: List<Animal>,
    initial: BreedingSchedule?,
    onDismiss: () -> Unit,
    onSave: (BreedingSchedule) -> Unit
) {
    if (animals.isEmpty()) {
        Column(modifier = Modifier.padding(20.dp)) {
            Text("Add livestock first", style = MaterialTheme.typography.headlineMedium)
            Spacer(Modifier.height(8.dp))
            Text("Breeding schedules need an animal group.")
        }
        return
    }

    var animalId by remember { mutableStateOf(initial?.animalId ?: animals.first().id) }
    var femaleLabel by remember { mutableStateOf(initial?.femaleLabel.orEmpty()) }
    var sireName by remember { mutableStateOf(initial?.sireName.orEmpty()) }
    var method by remember { mutableStateOf(initial?.method ?: BreedingMethod.NATURAL) }
    var status by remember { mutableStateOf(initial?.status ?: BreedingStatus.PLANNED) }
    var breedingDate by remember {
        mutableStateOf(
            formatDateInput(initial?.breedingDateMillis ?: System.currentTimeMillis())
        )
    }
    var dueDate by remember {
        mutableStateOf(
            formatDateInput(
                initial?.expectedDueDateMillis
                    ?: BreedingSchedule.expectedDueDate(
                        System.currentTimeMillis(),
                        animals.first().type
                    )
            )
        )
    }
    var offspring by remember {
        mutableStateOf((initial?.expectedOffspring ?: 1).toString())
    }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }
    var autoDue by remember { mutableStateOf(initial == null) }

    val selectedAnimal = animals.firstOrNull { it.id == animalId } ?: animals.first()

    LaunchedEffect(animalId, breedingDate, autoDue) {
        if (!autoDue) return@LaunchedEffect
        val bred = parseDateInput(breedingDate) ?: return@LaunchedEffect
        dueDate = formatDateInput(BreedingSchedule.expectedDueDate(bred, selectedAnimal.type))
    }

    FormSheet(
        title = if (initial == null) "Add breeding" else "Edit breeding",
        onDismiss = onDismiss,
        onSave = {
            val bredMillis = parseDateInput(breedingDate) ?: return@FormSheet
            val dueMillis = parseDateInput(dueDate) ?: return@FormSheet
            onSave(
                BreedingSchedule(
                    id = initial?.id ?: 0,
                    animalId = animalId,
                    femaleLabel = femaleLabel.trim(),
                    sireName = sireName.trim(),
                    method = method,
                    status = status,
                    breedingDateMillis = bredMillis,
                    expectedDueDateMillis = dueMillis,
                    expectedOffspring = offspring.toIntOrNull() ?: 1,
                    notes = notes.trim(),
                    active = initial?.active ?: true
                )
            )
        },
        saveEnabled = femaleLabel.isNotBlank() &&
            parseDateInput(breedingDate) != null &&
            parseDateInput(dueDate) != null
    ) {
        SimpleDropdown(
            label = "Livestock group",
            options = animals,
            selected = selectedAnimal,
            onSelected = { animalId = it.id },
            optionLabel = { "${it.name} (${it.type.name.lowercase()})" }
        )
        OutlinedTextField(
            value = femaleLabel,
            onValueChange = { femaleLabel = it },
            label = { Text("Female / dam label") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(
            value = sireName,
            onValueChange = { sireName = it },
            label = { Text("Sire / bull / rooster") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        SimpleDropdown(
            label = "Method",
            options = BreedingMethod.entries,
            selected = method,
            onSelected = { method = it },
            optionLabel = { prettyEnum(it.name) }
        )
        SimpleDropdown(
            label = "Status",
            options = BreedingStatus.entries,
            selected = status,
            onSelected = { status = it },
            optionLabel = { prettyEnum(it.name) }
        )
        OutlinedTextField(
            value = breedingDate,
            onValueChange = {
                breedingDate = it
                autoDue = true
            },
            label = { Text("Breeding date (yyyy-MM-dd)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(
            value = dueDate,
            onValueChange = {
                dueDate = it
                autoDue = false
            },
            label = {
                Text(
                    "Expected due (${BreedingSchedule.gestationDaysFor(selectedAnimal.type)} day default)"
                )
            },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        OutlinedTextField(
            value = offspring,
            onValueChange = { if (it.all(Char::isDigit)) offspring = it },
            label = { Text("Expected offspring") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
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

private fun prettyEnum(name: String): String =
    name.lowercase().replace('_', ' ').replaceFirstChar { it.titlecase() }
