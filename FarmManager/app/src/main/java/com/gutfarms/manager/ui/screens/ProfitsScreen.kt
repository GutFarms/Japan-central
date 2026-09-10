package com.gutfarms.manager.ui.screens

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Print
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.data.model.ExpenseCategory
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.IncomeCategory
import com.gutfarms.manager.data.model.ProfitSummary
import com.gutfarms.manager.data.model.TransactionType
import com.gutfarms.manager.print.FarmReportPrinter
import com.gutfarms.manager.ui.components.EmptyHint
import com.gutfarms.manager.ui.components.FormSheet
import com.gutfarms.manager.ui.components.MetricTile
import com.gutfarms.manager.ui.components.MoneyField
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.SimpleDropdown
import com.gutfarms.manager.ui.components.formatMoney
import com.gutfarms.manager.ui.components.formatPercent
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Forest
import com.gutfarms.manager.ui.theme.Mist
import com.gutfarms.manager.ui.theme.SoftRed
import com.gutfarms.manager.ui.theme.SoftTeal
import kotlinx.coroutines.flow.StateFlow
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.min

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProfitsScreen(
    farmName: StateFlow<String>,
    profitSummary: StateFlow<ProfitSummary>,
    transactions: StateFlow<List<FarmTransaction>>,
    onSave: (FarmTransaction) -> Unit,
    onDelete: (FarmTransaction) -> Unit
) {
    val context = LocalContext.current
    val brand by farmName.collectAsState()
    val profit by profitSummary.collectAsState()
    val list by transactions.collectAsState()
    var showSheet by remember { mutableStateOf(false) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    val marginProgress by animateFloatAsState(
        targetValue = min(1f, ((profit.marginPercent + 50.0) / 100.0).toFloat().coerceIn(0f, 1f)),
        animationSpec = tween(700),
        label = "margin"
    )

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = { showSheet = true }) {
                Icon(Icons.Filled.Add, contentDescription = "Add transaction")
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
                title = "Profit margins",
                subtitle = "Income, expenses, and projected feed costs."
            )

            LazyColumn(
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                item {
                    Button(
                        onClick = {
                            FarmReportPrinter.printProfitReport(
                                context = context,
                                farmName = brand,
                                profit = profit,
                                transactions = list
                            )
                        },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Icon(Icons.Outlined.Print, contentDescription = null)
                        Text("  Print profit report")
                    }
                }
                item {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(18.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .padding(16.dp)
                    ) {
                        Text("Net profit", style = MaterialTheme.typography.titleMedium)
                        Text(
                            formatMoney(profit.netProfit),
                            style = MaterialTheme.typography.displayMedium,
                            color = if (profit.netProfit >= 0) Forest else SoftRed
                        )
                        Spacer(Modifier.height(8.dp))
                        Text(
                            "Margin ${formatPercent(profit.marginPercent)}",
                            style = MaterialTheme.typography.bodyLarge
                        )
                        Spacer(Modifier.height(8.dp))
                        LinearProgressIndicator(
                            progress = { marginProgress },
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(8.dp)
                                .clip(RoundedCornerShape(8.dp)),
                            color = if (profit.marginPercent >= 0) SoftTeal else SoftRed,
                            trackColor = Mist
                        )
                    }
                }

                item {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        MetricTile(
                            label = "Income",
                            value = formatMoney(profit.totalIncome),
                            accent = Forest,
                            modifier = Modifier.weight(1f)
                        )
                        MetricTile(
                            label = "Expenses*",
                            value = formatMoney(profit.totalExpenses),
                            accent = SoftRed,
                            modifier = Modifier.weight(1f)
                        )
                    }
                }

                item {
                    MetricTile(
                        label = "Projected monthly feed",
                        value = formatMoney(profit.projectedMonthlyFeedCost),
                        accent = SoftTeal,
                        modifier = Modifier.fillMaxWidth()
                    )
                    Text(
                        "*Expenses include recorded costs plus projected monthly feed from active schedules.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 6.dp, start = 4.dp)
                    )
                }

                item {
                    Text("Ledger", style = MaterialTheme.typography.titleMedium)
                }

                if (list.isEmpty()) {
                    item { EmptyHint("Log income and expenses to track margins.") }
                }

                items(list, key = { it.id }) { tx ->
                    val isIncome = tx.type == TransactionType.INCOME
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(14.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(tx.description, style = MaterialTheme.typography.titleMedium)
                            Text(
                                buildString {
                                    append(if (isIncome) "Income" else "Expense")
                                    val category = if (isIncome) {
                                        tx.incomeCategory?.name
                                    } else {
                                        tx.expenseCategory?.name
                                    }
                                    if (category != null) {
                                        append(" · ")
                                        append(category.lowercase().replace('_', ' '))
                                    }
                                    append(" · ")
                                    append(
                                        SimpleDateFormat("MMM d, yyyy", Locale.US)
                                            .format(Date(tx.dateMillis))
                                    )
                                },
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                        Text(
                            (if (isIncome) "+" else "-") + formatMoney(tx.amount),
                            style = MaterialTheme.typography.titleMedium,
                            color = if (isIncome) Forest else SoftRed
                        )
                        IconButton(onClick = { onDelete(tx) }) {
                            Icon(Icons.Outlined.Delete, contentDescription = "Delete")
                        }
                    }
                }
                item { Spacer(Modifier.height(48.dp)) }
            }
        }
    }

    if (showSheet) {
        ModalBottomSheet(
            onDismissRequest = { showSheet = false },
            sheetState = sheetState
        ) {
            TransactionForm(
                onDismiss = { showSheet = false },
                onSave = {
                    onSave(it)
                    showSheet = false
                }
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TransactionForm(
    onDismiss: () -> Unit,
    onSave: (FarmTransaction) -> Unit
) {
    var type by remember { mutableStateOf(TransactionType.INCOME) }
    var amount by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var expenseCategory by remember { mutableStateOf(ExpenseCategory.FEED) }
    var incomeCategory by remember { mutableStateOf(IncomeCategory.LIVESTOCK_SALE) }

    FormSheet(
        title = "Add transaction",
        onDismiss = onDismiss,
        onSave = {
            onSave(
                FarmTransaction(
                    type = type,
                    amount = amount.toDoubleOrNull() ?: 0.0,
                    description = description.trim(),
                    expenseCategory = if (type == TransactionType.EXPENSE) expenseCategory else null,
                    incomeCategory = if (type == TransactionType.INCOME) incomeCategory else null
                )
            )
        },
        saveEnabled = description.isNotBlank() && (amount.toDoubleOrNull() ?: 0.0) > 0
    ) {
        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
            SegmentedButton(
                selected = type == TransactionType.INCOME,
                onClick = { type = TransactionType.INCOME },
                shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2)
            ) { Text("Income") }
            SegmentedButton(
                selected = type == TransactionType.EXPENSE,
                onClick = { type = TransactionType.EXPENSE },
                shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2)
            ) { Text("Expense") }
        }
        MoneyField(label = "Amount", value = amount, onValueChange = { amount = it })
        OutlinedTextField(
            value = description,
            onValueChange = { description = it },
            label = { Text("Description") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )
        if (type == TransactionType.INCOME) {
            SimpleDropdown(
                label = "Category",
                options = IncomeCategory.entries,
                selected = incomeCategory,
                onSelected = { incomeCategory = it },
                optionLabel = { it.name.lowercase().replace('_', ' ').replaceFirstChar { c -> c.titlecase() } }
            )
        } else {
            SimpleDropdown(
                label = "Category",
                options = ExpenseCategory.entries,
                selected = expenseCategory,
                onSelected = { expenseCategory = it },
                optionLabel = { it.name.lowercase().replace('_', ' ').replaceFirstChar { c -> c.titlecase() } }
            )
        }
    }
}
