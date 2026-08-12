package com.gutfarms.manager.print

import android.content.Context
import android.print.PrintAttributes
import android.print.PrintManager
import android.webkit.WebView
import android.webkit.WebViewClient
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrivalWithGroup
import com.gutfarms.manager.data.model.BreedingScheduleWithAnimal
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedingScheduleWithAnimal
import com.gutfarms.manager.data.model.ProfitSummary
import com.gutfarms.manager.data.model.TransactionType
import com.gutfarms.manager.ui.components.formatDate
import com.gutfarms.manager.ui.components.formatMoney
import com.gutfarms.manager.ui.components.formatPercent
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

object FarmReportPrinter {
    fun printFullReport(
        context: Context,
        farmName: String,
        animals: List<Animal>,
        feeds: List<FeedingScheduleWithAnimal>,
        breedings: List<BreedingScheduleWithAnimal>,
        arrivals: List<AnimalArrivalWithGroup>,
        transactions: List<FarmTransaction>,
        profit: ProfitSummary
    ) {
        val html = buildHtml(
            farmName = farmName,
            animals = animals,
            feeds = feeds,
            breedings = breedings,
            arrivals = arrivals,
            transactions = transactions,
            profit = profit
        )
        printHtml(context, "$farmName farm report", html)
    }

    fun printProfitReport(
        context: Context,
        farmName: String,
        profit: ProfitSummary,
        transactions: List<FarmTransaction>
    ) {
        val html = buildProfitHtml(farmName, profit, transactions)
        printHtml(context, "$farmName profit report", html)
    }

    private fun printHtml(context: Context, jobName: String, html: String) {
        val webView = WebView(context.applicationContext)
        webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView, url: String) {
                val printManager = context.getSystemService(Context.PRINT_SERVICE) as PrintManager
                val adapter = view.createPrintDocumentAdapter(jobName)
                val attrs = PrintAttributes.Builder()
                    .setMediaSize(PrintAttributes.MediaSize.ISO_A4)
                    .setResolution(PrintAttributes.Resolution("farm", "Farm", 300, 300))
                    .setMinMargins(PrintAttributes.Margins.NO_MARGINS)
                    .build()
                printManager.print(jobName, adapter, attrs)
            }
        }
        webView.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
    }

    private fun buildHtml(
        farmName: String,
        animals: List<Animal>,
        feeds: List<FeedingScheduleWithAnimal>,
        breedings: List<BreedingScheduleWithAnimal>,
        arrivals: List<AnimalArrivalWithGroup>,
        transactions: List<FarmTransaction>,
        profit: ProfitSummary
    ): String {
        val printedAt = SimpleDateFormat("MMM d, yyyy h:mm a", Locale.US).format(Date())
        val headCount = animals.sumOf { it.count }
        return """
            <!DOCTYPE html><html><head><meta charset="utf-8"/>
            <style>
              body { font-family: Georgia, serif; color: #1a241c; margin: 24px; }
              h1 { color: #2F5D3A; margin-bottom: 4px; }
              h2 { color: #3D7A6A; border-bottom: 1px solid #E4EDE3; padding-bottom: 4px; margin-top: 28px; }
              .meta { color: #5a665c; margin-bottom: 20px; }
              table { width: 100%; border-collapse: collapse; margin-top: 8px; font-size: 13px; }
              th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid #E4EDE3; vertical-align: top; }
              th { background: #F3F7F0; }
              .metrics td { font-size: 15px; }
            </style></head><body>
            <h1>${esc(farmName)}</h1>
            <div class="meta">Farm report · Printed $printedAt</div>
            <h2>Summary</h2>
            <table class="metrics">
              <tr><td>Livestock head count</td><td><b>$headCount</b></td></tr>
              <tr><td>Active feeding schedules</td><td><b>${feeds.count { it.schedule.active }}</b></td></tr>
              <tr><td>Active breeding records</td><td><b>${breedings.count { it.schedule.active }}</b></td></tr>
              <tr><td>Income</td><td><b>${formatMoney(profit.totalIncome)}</b></td></tr>
              <tr><td>Expenses (incl. projected feed)</td><td><b>${formatMoney(profit.totalExpenses)}</b></td></tr>
              <tr><td>Net profit</td><td><b>${formatMoney(profit.netProfit)}</b></td></tr>
              <tr><td>Margin</td><td><b>${formatPercent(profit.marginPercent)}</b></td></tr>
            </table>
            ${sectionAnimals(animals)}
            ${sectionFeeds(feeds)}
            ${sectionBreedings(breedings)}
            ${sectionArrivals(arrivals)}
            ${sectionLedger(transactions)}
            </body></html>
        """.trimIndent()
    }

    private fun buildProfitHtml(
        farmName: String,
        profit: ProfitSummary,
        transactions: List<FarmTransaction>
    ): String {
        val printedAt = SimpleDateFormat("MMM d, yyyy h:mm a", Locale.US).format(Date())
        return """
            <!DOCTYPE html><html><head><meta charset="utf-8"/>
            <style>
              body { font-family: Georgia, serif; color: #1a241c; margin: 24px; }
              h1 { color: #2F5D3A; } h2 { color: #3D7A6A; }
              table { width: 100%; border-collapse: collapse; font-size: 13px; }
              th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid #E4EDE3; }
              th { background: #F3F7F0; }
            </style></head><body>
            <h1>${esc(farmName)} — Profit report</h1>
            <p>Printed $printedAt</p>
            <table>
              <tr><td>Income</td><td>${formatMoney(profit.totalIncome)}</td></tr>
              <tr><td>Expenses*</td><td>${formatMoney(profit.totalExpenses)}</td></tr>
              <tr><td>Projected monthly feed</td><td>${formatMoney(profit.projectedMonthlyFeedCost)}</td></tr>
              <tr><td>Net profit</td><td>${formatMoney(profit.netProfit)}</td></tr>
              <tr><td>Margin</td><td>${formatPercent(profit.marginPercent)}</td></tr>
            </table>
            <p style="font-size:12px;color:#5a665c">*Includes recorded expenses plus projected monthly feed from active schedules.</p>
            ${sectionLedger(transactions)}
            </body></html>
        """.trimIndent()
    }

    private fun sectionAnimals(animals: List<Animal>): String {
        if (animals.isEmpty()) return "<h2>Livestock</h2><p>No livestock groups.</p>"
        val rows = animals.joinToString("") {
            "<tr><td>${esc(it.name)}</td><td>${esc(pretty(it.type.name))}</td><td>${it.count}</td><td>${formatMoney(it.purchaseCost)}</td></tr>"
        }
        return """
            <h2>Livestock</h2>
            <table><tr><th>Group</th><th>Type</th><th>Head</th><th>Purchase cost</th></tr>$rows</table>
        """.trimIndent()
    }

    private fun sectionFeeds(feeds: List<FeedingScheduleWithAnimal>): String {
        val active = feeds.filter { it.schedule.active }
        if (active.isEmpty()) return "<h2>Feeding schedules</h2><p>No active feeding schedules.</p>"
        val rows = active.joinToString("") {
            val s = it.schedule
            "<tr><td>${esc(s.timeOfDay)}</td><td>${esc(it.animalName)}</td><td>${esc(s.feedName)}</td><td>${s.amountKg} kg</td><td>${formatMoney(s.dailyCost)}/day</td></tr>"
        }
        return """
            <h2>Active feeding schedules</h2>
            <table><tr><th>Time</th><th>Group</th><th>Feed</th><th>Amount</th><th>Cost</th></tr>$rows</table>
        """.trimIndent()
    }

    private fun sectionBreedings(breedings: List<BreedingScheduleWithAnimal>): String {
        val active = breedings.filter { it.schedule.active }
        if (active.isEmpty()) return "<h2>Breeding</h2><p>No active breeding schedules.</p>"
        val rows = active.joinToString("") {
            val s = it.schedule
            "<tr><td>${esc(s.femaleLabel)}</td><td>${esc(it.animalName)}</td><td>${esc(pretty(s.status.name))}</td><td>${formatDate(s.breedingDateMillis)}</td><td>${formatDate(s.expectedDueDateMillis)}</td></tr>"
        }
        return """
            <h2>Active breeding schedules</h2>
            <table><tr><th>Female</th><th>Group</th><th>Status</th><th>Bred</th><th>Due</th></tr>$rows</table>
        """.trimIndent()
    }

    private fun sectionArrivals(arrivals: List<AnimalArrivalWithGroup>): String {
        if (arrivals.isEmpty()) return "<h2>Arrivals</h2><p>No arrivals recorded.</p>"
        val rows = arrivals.joinToString("") {
            val a = it.arrival
            "<tr><td>${esc(a.displayName)}</td><td>${esc(pretty(a.type.name))}</td><td>${esc(pretty(a.origin.name))}</td><td>${formatDate(a.eventDateMillis)}</td><td>${esc(pretty(a.registrationStatus.name))}</td></tr>"
        }
        return """
            <h2>Animal arrivals</h2>
            <table><tr><th>Name</th><th>Type</th><th>Origin</th><th>Date</th><th>Registration</th></tr>$rows</table>
        """.trimIndent()
    }

    private fun sectionLedger(transactions: List<FarmTransaction>): String {
        if (transactions.isEmpty()) return "<h2>Ledger</h2><p>No transactions.</p>"
        val rows = transactions.joinToString("") { tx ->
            val sign = if (tx.type == TransactionType.INCOME) "+" else "-"
            val cat = when (tx.type) {
                TransactionType.INCOME -> tx.incomeCategory?.name
                TransactionType.EXPENSE -> tx.expenseCategory?.name
            }?.let { pretty(it) } ?: ""
            "<tr><td>${formatDate(tx.dateMillis)}</td><td>${esc(tx.type.name.lowercase().replaceFirstChar { it.titlecase() })}</td><td>${esc(tx.description)}</td><td>${esc(cat)}</td><td>$sign${formatMoney(tx.amount)}</td></tr>"
        }
        return """
            <h2>Ledger</h2>
            <table><tr><th>Date</th><th>Type</th><th>Description</th><th>Category</th><th>Amount</th></tr>$rows</table>
        """.trimIndent()
    }

    private fun esc(value: String): String =
        value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

    private fun pretty(name: String): String =
        name.lowercase().replace('_', ' ').replaceFirstChar { it.titlecase() }
}
