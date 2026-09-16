package com.gutfarms.llcmanager.ui.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Business
import androidx.compose.material.icons.outlined.Calculate
import androidx.compose.material.icons.outlined.ChevronRight
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.gutfarms.llcmanager.data.model.LlcSummary
import com.gutfarms.llcmanager.ui.components.AddFab
import com.gutfarms.llcmanager.ui.components.AtmosphereBackground
import com.gutfarms.llcmanager.ui.components.ConfirmDialog
import com.gutfarms.llcmanager.ui.components.EmptyHint
import com.gutfarms.llcmanager.ui.components.FormField
import com.gutfarms.llcmanager.ui.components.MetricChip
import com.gutfarms.llcmanager.ui.components.SimpleFormDialog
import com.gutfarms.llcmanager.ui.components.formatMoney
import com.gutfarms.llcmanager.ui.theme.DeepTeal
import com.gutfarms.llcmanager.ui.theme.MistLine
import com.gutfarms.llcmanager.ui.theme.SoftCoral
import com.gutfarms.llcmanager.ui.viewmodel.LlcViewModel

@Composable
fun HomeScreen(
    viewModel: LlcViewModel,
    onOpenLlc: (Long) -> Unit,
    onOpenTaxCalculator: () -> Unit = {}
) {
    val summaries by viewModel.summaries.collectAsStateWithLifecycle()
    val allSummaries by viewModel.allSummaries.collectAsStateWithLifecycle()
    val search by viewModel.searchQuery.collectAsState()
    var showAdd by remember { mutableStateOf(false) }
    var pendingDelete by remember { mutableStateOf<LlcSummary?>(null) }

    AtmosphereBackground(modifier = Modifier.fillMaxSize()) {
        Scaffold(
            containerColor = androidx.compose.ui.graphics.Color.Transparent,
            floatingActionButton = {
                AddFab(onClick = { showAdd = true }, contentDescription = "Add LLC")
            }
        ) { padding ->
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(horizontal = 20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                item {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = "LLC Manager",
                        style = MaterialTheme.typography.displayMedium,
                        color = DeepTeal
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(
                        text = "Entities, income, deductions, and inventory — together.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.height(10.dp))
                    val totalIncome = allSummaries.sumOf { it.incomeTotal }
                    val totalDed = allSummaries.sumOf { it.deductionTotal }
                    val totalInv = allSummaries.sumOf { it.inventoryValue }
                    val lowStock = allSummaries.sumOf { it.lowStockCount }
                    val staff = allSummaries.sumOf { it.activeEmployeeCount }
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(20.dp)
                    ) {
                        MetricChip("Net", formatMoney(totalIncome - totalDed))
                        MetricChip("Income", formatMoney(totalIncome))
                        MetricChip("Deductions", formatMoney(totalDed))
                    }
                    Spacer(Modifier.height(8.dp))
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(20.dp)
                    ) {
                        MetricChip("Entities", allSummaries.size.toString())
                        MetricChip("Staff", staff.toString())
                        MetricChip("Inventory", formatMoney(totalInv))
                        if (lowStock > 0) {
                            MetricChip("Low stock", lowStock.toString())
                        }
                    }
                    Spacer(Modifier.height(10.dp))
                    OutlinedTextField(
                        value = search,
                        onValueChange = viewModel::setSearchQuery,
                        label = { Text("Search LLCs") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                    Spacer(Modifier.height(4.dp))
                    OutlinedButton(
                        onClick = onOpenTaxCalculator,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Icon(Icons.Outlined.Calculate, contentDescription = null)
                        Spacer(modifier = Modifier.padding(start = 8.dp))
                        Text("1099 tax savings calculator")
                    }
                    Spacer(Modifier.height(4.dp))
                    HorizontalDivider(color = MistLine)
                }

                if (summaries.isEmpty()) {
                    item {
                        EmptyHint(
                            if (search.isNotBlank()) "No LLCs match \"$search\"."
                            else "No LLCs yet. Tap + to add your first entity."
                        )
                    }
                }

                itemsIndexed(summaries, key = { _, s -> s.llc.id }) { index, summary ->
                    AnimatedVisibility(
                        visible = true,
                        enter = fadeIn(tween(280, delayMillis = index * 40)) +
                            slideInVertically(tween(280, delayMillis = index * 40)) { it / 3 }
                    ) {
                        LlcRow(
                            summary = summary,
                            onOpen = { onOpenLlc(summary.llc.id) },
                            onDelete = { pendingDelete = summary }
                        )
                    }
                }

                item { Spacer(Modifier.height(72.dp)) }
            }
        }
    }

    if (showAdd) {
        LlcEditorDialog(
            title = "Add LLC",
            onDismiss = { showAdd = false },
            onSave = { name, ein, state, notes ->
                viewModel.saveLlc(name = name, ein = ein, state = state, notes = notes) {
                    showAdd = false
                }
            }
        )
    }

    pendingDelete?.let { target ->
        ConfirmDialog(
            title = "Remove LLC?",
            message = "\"${target.llc.name}\" and all of its income, deductions, and inventory will be deleted.",
            onConfirm = {
                viewModel.deleteLlc(target.llc.id)
                pendingDelete = null
            },
            onDismiss = { pendingDelete = null }
        )
    }
}

@Composable
private fun LlcRow(
    summary: LlcSummary,
    onOpen: () -> Unit,
    onDelete: () -> Unit
) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen),
        color = MaterialTheme.colorScheme.surface.copy(alpha = 0.78f),
        shape = RoundedCornerShape(6.dp),
        tonalElevation = 0.dp,
        shadowElevation = 0.dp
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(
                Icons.Outlined.Business,
                contentDescription = null,
                tint = DeepTeal
            )
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(horizontal = 12.dp)
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        summary.llc.name,
                        style = MaterialTheme.typography.titleLarge,
                        modifier = Modifier.weight(1f, fill = false)
                    )
                    if (summary.lowStockCount > 0) {
                        Icon(
                            Icons.Outlined.WarningAmber,
                            contentDescription = "Low stock",
                            tint = SoftCoral,
                            modifier = Modifier.padding(start = 6.dp)
                        )
                    }
                }
                val meta = listOfNotNull(
                    summary.llc.state.takeIf { it.isNotBlank() },
                    summary.llc.ein.takeIf { it.isNotBlank() }?.let { "EIN $it" }
                ).joinToString(" · ")
                if (meta.isNotBlank()) {
                    Text(
                        meta,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Spacer(Modifier.height(6.dp))
                Text(
                    "Net ${formatMoney(summary.net)}  ·  " +
                        "${summary.activeEmployeeCount} staff  ·  " +
                        "${summary.incomeCount} income · ${summary.deductionCount} deductions  ·  " +
                        "${summary.inventoryCount} items",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            IconButton(onClick = onDelete) {
                Icon(Icons.Outlined.Delete, contentDescription = "Remove LLC")
            }
            Icon(Icons.Outlined.ChevronRight, contentDescription = null, tint = DeepTeal)
        }
    }
}

@Composable
fun LlcEditorDialog(
    title: String,
    initialName: String = "",
    initialEin: String = "",
    initialState: String = "",
    initialNotes: String = "",
    onDismiss: () -> Unit,
    onSave: (name: String, ein: String, state: String, notes: String) -> Unit
) {
    var name by remember { mutableStateOf(initialName) }
    var ein by remember { mutableStateOf(initialEin) }
    var state by remember { mutableStateOf(initialState) }
    var notes by remember { mutableStateOf(initialNotes) }

    SimpleFormDialog(
        title = title,
        onDismiss = onDismiss,
        confirmEnabled = name.isNotBlank(),
        onConfirm = { onSave(name, ein, state, notes) }
    ) {
        FormField(name, { name = it }, "LLC name")
        FormField(ein, { ein = it }, "EIN (optional)")
        FormField(state, { state = it }, "State (optional)")
        FormField(notes, { notes = it }, "Notes", singleLine = false)
    }
}
