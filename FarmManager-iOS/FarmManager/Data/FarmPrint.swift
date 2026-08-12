import SwiftUI
import UIKit

enum FarmPrint {
    static func present(html: String, jobName: String) {
        let formatter = UIMarkupTextPrintFormatter(markupText: html)
        let controller = UIPrintInteractionController.shared
        controller.printFormatter = formatter
        let info = UIPrintInfo(dictionary: nil)
        info.jobName = jobName
        info.outputType = .general
        controller.printInfo = info

        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let root = scene.windows.first(where: { $0.isKeyWindow })?.rootViewController else {
            controller.present(animated: true)
            return
        }
        controller.present(from: root.view.bounds, in: root.view, animated: true, completionHandler: nil)
    }

    static func fullReportHTML(
        farmName: String,
        animals: [AnimalGroup],
        feeds: [FeedingSchedule],
        breedings: [BreedingSchedule],
        arrivals: [AnimalArrival],
        transactions: [FarmTransaction],
        income: Double,
        expenses: Double,
        projectedFeed: Double,
        net: Double,
        margin: Double
    ) -> String {
        let printedAt = Date.now.formatted(date: .abbreviated, time: .shortened)
        let head = animals.reduce(0) { $0 + $1.count }
        var html = """
        <html><head><meta charset="utf-8"/><style>
        body{font-family:Georgia,serif;color:#1a241c;margin:24px}
        h1{color:#2F5D3A} h2{color:#3D7A6A;border-bottom:1px solid #E4EDE3;padding-bottom:4px}
        table{width:100%;border-collapse:collapse;font-size:13px}
        th,td{text-align:left;padding:6px 8px;border-bottom:1px solid #E4EDE3}
        th{background:#F3F7F0}
        </style></head><body>
        <h1>\(esc(farmName))</h1>
        <p>Farm report · Printed \(printedAt)</p>
        <h2>Summary</h2>
        <table>
        <tr><td>Livestock head count</td><td><b>\(head)</b></td></tr>
        <tr><td>Active feeds</td><td><b>\(feeds.filter(\.active).count)</b></td></tr>
        <tr><td>Active breeding</td><td><b>\(breedings.filter(\.active).count)</b></td></tr>
        <tr><td>Income</td><td><b>\(income.asCurrency)</b></td></tr>
        <tr><td>Expenses</td><td><b>\(expenses.asCurrency)</b></td></tr>
        <tr><td>Projected monthly feed</td><td><b>\(projectedFeed.asCurrency)</b></td></tr>
        <tr><td>Net profit</td><td><b>\(net.asCurrency)</b></td></tr>
        <tr><td>Margin</td><td><b>\(String(format: "%.1f%%", margin))</b></td></tr>
        </table>
        """

        html += "<h2>Livestock</h2><table><tr><th>Group</th><th>Type</th><th>Head</th><th>Cost</th></tr>"
        if animals.isEmpty {
            html += "<tr><td colspan='4'>None</td></tr>"
        } else {
            for a in animals {
                html += "<tr><td>\(esc(a.name))</td><td>\(esc(a.type.displayName))</td><td>\(a.count)</td><td>\(a.purchaseCost.asCurrency)</td></tr>"
            }
        }
        html += "</table>"

        html += "<h2>Active feeding</h2><table><tr><th>Time</th><th>Group</th><th>Feed</th><th>Amount</th><th>Daily</th></tr>"
        let activeFeeds = feeds.filter(\.active)
        if activeFeeds.isEmpty {
            html += "<tr><td colspan='5'>None</td></tr>"
        } else {
            for f in activeFeeds {
                html += "<tr><td>\(esc(f.timeOfDay))</td><td>\(esc(f.animalGroupName))</td><td>\(esc(f.feedName))</td><td>\(f.amountKg) kg</td><td>\(f.dailyCost.asCurrency)</td></tr>"
            }
        }
        html += "</table>"

        html += "<h2>Active breeding</h2><table><tr><th>Female</th><th>Group</th><th>Status</th><th>Bred</th><th>Due</th></tr>"
        let activeBreed = breedings.filter(\.active)
        if activeBreed.isEmpty {
            html += "<tr><td colspan='5'>None</td></tr>"
        } else {
            for b in activeBreed {
                html += "<tr><td>\(esc(b.femaleLabel))</td><td>\(esc(b.animalGroupName))</td><td>\(esc(b.status.displayName))</td><td>\(b.breedingDate.mediumString)</td><td>\(b.expectedDueDate.mediumString)</td></tr>"
            }
        }
        html += "</table>"

        html += "<h2>Arrivals</h2><table><tr><th>Name</th><th>Type</th><th>Origin</th><th>Date</th><th>Registration</th></tr>"
        if arrivals.isEmpty {
            html += "<tr><td colspan='5'>None</td></tr>"
        } else {
            for a in arrivals {
                html += "<tr><td>\(esc(a.displayName))</td><td>\(esc(a.type.displayName))</td><td>\(esc(a.origin.displayName))</td><td>\(a.eventDate.mediumString)</td><td>\(esc(a.registrationStatus.displayName))</td></tr>"
            }
        }
        html += "</table>"

        html += ledgerTable(transactions)
        html += "</body></html>"
        return html
    }

    static func profitReportHTML(
        farmName: String,
        income: Double,
        expenses: Double,
        projectedFeed: Double,
        net: Double,
        margin: Double,
        transactions: [FarmTransaction]
    ) -> String {
        let printedAt = Date.now.formatted(date: .abbreviated, time: .shortened)
        var html = """
        <html><head><meta charset="utf-8"/><style>
        body{font-family:Georgia,serif;margin:24px} h1{color:#2F5D3A}
        table{width:100%;border-collapse:collapse;font-size:13px}
        th,td{text-align:left;padding:6px 8px;border-bottom:1px solid #E4EDE3}
        th{background:#F3F7F0}
        </style></head><body>
        <h1>\(esc(farmName)) — Profit report</h1>
        <p>Printed \(printedAt)</p>
        <table>
        <tr><td>Income</td><td>\(income.asCurrency)</td></tr>
        <tr><td>Expenses*</td><td>\(expenses.asCurrency)</td></tr>
        <tr><td>Projected monthly feed</td><td>\(projectedFeed.asCurrency)</td></tr>
        <tr><td>Net profit</td><td>\(net.asCurrency)</td></tr>
        <tr><td>Margin</td><td>\(String(format: "%.1f%%", margin))</td></tr>
        </table>
        <p style="font-size:12px;color:#666">*Includes recorded expenses plus projected monthly feed.</p>
        """
        html += ledgerTable(transactions)
        html += "</body></html>"
        return html
    }

    private static func ledgerTable(_ transactions: [FarmTransaction]) -> String {
        var html = "<h2>Ledger</h2><table><tr><th>Date</th><th>Type</th><th>Description</th><th>Amount</th></tr>"
        if transactions.isEmpty {
            html += "<tr><td colspan='4'>None</td></tr>"
        } else {
            for tx in transactions {
                let sign = tx.type == .income ? "+" : "-"
                html += "<tr><td>\(tx.date.mediumString)</td><td>\(tx.type == .income ? "Income" : "Expense")</td><td>\(esc(tx.detail))</td><td>\(sign)\(tx.amount.asCurrency)</td></tr>"
            }
        }
        html += "</table>"
        return html
    }

    private static func esc(_ value: String) -> String {
        value
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
    }
}
