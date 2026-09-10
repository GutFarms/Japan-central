import SwiftUI
import SwiftData

struct FeedingView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \FeedingSchedule.timeOfDay) private var schedules: [FeedingSchedule]
    @Query(sort: \AnimalGroup.name) private var animals: [AnimalGroup]
    @State private var showEditor = false
    @State private var editing: FeedingSchedule?
    @State private var onlyActive = false

    private var visible: [FeedingSchedule] {
        onlyActive ? schedules.filter(\.active) : schedules
    }

    private var monthlyFeed: Double {
        schedules.filter(\.active).reduce(0) { $0 + $1.monthlyCost }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 12) {
                    ScreenHeader(
                        brand: farmName,
                        title: "Feeding schedules",
                        subtitle: "Track feed quantity, stock, and projected cost."
                    )

                    HStack {
                        VStack(alignment: .leading) {
                            Text("Projected monthly feed").font(.headline)
                            Text(monthlyFeed.asCurrency)
                                .font(.system(.largeTitle, design: .serif).weight(.semibold))
                                .foregroundStyle(FarmTheme.forest)
                        }
                        Spacer()
                        Toggle("Active only", isOn: $onlyActive)
                            .labelsHidden()
                        Text(onlyActive ? "Active only" : "All").font(.caption)
                    }
                    .padding(.horizontal, 16)

                    if visible.isEmpty {
                        Text("Create a feeding schedule for your livestock.")
                            .foregroundStyle(.secondary)
                            .padding()
                    }

                    ForEach(visible) { item in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text("\(item.timeOfDay) · \(item.feedName)").font(.title3.weight(.semibold))
                                    Text("\(item.animalGroupName) · \(item.quantityLabel) · \(item.frequency.displayName)")
                                        .foregroundStyle(.secondary)
                                    Text("\(item.animalsFed) animals · \(item.perHeadLabel) · \(item.stockLabel)")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                    Text("\(item.dailyCost.asCurrency)/day · \(item.monthlyCost.asCurrency)/mo")
                                        .foregroundStyle(FarmTheme.softTeal)
                                }
                                Spacer()
                                Toggle("", isOn: Binding(
                                    get: { item.active },
                                    set: {
                                        item.active = $0
                                        try? context.save()
                                    }
                                ))
                            }
                            HStack {
                                Button("Edit") {
                                    editing = item
                                    showEditor = true
                                }
                                Spacer()
                                Button("Delete", role: .destructive) {
                                    context.delete(item)
                                    try? context.save()
                                }
                            }
                            .font(.subheadline.weight(.semibold))
                        }
                        .padding(16)
                        .background(Color.white)
                        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                        .padding(.horizontal, 16)
                    }
                }
                .padding(.bottom, 80)
            }
            .background(FarmTheme.cream.ignoresSafeArea())
            .navigationBarHidden(true)
            .overlay(alignment: .bottomTrailing) {
                Button {
                    editing = nil
                    showEditor = true
                } label: {
                    Image(systemName: "plus")
                        .font(.title2.weight(.bold))
                        .foregroundStyle(.white)
                        .padding(18)
                        .background(FarmTheme.softTeal)
                        .clipShape(Circle())
                }
                .padding(24)
            }
            .sheet(isPresented: $showEditor) {
                FeedingEditor(schedule: editing, animals: animals)
            }
        }
    }
}

struct FeedingEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var schedule: FeedingSchedule?
    var animals: [AnimalGroup]

    @State private var groupName = ""
    @State private var feedName = ""
    @State private var quantity = ""
    @State private var unit: FeedUnit = .kg
    @State private var costPerUnit = ""
    @State private var animalsFed = "1"
    @State private var stockOnHand = ""
    @State private var frequency: FeedFrequency = .daily
    @State private var timeOfDay = "08:00"
    @State private var notes = ""

    var body: some View {
        NavigationStack {
            Form {
                if animals.isEmpty {
                    Text("Add livestock first.")
                } else {
                    ChoicePicker(
                        label: "Livestock group",
                        options: animals.map { StringID($0.name) },
                        selection: Binding(
                            get: { StringID(groupName.isEmpty ? (animals.first?.name ?? "") : groupName) },
                            set: { groupName = $0.value }
                        )
                    ) { id in
                        let animal = animals.first(where: { $0.name == id.value })
                        if let animal {
                            return "\(animal.name) (\(animal.type.displayName.lowercased()))"
                        }
                        return id.value
                    }

                    TextField("Feed name", text: $feedName)
                    TextField("Quantity per feeding", text: $quantity).keyboardType(.decimalPad)
                    ChoicePicker(label: "Unit", options: FeedUnit.allCases, selection: $unit) { $0.label }
                    TextField("Cost per \(unit.label)", text: $costPerUnit).keyboardType(.decimalPad)
                    TextField("Animals fed", text: $animalsFed).keyboardType(.numberPad)
                    TextField("Stock on hand (\(unit.label))", text: $stockOnHand).keyboardType(.decimalPad)
                    ChoicePicker(label: "Frequency", options: FeedFrequency.allCases, selection: $frequency) { $0.displayName }
                    TextField("Time (HH:mm)", text: $timeOfDay)
                    TextField("Notes", text: $notes, axis: .vertical)
                }
            }
            .navigationTitle(schedule == nil ? "Add feeding" : "Edit feeding")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        guard let qty = Double(quantity), qty > 0, !feedName.isEmpty else { return }
                        let group = animals.first(where: { $0.name == groupName }) ?? animals.first
                        guard let group else { return }
                        if let schedule {
                            schedule.animalGroupName = group.name
                            schedule.feedName = feedName.trimmingCharacters(in: .whitespaces)
                            schedule.feedQuantity = qty
                            schedule.quantityUnit = unit
                            schedule.costPerUnit = Double(costPerUnit) ?? 0
                            schedule.animalsFed = max(Int(animalsFed) ?? 1, 1)
                            schedule.stockOnHand = Double(stockOnHand) ?? 0
                            schedule.frequency = frequency
                            schedule.timeOfDay = timeOfDay.trimmingCharacters(in: .whitespaces)
                            schedule.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                FeedingSchedule(
                                    animalGroupName: group.name,
                                    feedName: feedName.trimmingCharacters(in: .whitespaces),
                                    feedQuantity: qty,
                                    quantityUnit: unit,
                                    costPerUnit: Double(costPerUnit) ?? 0,
                                    animalsFed: max(Int(animalsFed) ?? group.count, 1),
                                    stockOnHand: Double(stockOnHand) ?? 0,
                                    frequency: frequency,
                                    timeOfDay: timeOfDay.trimmingCharacters(in: .whitespaces),
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(animals.isEmpty || feedName.isEmpty || Double(quantity) == nil)
                }
            }
            .onAppear {
                groupName = schedule?.animalGroupName ?? animals.first?.name ?? ""
                feedName = schedule?.feedName ?? ""
                quantity = schedule.map { String($0.feedQuantity) } ?? ""
                unit = schedule?.quantityUnit ?? .kg
                costPerUnit = schedule.map { String($0.costPerUnit) } ?? ""
                animalsFed = schedule.map { String($0.animalsFed) } ?? String(animals.first?.count ?? 1)
                stockOnHand = schedule.map { String($0.stockOnHand) } ?? ""
                frequency = schedule?.frequency ?? .daily
                timeOfDay = schedule?.timeOfDay ?? "08:00"
                notes = schedule?.notes ?? ""
            }
            .onChange(of: groupName) { _, newValue in
                if schedule == nil, let animal = animals.first(where: { $0.name == newValue }) {
                    animalsFed = String(animal.count)
                }
            }
        }
    }
}
