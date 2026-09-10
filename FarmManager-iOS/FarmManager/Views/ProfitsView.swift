import SwiftUI
import SwiftData

struct ProfitsView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \FarmTransaction.date, order: .reverse) private var transactions: [FarmTransaction]
    @Query private var feeds: [FeedingSchedule]
    @State private var showEditor = false

    private var income: Double {
        transactions.filter { $0.type == .income }.reduce(0) { $0 + $1.amount }
    }

    private var projectedFeed: Double {
        feeds.filter(\.active).reduce(0) { $0 + $1.monthlyCost }
    }

    private var expenses: Double {
        transactions.filter { $0.type == .expense }.reduce(0) { $0 + $1.amount } + projectedFeed
    }

    private var net: Double { income - expenses }

    private var margin: Double {
        guard income > 0 else { return 0 }
        return (net / income) * 100
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 12) {
                    ScreenHeader(
                        brand: farmName,
                        title: "Profit margins",
                        subtitle: "Income, expenses, and projected feed costs."
                    )

                    Button {
                        let html = FarmPrint.profitReportHTML(
                            farmName: farmName,
                            income: income,
                            expenses: expenses,
                            projectedFeed: projectedFeed,
                            net: net,
                            margin: margin,
                            transactions: transactions
                        )
                        FarmPrint.present(html: html, jobName: "\(farmName) profit report")
                    } label: {
                        Label("Print profit report", systemImage: "printer")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(FarmTheme.softTeal)
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .padding(.horizontal, 16)

                    VStack(alignment: .leading, spacing: 8) {
                        Text("Net profit").font(.headline)
                        Text(net.asCurrency)
                            .font(.system(.largeTitle, design: .serif).weight(.bold))
                            .foregroundStyle(net >= 0 ? FarmTheme.forest : FarmTheme.softRed)
                        Text(String(format: "Margin %.1f%%", margin))
                        ProgressView(value: min(1, max(0, (margin + 50) / 100)))
                            .tint(margin >= 0 ? FarmTheme.softTeal : FarmTheme.softRed)
                    }
                    .padding(16)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color.white)
                    .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
                    .padding(.horizontal, 16)

                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                        MetricTile(label: "Income", value: income.asCurrency, accent: FarmTheme.forest)
                        MetricTile(label: "Expenses*", value: expenses.asCurrency, accent: FarmTheme.softRed)
                    }
                    .padding(.horizontal, 16)

                    MetricTile(label: "Projected monthly feed", value: projectedFeed.asCurrency, accent: FarmTheme.softTeal)
                        .padding(.horizontal, 16)

                    Text("*Expenses include recorded costs plus projected monthly feed from active schedules.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 20)

                    Text("Ledger")
                        .font(.headline)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 20)

                    if transactions.isEmpty {
                        Text("Log income and expenses to track margins.")
                            .foregroundStyle(.secondary)
                            .padding()
                    }

                    ForEach(transactions) { tx in
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(tx.detail).font(.headline)
                                Text(ledgerSubtitle(tx))
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Text((tx.type == .income ? "+" : "-") + tx.amount.asCurrency)
                                .foregroundStyle(tx.type == .income ? FarmTheme.forest : FarmTheme.softRed)
                                .font(.headline)
                            Button(role: .destructive) {
                                context.delete(tx)
                                try? context.save()
                            } label: {
                                Image(systemName: "trash")
                            }
                        }
                        .padding(14)
                        .background(Color.white)
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                        .padding(.horizontal, 16)
                    }
                }
                .padding(.bottom, 80)
            }
            .background(FarmTheme.cream.ignoresSafeArea())
            .navigationBarHidden(true)
            .overlay(alignment: .bottomTrailing) {
                Button { showEditor = true } label: {
                    Image(systemName: "plus")
                        .font(.title2.weight(.bold))
                        .foregroundStyle(.white)
                        .padding(18)
                        .background(FarmTheme.forest)
                        .clipShape(Circle())
                }
                .padding(24)
            }
            .sheet(isPresented: $showEditor) {
                TransactionEditor()
            }
        }
    }

    private func ledgerSubtitle(_ tx: FarmTransaction) -> String {
        let kind = tx.type == .income ? "Income" : "Expense"
        let category = tx.type == .income
            ? (tx.incomeCategory?.displayName ?? "")
            : (tx.expenseCategory?.displayName ?? "")
        let cat = category.isEmpty ? "" : " · \(category)"
        return "\(kind)\(cat) · \(tx.date.mediumString)"
    }
}

struct TransactionEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss

    @State private var type: TransactionType = .income
    @State private var amount = ""
    @State private var detail = ""
    @State private var incomeCategory: IncomeCategory = .livestockSale
    @State private var expenseCategory: ExpenseCategory = .feed

    var body: some View {
        NavigationStack {
            Form {
                Picker("Type", selection: $type) {
                    Text("Income").tag(TransactionType.income)
                    Text("Expense").tag(TransactionType.expense)
                }
                .pickerStyle(.segmented)

                TextField("Amount", text: $amount).keyboardType(.decimalPad)
                TextField("Description", text: $detail)

                if type == .income {
                    ChoicePicker(label: "Category", options: IncomeCategory.allCases, selection: $incomeCategory) { $0.displayName }
                } else {
                    ChoicePicker(label: "Category", options: ExpenseCategory.allCases, selection: $expenseCategory) { $0.displayName }
                }
            }
            .navigationTitle("Add transaction")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        guard let value = Double(amount), value > 0, !detail.trimmingCharacters(in: .whitespaces).isEmpty else { return }
                        context.insert(
                            FarmTransaction(
                                type: type,
                                amount: value,
                                detail: detail.trimmingCharacters(in: .whitespaces),
                                expenseCategory: type == .expense ? expenseCategory : nil,
                                incomeCategory: type == .income ? incomeCategory : nil
                            )
                        )
                        try? context.save()
                        dismiss()
                    }
                    .disabled(detail.trimmingCharacters(in: .whitespaces).isEmpty || Double(amount) == nil)
                }
            }
        }
    }
}
