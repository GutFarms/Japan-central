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
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.FeedingScheduleWithAnimal
import com.gutfarms.manager.data.model.ProfitSummary
import com.gutfarms.manager.ui.components.MetricTile
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.SectionLabel
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
    animals: StateFlow<List<Animal>>,
    schedules: StateFlow<List<FeedingScheduleWithAnimal>>,
    profitSummary: StateFlow<ProfitSummary>,
    onOpenAnimals: () -> Unit,
    onOpenFeeding: () -> Unit,
    onOpenProfits: () -> Unit
) {
    val animalList by animals.collectAsState()
    val scheduleList by schedules.collectAsState()
    val profit by profitSummary.collectAsState()
    var visible by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        delay(80)
        visible = true
    }

    val headCount = animalList.sumOf { it.count }
    val activeFeeds = scheduleList.count { it.schedule.active }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(listOf(CreamLeaf, Mist, CreamLeaf))
            )
            .verticalScroll(rememberScrollState())
    ) {
        ScreenHeader(
            brand = "Gut Farms",
            title = "Farm management at a glance",
            subtitle = "Track livestock, feeding, and margins in one place."
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

                MetricTile(
                    label = "Profit margin",
                    value = formatPercent(profit.marginPercent),
                    accent = if (profit.marginPercent >= 0) Forest else MaterialTheme.colorScheme.error,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(onClick = onOpenProfits)
                )

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
                    QuickAction("Livestock", onOpenAnimals, Modifier.weight(1f))
                    QuickAction("Feeding", onOpenFeeding, Modifier.weight(1f))
                    QuickAction("Profits", onOpenProfits, Modifier.weight(1f))
                }
                Spacer(Modifier.height(12.dp))
            }
        }
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
