import SwiftUI
import SwiftData

struct HomeView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query private var profiles: [FarmProfile]
    @Query(sort: \AnimalGroup.name) private var animals: [AnimalGroup]
    @Query(sort: \FeedingSchedule.timeOfDay) private var feeds: [FeedingSchedule]
    @Query(sort: \BreedingSchedule.expectedDueDate) private var breedings: [BreedingSchedule]
    @Query(sort: \AnimalArrival.eventDate, order: .reverse) private var arrivals: [AnimalArrival]
    @Query private var transactions: [FarmTransaction]

    @State private var showRename = false
    @State private var draftName = ""

    private var headCount: Int { animals.reduce(0) { $0 + $1.count } }
    private var activeFeeds: Int { feeds.filter(\.active).count }
    private var activeBreeding: Int { breedings.filter(\.active).count }
    private var pendingArrivals: Int {
        arrivals.filter { $0.registrationStatus == .pending }.count
    }

    private var projectedFeed: Double {
        feeds.filter(\.active).reduce(0) { $0 + $1.monthlyCost }
    }

    private var income: Double {
        transactions.filter { $0.type == .income }.reduce(0) { $0 + $1.amount }
    }

    private var expenses: Double {
        transactions.filter { $0.type == .expense }.reduce(0) { $0 + $1.amount } + projectedFeed
    }

    private var margin: Double {
        guard income > 0 else { return 0 }
        return ((income - expenses) / income) * 100
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 14) {
                    ScreenHeader(
                        brand: farmName,
                        title: "Farm management at a glance",
                        subtitle: "Track livestock, arrivals, feeding, breeding, and margins.",
                        onBrandTap: {
                            draftName = farmName
                            showRename = true
                        }
                    )
                    .padding(.horizontal, 0)

                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                        MetricTile(label: "Livestock", value: "\(headCount)")
                        MetricTile(label: "Active feeds", value: "\(activeFeeds)", accent: FarmTheme.softTeal)
                        MetricTile(label: "Breeding", value: "\(activeBreeding)", accent: FarmTheme.softTeal)
                        MetricTile(label: "New arrivals", value: "\(pendingArrivals) pending", accent: FarmTheme.softTeal)
                    }
                    .padding(.horizontal, 16)

                    MetricTile(
                        label: "Profit margin",
                        value: String(format: "%.1f%%", margin),
                        accent: margin >= 0 ? FarmTheme.forest : FarmTheme.softRed
                    )
                    .padding(.horizontal, 16)

                    section("Recent arrivals") {
                        if arrivals.isEmpty {
                            Text("No animal arrivals recorded yet.").foregroundStyle(.secondary)
                        } else {
                            ForEach(Array(arrivals.prefix(3))) { item in
                                NavigationLink {
                                    ArrivalsView(farmName: farmName)
                                } label: {
                                    card {
                                        Text(item.displayName).font(.headline).foregroundStyle(FarmTheme.ink)
                                        Text("\(item.origin.dateLabel) \(item.eventDate.mediumString) · \(item.registrationStatus.displayName)")
                                            .font(.subheadline)
                                            .foregroundStyle(.secondary)
                                    }
                                }
                            }
                        }
                    }

                    section("Upcoming due dates") {
                        let upcoming = breedings.filter(\.active).prefix(3)
                        if upcoming.isEmpty {
                            Text("No active breeding schedules yet.").foregroundStyle(.secondary)
                        } else {
                            ForEach(Array(upcoming)) { item in
                                card {
                                    let due: String = {
                                        if item.daysUntilDue < 0 { return "Overdue by \(-item.daysUntilDue)d" }
                                        if item.daysUntilDue == 0 { return "Due today" }
                                        return "In \(item.daysUntilDue)d"
                                    }()
                                    Text("\(item.femaleLabel) · \(due)").font(.headline)
                                    Text("\(item.animalGroupName) · due \(item.expectedDueDate.mediumString)")
                                        .font(.subheadline)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    }

                    section("Today's feeding") {
                        let upcoming = feeds.filter(\.active).prefix(3)
                        if upcoming.isEmpty {
                            Text("No active feeding schedules yet.").foregroundStyle(.secondary)
                        } else {
                            ForEach(Array(upcoming)) { item in
                                card {
                                    Text("\(item.timeOfDay) · \(item.feedName)").font(.headline)
                                    Text("\(item.animalGroupName) · \(item.amountKg, specifier: "%.1f") kg · \(item.dailyCost.asCurrency)/day")
                                        .font(.subheadline)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    }

                    NavigationLink {
                        ArrivalsView(farmName: farmName)
                    } label: {
                        Text("New animal arrivals")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(FarmTheme.forest)
                            .foregroundStyle(FarmTheme.wheat)
                            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .padding(.horizontal, 16)

                    NavigationLink {
                        DataImportView(farmName: farmName)
                    } label: {
                        Text("Upload KMZ / pull site APIs")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(FarmTheme.softTeal)
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .padding(.horizontal, 16)

                    Button {
                        let html = FarmPrint.fullReportHTML(
                            farmName: farmName,
                            animals: animals,
                            feeds: feeds,
                            breedings: breedings,
                            arrivals: arrivals,
                            transactions: transactions,
                            income: income,
                            expenses: expenses,
                            projectedFeed: projectedFeed,
                            net: income - expenses,
                            margin: margin
                        )
                        FarmPrint.present(html: html, jobName: "\(farmName) farm report")
                    } label: {
                        Label("Print farm report", systemImage: "printer")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(FarmTheme.softTeal)
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .padding(.horizontal, 16)
                    .padding(.bottom, 24)
                }
            }
            .background(
                LinearGradient(colors: [FarmTheme.cream, FarmTheme.mist, FarmTheme.cream], startPoint: .top, endPoint: .bottom)
                    .ignoresSafeArea()
            )
            .navigationBarHidden(true)
            .alert("Farm name", isPresented: $showRename) {
                TextField("Name", text: $draftName)
                Button("Save") {
                    let name = draftName.trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !name.isEmpty else { return }
                    if let profile = profiles.first {
                        profile.farmName = name
                    } else {
                        context.insert(FarmProfile(farmName: name))
                    }
                    try? context.save()
                }
                Button("Cancel", role: .cancel) {}
            }
        }
    }

    @ViewBuilder
    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.headline)
                .padding(.horizontal, 20)
            content()
                .padding(.horizontal, 16)
        }
    }

    private func card<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(Color.white)
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}
