package com.gutfarms.manager.ui.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrivalWithGroup
import com.gutfarms.manager.data.model.BreedingScheduleWithAnimal
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedingScheduleWithAnimal
import com.gutfarms.manager.data.model.ProfitSummary
import com.gutfarms.manager.data.model.RegistrationStatus
import com.gutfarms.manager.print.FarmReportPrinter
import com.gutfarms.manager.ui.components.MetricTile
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.SectionLabel
import com.gutfarms.manager.ui.components.formatDate
import com.gutfarms.manager.ui.components.formatMoney
import com.gutfarms.manager.ui.components.formatPercent
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Forest
import com.gutfarms.manager.ui.theme.Mist
import com.gutfarms.manager.ui.theme.SoftTeal
import com.gutfarms.manager.ui.theme.Wheat
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow

@Composable
fun HomeScreen(
    farmName: StateFlow<String>,
    animals: StateFlow<List<Animal>>,
    schedules: StateFlow<List<FeedingScheduleWithAnimal>>,
    breedingSchedules: StateFlow<List<BreedingScheduleWithAnimal>>,
    arrivals: StateFlow<List<AnimalArrivalWithGroup>>,
    transactions: StateFlow<List<FarmTransaction>>,
    profitSummary: StateFlow<ProfitSummary>,
    onUpdateFarmName: (String) -> Unit,
    onOpenAnimals: () -> Unit,
    onOpenArrivals: () -> Unit,
    onOpenFeeding: () -> Unit,
    onOpenBreeding: () -> Unit,
    onOpenProfits: () -> Unit,
    onOpenDataImport: () -> Unit
) {
    val context = LocalContext.current
    val brand by farmName.collectAsState()
    val animalList by animals.collectAsState()
    val scheduleList by schedules.collectAsState()
    val breedingList by breedingSchedules.collectAsState()
    val arrivalList by arrivals.collectAsState()
    val transactionList by transactions.collectAsState()
    val profit by profitSummary.collectAsState()
    var visible by remember { mutableStateOf(false) }
    var showRename by remember { mutableStateOf(false) }
    var draftName by remember { mutableStateOf(brand) }

    LaunchedEffect(Unit) {
        delay(80)
        visible = true
    }

    val headCount = animalList.sumOf { it.count }
    val activeFeeds = scheduleList.count { it.schedule.active }
    val activeBreeding = breedingList.count { it.schedule.active }
    val pendingRegistration = arrivalList.count {
        it.arrival.registrationStatus == RegistrationStatus.PENDING
    }
    val recentArrivals = arrivalList.take(3)
    val upcomingBreeding = breedingList
        .filter { it.schedule.active }
        .sortedBy { it.schedule.expectedDueDateMillis }
        .take(3)

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(listOf(CreamLeaf, Mist, CreamLeaf))
            )
            .verticalScroll(rememberScrollState())
    ) {
        ScreenHeader(
            brand = brand,
            title = "Farm management at a glance",
            subtitle = "Track livestock, arrivals, feeding, breeding, and margins.",
            onBrandClick = {
                draftName = brand
                showRename = true
            }
        )

        AnimatedVisibility(
            visible = visible,
            enter = fadeIn(tween(500)) + slideInVertically(tween(500)) { it / 6 }
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    MetricTile(
                        label = "Livestock",
                        value = "$headCount",
                        modifier = Modifier
                            .weight(1f)
                            .clickable(onClick = onOpenAnimals)
                    )
                    MetricTile(
                        label = "Active feeds",
                        value = "$activeFeeds",
                        accent = SoftTeal,
                        modifier = Modifier
                            .weight(1f)
                            .clickable(onClick = onOpenFeeding)
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    MetricTile(
                        label = "Breeding",
                        value = "$activeBreeding",
                        accent = SoftTeal,
                        modifier = Modifier
                            .weight(1f)
                            .clickable(onClick = onOpenBreeding)
                    )
                    MetricTile(
                        label = "New arrivals",
                        value = "$pendingRegistration pending",
                        accent = SoftTeal,
                        modifier = Modifier
                            .weight(1f)
                            .clickable(onClick = onOpenArrivals)
                    )
                }

                MetricTile(
                    label = "Profit margin",
                    value = formatPercent(profit.marginPercent),
                    accent = if (profit.marginPercent >= 0) Forest else MaterialTheme.colorScheme.error,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(onClick = onOpenProfits)
                )

                SectionLabel("Recent arrivals")
                if (recentArrivals.isEmpty()) {
                    Text(
                        "No animal arrivals recorded yet.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                } else {
                    recentArrivals.forEach { item ->
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(16.dp))
                                .background(MaterialTheme.colorScheme.surface)
                                .clickable(onClick = onOpenArrivals)
                                .padding(16.dp)
                        ) {
                            Text(
                                item.arrival.displayName,
                                style = MaterialTheme.typography.titleMedium
                            )
                            Spacer(Modifier.height(4.dp))
                            Text(
                                "${item.arrival.eventDateLabel} ${formatDate(item.arrival.eventDateMillis)} · ${
                                    item.arrival.registrationStatus.name.lowercase().replace('_', ' ')
                                }",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }

                SectionLabel("Upcoming due dates")
                if (upcomingBreeding.isEmpty()) {
                    Text(
                        "No active breeding schedules yet.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                } else {
                    upcomingBreeding.forEach { item ->
                        val days = item.schedule.daysUntilDue
                        val dueText = when {
                            days < 0 -> "Overdue by ${-days}d"
                            days == 0L -> "Due today"
                            else -> "In ${days}d"
                        }
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(16.dp))
                                .background(MaterialTheme.colorScheme.surface)
                                .clickable(onClick = onOpenBreeding)
                                .padding(16.dp)
                        ) {
                            Text(
                                "${item.schedule.femaleLabel} · $dueText",
                                style = MaterialTheme.typography.titleMedium
                            )
                            Spacer(Modifier.height(4.dp))
                            Text(
                                "${item.animalName} · due ${formatDate(item.schedule.expectedDueDateMillis)}",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }

                SectionLabel("Today's feeding")
                val upcoming = scheduleList.filter { it.schedule.active }.take(3)
                if (upcoming.isEmpty()) {
                    Text(
                        "No active feeding schedules yet.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                } else {
                    upcoming.forEach { item ->
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(16.dp))
                                .background(MaterialTheme.colorScheme.surface)
                                .clickable(onClick = onOpenFeeding)
                                .padding(16.dp)
                        ) {
                            Text(
                                "${item.schedule.timeOfDay} · ${item.schedule.feedName}",
                                style = MaterialTheme.typography.titleMedium
                            )
                            Spacer(Modifier.height(4.dp))
                            Text(
                                "${item.animalName} · ${item.schedule.amountKg} kg · ${formatMoney(item.schedule.dailyCost)}/day",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }

                SectionLabel("Quick actions")
                Row(
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    QuickAction("Arrive", onOpenArrivals, Modifier.weight(1f))
                    QuickAction("Feed", onOpenFeeding, Modifier.weight(1f))
                    QuickAction("Breed", onOpenBreeding, Modifier.weight(1f))
                    QuickAction("Data", onOpenDataImport, Modifier.weight(1f))
                }

                Button(
                    onClick = onOpenDataImport,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp)
                ) {
                    Text("Upload KMZ / pull site APIs")
                }

                Button(
                    onClick = {
                        FarmReportPrinter.printFullReport(
                            context = context,
                            farmName = brand,
                            animals = animalList,
                            feeds = scheduleList,
                            breedings = breedingList,
                            arrivals = arrivalList,
                            transactions = transactionList,
                            profit = profit
                        )
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp)
                ) {
                    Text("Print farm report")
                }
                Spacer(Modifier.height(12.dp))
            }
        }
    }

    if (showRename) {
        AlertDialog(
            onDismissRequest = { showRename = false },
            title = { Text("Farm name") },
            text = {
                OutlinedTextField(
                    value = draftName,
                    onValueChange = { draftName = it },
                    label = { Text("Name") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        onUpdateFarmName(draftName)
                        showRename = false
                    },
                    enabled = draftName.isNotBlank()
                ) { Text("Save") }
            },
            dismissButton = {
                TextButton(onClick = { showRename = false }) { Text("Cancel") }
            }
        )
    }
}

@Composable
private fun QuickAction(label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(14.dp))
            .background(Forest)
            .clickable(onClick = onClick)
            .padding(vertical = 14.dp, horizontal = 10.dp),
        horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally
    ) {
        Text(label, color = Wheat, style = MaterialTheme.typography.labelLarge)
    }
}
