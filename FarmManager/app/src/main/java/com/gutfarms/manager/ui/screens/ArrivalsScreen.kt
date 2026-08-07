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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
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
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrival
import com.gutfarms.manager.data.model.AnimalArrivalWithGroup
import com.gutfarms.manager.data.model.AnimalType
import com.gutfarms.manager.data.model.ArrivalOrigin
import com.gutfarms.manager.data.model.RegistrationStatus
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
fun ArrivalsScreen(
    farmName: StateFlow<String>,
    animals: StateFlow<List<Animal>>,
    arrivals: StateFlow<List<AnimalArrivalWithGroup>>,
    onSave: (AnimalArrival) -> Unit,
    onDelete: (AnimalArrival) -> Unit,
    onBack: (() -> Unit)? = null
) {
    val brand by farmName.collectAsState()
    val animalList by animals.collectAsState()
    val arrivalList by arrivals.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<AnimalArrival?>(null) }
    var pendingOnly by remember { mutableStateOf(false) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    val visible = if (pendingOnly) {
        arrivalList.filter {
            it.arrival.registrationStatus == RegistrationStatus.PENDING ||
                it.arrival.registrationStatus == RegistrationStatus.EXPIRED
        }
    } else {
        arrivalList
    }
    val pendingCount = arrivalList.count {
        it.arrival.registrationStatus == RegistrationStatus.PENDING
    }

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(
                onClick = {
                    editing = null
                    showSheet = true
                },
                containerColor = Forest
            ) {
                Icon(Icons.Filled.Add, contentDescription = "Record new arrival")
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Brush.verticalGradient(listOf(CreamLeaf, Mist)))
        ) {
            if (onBack != null) {
                TextButton(
                    onClick = onBack,
                    modifier = Modifier.padding(start = 8.dp, top = 8.dp)
                ) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = null)
                    Text("Back", modifier = Modifier.padding(start = 4.dp))
                }
            }
            ScreenHeader(
                brand = brand,
                title = "New animal arrivals",
                subtitle = "Record acquire or birth date, registration, and name."
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
                            Text("Pending registration", style = MaterialTheme.typography.titleMedium)
                            Text(
                                "$pendingCount",
                                style = MaterialTheme.typography.headlineMedium,
                                color = SoftTeal
                            )
                        }
                        FilterChip(
                            selected = pendingOnly,
                            onClick = { pendingOnly = !pendingOnly },
                            label = { Text(if (pendingOnly) "Needs attention" else "All arrivals") }
                        )
                    }
                }

                if (visible.isEmpty()) {
                    item {
                        EmptyHint("Log a purchase, birth, or transfer to start the arrival record.")
                    }
                }

                items(visible, key = { it.arrival.id }) { item ->
                    val arrival = item.arrival
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .animateContentSize()
                            .padding(16.dp)
                    ) {
                        Text(arrival.displayName, style = MaterialTheme.typography.titleLarge)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            "${prettyEnum(arrival.type.name)} · ${prettyEnum(arrival.origin.name)}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            "${arrival.eventDateLabel} date · ${formatDate(arrival.eventDateMillis)}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            "Registration · ${prettyEnum(arrival.registrationStatus.name)}" +
                                if (arrival.registrationId.isNotBlank()) {
                                    " · ${arrival.registrationId}"
                                } else {
                                    ""
                                },
                            style = MaterialTheme.typography.bodyMedium,
                            color = SoftTeal
                        )
                        if (item.groupName != null) {
                            Text(
                                "Group · ${item.groupName}",
                                style = MaterialTheme.typography.bodyMedium
                            )
                        }
                        if (arrival.notes.isNotBlank()) {
                            Spacer(Modifier.height(4.dp))
                            Text(arrival.notes, style = MaterialTheme.typography.bodyMedium)
                        }
                        Row(
                            horizontalArrangement = Arrangement.End,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            IconButton(onClick = {
                                editing = arrival
                                showSheet = true
                            }) {
                                Text("Edit", style = MaterialTheme.typography.labelLarge)
                            }
                            IconButton(onClick = { onDelete(arrival) }) {
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
            ArrivalForm(
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
private fun ArrivalForm(
    animals: List<Animal>,
    initial: AnimalArrival?,
    onDismiss: () -> Unit,
    onSave: (AnimalArrival) -> Unit
) {
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var type by remember { mutableStateOf(initial?.type ?: AnimalType.CATTLE) }
    var origin by remember { mutableStateOf(initial?.origin ?: ArrivalOrigin.PURCHASED) }
    var eventDate by remember {
        mutableStateOf(formatDateInput(initial?.eventDateMillis ?: System.currentTimeMillis()))
    }
    var registrationStatus by remember {
        mutableStateOf(initial?.registrationStatus ?: RegistrationStatus.PENDING)
    }
    var registrationId by remember { mutableStateOf(initial?.registrationId.orEmpty()) }
    var groupId by remember { mutableStateOf(initial?.groupAnimalId) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }

    val dateLabel = when (origin) {
        ArrivalOrigin.BORN_ON_FARM -> "Birth date (yyyy-MM-dd)"
        ArrivalOrigin.PURCHASED -> "Acquire date (yyyy-MM-dd)"
        ArrivalOrigin.TRANSFERRED_IN -> "Transfer date (yyyy-MM-dd)"
        ArrivalOrigin.OTHER -> "Arrival date (yyyy-MM-dd)"
    }

    val groupOptions = listOf<Animal?>(null) + animals
    val selectedGroup = animals.firstOrNull { it.id == groupId }

    FormSheet(
        title = if (initial == null) "New animal arrival" else "Edit arrival",
        onDismiss = onDismiss,
        onSave = {
            val eventMillis = parseDateInput(eventDate) ?: return@FormSheet
            onSave(
                AnimalArrival(
                    id = initial?.id ?: 0,
                    name = name.trim(),
                    type = type,
                    origin = origin,
                    eventDateMillis = eventMillis,
                    registrationStatus = registrationStatus,
                    registrationId = registrationId.trim(),
                    groupAnimalId = groupId,
                    notes = notes.trim(),
                    createdAt = initial?.createdAt ?: System.currentTimeMillis()
                )
            )
        },
        saveEnabled = parseDateInput(eventDate) != null
    ) {
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text("Name (optional)") },
            placeholder = { Text("Leave blank if unnamed") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        SimpleDropdown(
            label = "Animal type",
            options = AnimalType.entries,
            selected = type,
            onSelected = { type = it },
            optionLabel = { prettyEnum(it.name) }
        )
        SimpleDropdown(
            label = "How they arrived",
            options = ArrivalOrigin.entries,
            selected = origin,
            onSelected = { origin = it },
            optionLabel = { prettyEnum(it.name) }
        )
        OutlinedTextField(
            value = eventDate,
            onValueChange = { eventDate = it },
            label = { Text(dateLabel) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        SimpleDropdown(
            label = "Registration status",
            options = RegistrationStatus.entries,
            selected = registrationStatus,
            onSelected = { registrationStatus = it },
            optionLabel = { prettyEnum(it.name) }
        )
        OutlinedTextField(
            value = registrationId,
            onValueChange = { registrationId = it },
            label = { Text("Registration / tag ID (optional)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        SimpleDropdown(
            label = "Livestock group (optional)",
            options = groupOptions,
            selected = selectedGroup,
            onSelected = { groupId = it?.id },
            optionLabel = { it?.name ?: "None" }
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
