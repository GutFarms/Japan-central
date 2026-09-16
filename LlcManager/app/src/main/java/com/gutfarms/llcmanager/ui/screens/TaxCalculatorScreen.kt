package com.gutfarms.llcmanager.ui.screens

import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.gutfarms.llcmanager.tax.CalculatorMode
import com.gutfarms.llcmanager.tax.EmployerSavingsInput
import com.gutfarms.llcmanager.tax.RecipientSavingsInput
import com.gutfarms.llcmanager.tax.TaxRates
import com.gutfarms.llcmanager.tax.TaxSavingsCalculator
import com.gutfarms.llcmanager.tax.formatPercent
import com.gutfarms.llcmanager.ui.components.AtmosphereBackground
import com.gutfarms.llcmanager.ui.components.FormField
import com.gutfarms.llcmanager.ui.components.MetricChip
import com.gutfarms.llcmanager.ui.components.formatMoney
import com.gutfarms.llcmanager.ui.theme.DeepTeal
import com.gutfarms.llcmanager.ui.theme.MistLine
import com.gutfarms.llcmanager.ui.theme.Olive

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TaxCalculatorScreen(
    onBack: () -> Unit,
    initialGrossIncome: Double = 0.0,
    initialDeductions: Double = 0.0
) {
    var mode by remember { mutableStateOf(CalculatorMode.EMPLOYER_SAVINGS) }
    var payText by remember {
        mutableStateOf(
            if (initialGrossIncome > 0) trimNumber(initialGrossIncome) else ""
        )
    }
    var includeFuta by remember { mutableStateOf(true) }
    var includeSuta by remember { mutableStateOf(true) }
    var sutaRateText by remember { mutableStateOf("2.7") }
    var extrasText by remember { mutableStateOf("") }

    var grossText by remember {
        mutableStateOf(
            if (initialGrossIncome > 0) trimNumber(initialGrossIncome) else ""
        )
    }
    var deductionsText by remember {
        mutableStateOf(
            if (initialDeductions > 0) trimNumber(initialDeductions) else ""
        )
    }
    var federalRateText by remember { mutableStateOf("22") }
    var applyQbi by remember { mutableStateOf(true) }
    var otherW2Text by remember { mutableStateOf("") }

    AtmosphereBackground(modifier = Modifier.fillMaxSize()) {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                TopAppBar(
                    title = {
                        Column {
                            Text("1099 tax savings", style = MaterialTheme.typography.titleLarge)
                            Text(
                                "Planning estimates only — not tax advice",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
                )
            }
        ) { padding ->
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(horizontal = 20.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Row(
                    modifier = Modifier.horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    FilterChip(
                        selected = mode == CalculatorMode.EMPLOYER_SAVINGS,
                        onClick = { mode = CalculatorMode.EMPLOYER_SAVINGS },
                        label = { Text("Employer savings") }
                    )
                    FilterChip(
                        selected = mode == CalculatorMode.RECIPIENT_SAVINGS,
                        onClick = { mode = CalculatorMode.RECIPIENT_SAVINGS },
                        label = { Text("1099 income") }
                    )
                }

                Text(
                    when (mode) {
                        CalculatorMode.EMPLOYER_SAVINGS ->
                            "Compare what your LLC pays for the same amount as a W-2 employee vs a 1099 contractor."
                        CalculatorMode.RECIPIENT_SAVINGS ->
                            "Estimate self-employment and income tax, and how much deductions save on 1099 income."
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                when (mode) {
                    CalculatorMode.EMPLOYER_SAVINGS -> {
                        FormField(
                            payText,
                            { payText = filterMoney(it) },
                            "Annual pay / contract amount",
                            keyboardType = KeyboardType.Decimal
                        )
                        ToggleRow("Include FUTA (0.6% on first \$7k)", includeFuta) {
                            includeFuta = it
                        }
                        ToggleRow("Include state unemployment (SUTA)", includeSuta) {
                            includeSuta = it
                        }
                        if (includeSuta) {
                            FormField(
                                sutaRateText,
                                { sutaRateText = filterMoney(it) },
                                "SUTA rate %",
                                keyboardType = KeyboardType.Decimal
                            )
                        }
                        FormField(
                            extrasText,
                            { extrasText = filterMoney(it) },
                            "Extra W-2-only costs (benefits, WC…)",
                            keyboardType = KeyboardType.Decimal
                        )

                        val pay = payText.toDoubleOrNull() ?: 0.0
                        val result = TaxSavingsCalculator.employerSavings(
                            EmployerSavingsInput(
                                annualPay = pay,
                                includeFuta = includeFuta,
                                includeSuta = includeSuta,
                                sutaRate = (sutaRateText.toDoubleOrNull() ?: 2.7) / 100.0,
                                extraW2Costs = extrasText.toDoubleOrNull() ?: 0.0
                            )
                        )

                        ResultPanel {
                            MetricChip("Est. employer savings", formatMoney(result.estimatedSavings))
                            Spacer(Modifier.height(8.dp))
                            Text(
                                "${formatPercent(result.savingsPercent)} of total W-2 cost",
                                style = MaterialTheme.typography.bodyMedium,
                                color = Olive
                            )
                            Spacer(Modifier.height(12.dp))
                            HorizontalDivider(color = MistLine)
                            Spacer(Modifier.height(10.dp))
                            LineItem("W-2 wages", result.w2Wages)
                            LineItem("Employer Social Security (6.2%)", result.employerSocialSecurity)
                            LineItem("Employer Medicare (1.45%)", result.employerMedicare)
                            if (includeFuta) LineItem("FUTA", result.futa)
                            if (includeSuta) LineItem("SUTA", result.suta)
                            if (result.extraCosts > 0) LineItem("Extra W-2 costs", result.extraCosts)
                            LineItem("Total W-2 cost", result.totalW2Cost, emphasize = true)
                            LineItem("1099 contractor cost", result.contractorCost, emphasize = true)
                        }
                    }

                    CalculatorMode.RECIPIENT_SAVINGS -> {
                        FormField(
                            grossText,
                            { grossText = filterMoney(it) },
                            "Gross 1099 income",
                            keyboardType = KeyboardType.Decimal
                        )
                        FormField(
                            deductionsText,
                            { deductionsText = filterMoney(it) },
                            "Business deductions",
                            keyboardType = KeyboardType.Decimal
                        )
                        FormField(
                            federalRateText,
                            { federalRateText = filterMoney(it) },
                            "Federal marginal rate %",
                            keyboardType = KeyboardType.Decimal
                        )
                        FormField(
                            otherW2Text,
                            { otherW2Text = filterMoney(it) },
                            "Other W-2 wages (SS wage base)",
                            keyboardType = KeyboardType.Decimal
                        )
                        ToggleRow("Apply QBI deduction (20%)", applyQbi) { applyQbi = it }

                        val result = TaxSavingsCalculator.recipientSavings(
                            RecipientSavingsInput(
                                gross1099Income = grossText.toDoubleOrNull() ?: 0.0,
                                businessDeductions = deductionsText.toDoubleOrNull() ?: 0.0,
                                federalMarginalRate = (federalRateText.toDoubleOrNull() ?: 22.0) / 100.0,
                                applyQbi = applyQbi,
                                otherW2Wages = otherW2Text.toDoubleOrNull() ?: 0.0
                            )
                        )

                        ResultPanel {
                            MetricChip(
                                "Tax savings from deductions",
                                formatMoney(result.taxSavingsFromDeductions)
                            )
                            Spacer(Modifier.height(8.dp))
                            Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                                MetricChip("SE tax", formatMoney(result.selfEmploymentTax))
                                MetricChip("Income tax", formatMoney(result.estimatedIncomeTax))
                            }
                            Spacer(Modifier.height(8.dp))
                            Text(
                                "Effective ~${formatPercent(result.effectiveTaxRateOnGross)} of gross",
                                style = MaterialTheme.typography.bodyMedium,
                                color = Olive
                            )
                            Spacer(Modifier.height(12.dp))
                            HorizontalDivider(color = MistLine)
                            Spacer(Modifier.height(10.dp))
                            LineItem("Gross 1099 income", result.grossIncome)
                            LineItem("Business deductions", result.deductions)
                            LineItem("Net profit", result.netProfit, emphasize = true)
                            LineItem("SE taxable base (92.35%)", result.seTaxableBase)
                            LineItem("Self-employment tax (15.3%)", result.selfEmploymentTax)
                            LineItem("½ SE tax deduction", result.deductibleHalfSeTax)
                            if (applyQbi) LineItem("QBI deduction (20%)", result.qbiDeduction)
                            LineItem("Est. taxable income", result.estimatedTaxableIncome)
                            LineItem("Est. federal income tax", result.estimatedIncomeTax)
                            LineItem("Total tax with deductions", result.totalTaxWithDeductions, emphasize = true)
                            LineItem("Total tax without deductions", result.totalTaxWithoutDeductions)
                        }
                    }
                }

                Text(
                    "Uses SS ${formatPercent(TaxRates.SOCIAL_SECURITY)}, Medicare ${formatPercent(TaxRates.MEDICARE)}, " +
                        "SE ${formatPercent(TaxRates.SELF_EMPLOYMENT)}, SS wage base ${formatMoney(TaxRates.SS_WAGE_BASE)}. " +
                        "State income tax not included. Confirm with a tax professional.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(24.dp))
            }
        }
    }
}

@Composable
private fun ToggleRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
private fun ResultPanel(content: @Composable () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.surface.copy(alpha = 0.82f),
        shape = RoundedCornerShape(6.dp),
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(modifier = Modifier.padding(16.dp), content = { content() })
    }
}

@Composable
private fun LineItem(label: String, amount: Double, emphasize: Boolean = false) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            label,
            style = if (emphasize) MaterialTheme.typography.titleMedium else MaterialTheme.typography.bodyMedium,
            color = if (emphasize) DeepTeal else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f)
        )
        Text(
            formatMoney(amount),
            style = if (emphasize) MaterialTheme.typography.titleMedium else MaterialTheme.typography.bodyMedium,
            color = if (emphasize) DeepTeal else MaterialTheme.colorScheme.onSurface
        )
    }
}

private fun filterMoney(raw: String): String = raw.filter { it.isDigit() || it == '.' }

private fun trimNumber(value: Double): String =
    if (value % 1.0 == 0.0) value.toLong().toString() else value.toString()
