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
import com.gutfarms.manager.data.model.AnimalType
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
fun AnimalsScreen(
    farmName: StateFlow<String>,
    animals: StateFlow<List<Animal>>,
    onSave: (Animal) -> Unit,
    onDelete: (Animal) -> Unit,
    onOpenArrivals: () -> Unit = {}
) {
    val brand by farmName.collectAsState()
    val list by animals.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<Animal?>(null) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(
                onClick = {
                    editing = null
                    showSheet = true
                }
            ) {
                Icon(Icons.Filled.Add, contentDescription = "Add livestock")
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
                title = "Livestock",
                subtitle = "Groups and herds across the farm."
            )

            Button(
                onClick = onOpenArrivals,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp)
            ) {
                Text("New animal arrivals")
            }

            LazyColumn(
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                if (list.isEmpty()) {
                    item { EmptyHint("Add your first animal group to get started.") }
                }
                items(list, key = { it.id }) { animal ->
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
                                Text(animal.name, style = MaterialTheme.typography.titleLarge)
                                Spacer(Modifier.height(4.dp))
                                Text(
                                    "${animal.type.name.lowercase().replaceFirstChar { it.titlecase() }} · ${animal.count} head",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                if (animal.purchaseCost > 0) {
                                    Text(
                                        "Purchase cost ${formatMoney(animal.purchaseCost)}",
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                                if (animal.notes.isNotBlank()) {
                                    Spacer(Modifier.height(6.dp))
                                    Text(animal.notes, style = MaterialTheme.typography.bodyMedium)
                                }
                            }
                            IconButton(onClick = {
                                editing = animal
                                showSheet = true
                            }) {
                                Text("Edit", style = MaterialTheme.typography.labelLarge)
                            }
                            IconButton(onClick = { onDelete(animal) }) {
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
            AnimalForm(
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
private fun AnimalForm(
    initial: Animal?,
    onDismiss: () -> Unit,
    onSave: (Animal) -> Unit
) {
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var type by remember { mutableStateOf(initial?.type ?: AnimalType.CATTLE) }
    var count by remember { mutableStateOf(initial?.count?.toString().orEmpty()) }
    var cost by remember { mutableStateOf(initial?.purchaseCost?.takeIf { it > 0 }?.toString().orEmpty()) }
    var notes by remember { mutableStateOf(initial?.notes.orEmpty()) }

    FormSheet(
        title = if (initial == null) "Add livestock" else "Edit livestock",
        onDismiss = onDismiss,
        onSave = {
            val parsedCount = count.toIntOrNull() ?: return@FormSheet
            onSave(
                Animal(
                    id = initial?.id ?: 0,
                    name = name.trim(),
                    type = type,
                    count = parsedCount,
                    notes = notes.trim(),
                    purchaseCost = cost.toDoubleOrNull() ?: 0.0,
                    createdAt = initial?.createdAt ?: System.currentTimeMillis()
                )
            )
        },
        saveEnabled = name.isNotBlank() && (count.toIntOrNull() ?: 0) > 0
    ) {
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text("Group name") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        SimpleDropdown(
            label = "Type",
            options = AnimalType.entries,
            selected = type,
            onSelected = { type = it },
            optionLabel = {
                it.name.lowercase().replace('_', ' ').replaceFirstChar { c -> c.titlecase() }
            }
        )
        OutlinedTextField(
            value = count,
            onValueChange = { if (it.all(Char::isDigit)) count = it },
            label = { Text("Head count") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        MoneyField(label = "Purchase cost", value = cost, onValueChange = { cost = it })
        OutlinedTextField(
            value = notes,
            onValueChange = { notes = it },
            label = { Text("Notes") },
            modifier = Modifier.fillMaxWidth()
        )
    }
}
