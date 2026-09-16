package com.gutfarms.llcmanager.tax

/**
 * Estimate-only 1099 / contractor tax helpers.
 * Rates are illustrative defaults for planning — not tax advice.
 */
object TaxRates {
    const val SOCIAL_SECURITY = 0.062
    const val MEDICARE = 0.0145
    const val SELF_EMPLOYMENT = SOCIAL_SECURITY * 2 + MEDICARE * 2 // 15.3%
    const val SE_TAXABLE_PORTION = 0.9235
    /** 2026 Social Security wage base (approx). */
    const val SS_WAGE_BASE = 184_500.0
    const val FUTA_RATE = 0.006
    const val FUTA_WAGE_BASE = 7_000.0
    const val DEFAULT_SUTA_RATE = 0.027
    const val DEFAULT_SUTA_WAGE_BASE = 10_000.0
    const val DEFAULT_FEDERAL_RATE = 0.22
    const val QBI_RATE = 0.20
}

enum class CalculatorMode {
    EMPLOYER_SAVINGS,
    RECIPIENT_SAVINGS
}

data class EmployerSavingsInput(
    val annualPay: Double,
    val includeFuta: Boolean = true,
    val sutaRate: Double = TaxRates.DEFAULT_SUTA_RATE,
    val sutaWageBase: Double = TaxRates.DEFAULT_SUTA_WAGE_BASE,
    val includeSuta: Boolean = true,
    /** Optional extras as W-2 only costs (benefits, WC, etc.). */
    val extraW2Costs: Double = 0.0
)

data class EmployerSavingsResult(
    val w2Wages: Double,
    val employerSocialSecurity: Double,
    val employerMedicare: Double,
    val futa: Double,
    val suta: Double,
    val extraCosts: Double,
    val totalW2Cost: Double,
    val contractorCost: Double,
    val estimatedSavings: Double,
    val savingsPercent: Double
)

data class RecipientSavingsInput(
    val gross1099Income: Double,
    val businessDeductions: Double,
    val federalMarginalRate: Double = TaxRates.DEFAULT_FEDERAL_RATE,
    val applyQbi: Boolean = true,
    /** Other W-2 wages that consume SS wage base. */
    val otherW2Wages: Double = 0.0
)

data class RecipientSavingsResult(
    val grossIncome: Double,
    val deductions: Double,
    val netProfit: Double,
    val seTaxableBase: Double,
    val selfEmploymentTax: Double,
    val deductibleHalfSeTax: Double,
    val qbiDeduction: Double,
    val estimatedTaxableIncome: Double,
    val estimatedIncomeTax: Double,
    val totalTaxWithDeductions: Double,
    val totalTaxWithoutDeductions: Double,
    val taxSavingsFromDeductions: Double,
    val effectiveTaxRateOnGross: Double
)

object TaxSavingsCalculator {

    fun employerSavings(input: EmployerSavingsInput): EmployerSavingsResult {
        val pay = input.annualPay.coerceAtLeast(0.0)
        val ssBase = minOf(pay, TaxRates.SS_WAGE_BASE)
        val ss = ssBase * TaxRates.SOCIAL_SECURITY
        val medicare = pay * TaxRates.MEDICARE
        val futa = if (input.includeFuta) {
            minOf(pay, TaxRates.FUTA_WAGE_BASE) * TaxRates.FUTA_RATE
        } else 0.0
        val suta = if (input.includeSuta) {
            minOf(pay, input.sutaWageBase.coerceAtLeast(0.0)) *
                input.sutaRate.coerceAtLeast(0.0)
        } else 0.0
        val extras = input.extraW2Costs.coerceAtLeast(0.0)
        val totalW2 = pay + ss + medicare + futa + suta + extras
        val savings = (totalW2 - pay).coerceAtLeast(0.0)
        val pct = if (totalW2 > 0) savings / totalW2 else 0.0
        return EmployerSavingsResult(
            w2Wages = pay,
            employerSocialSecurity = ss,
            employerMedicare = medicare,
            futa = futa,
            suta = suta,
            extraCosts = extras,
            totalW2Cost = totalW2,
            contractorCost = pay,
            estimatedSavings = savings,
            savingsPercent = pct
        )
    }

    fun recipientSavings(input: RecipientSavingsInput): RecipientSavingsResult {
        val gross = input.gross1099Income.coerceAtLeast(0.0)
        val deductions = input.businessDeductions.coerceIn(0.0, gross)
        val netWith = (gross - deductions).coerceAtLeast(0.0)
        val netWithout = gross

        fun seTax(net: Double): Double {
            if (net <= 0) return 0.0
            val base = net * TaxRates.SE_TAXABLE_PORTION
            val remainingSs = (TaxRates.SS_WAGE_BASE - input.otherW2Wages.coerceAtLeast(0.0))
                .coerceAtLeast(0.0)
            val ssPortion = minOf(base, remainingSs) * (TaxRates.SOCIAL_SECURITY * 2)
            val medicarePortion = base * (TaxRates.MEDICARE * 2)
            return ssPortion + medicarePortion
        }

        fun pack(net: Double): Triple<Double, Double, Double> {
            val se = seTax(net)
            val halfSe = se / 2.0
            val qbiBase = (net - halfSe).coerceAtLeast(0.0)
            val qbi = if (input.applyQbi) qbiBase * TaxRates.QBI_RATE else 0.0
            val taxable = (net - halfSe - qbi).coerceAtLeast(0.0)
            val incomeTax = taxable * input.federalMarginalRate.coerceIn(0.0, 0.5)
            return Triple(se, qbi, incomeTax)
        }

        val (seWith, qbiWith, incomeWith) = pack(netWith)
        val (seWithout, _, incomeWithout) = pack(netWithout)
        val totalWith = seWith + incomeWith
        val totalWithout = seWithout + incomeWithout
        val savings = (totalWithout - totalWith).coerceAtLeast(0.0)
        val halfSe = seWith / 2.0
        val taxable = (netWith - halfSe - qbiWith).coerceAtLeast(0.0)

        return RecipientSavingsResult(
            grossIncome = gross,
            deductions = deductions,
            netProfit = netWith,
            seTaxableBase = netWith * TaxRates.SE_TAXABLE_PORTION,
            selfEmploymentTax = seWith,
            deductibleHalfSeTax = halfSe,
            qbiDeduction = qbiWith,
            estimatedTaxableIncome = taxable,
            estimatedIncomeTax = incomeWith,
            totalTaxWithDeductions = totalWith,
            totalTaxWithoutDeductions = totalWithout,
            taxSavingsFromDeductions = savings,
            effectiveTaxRateOnGross = if (gross > 0) totalWith / gross else 0.0
        )
    }
}

fun formatPercent(value: Double): String =
    String.format(java.util.Locale.US, "%.1f%%", value * 100.0)
