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
                        subtitle: "Plan rations and see projected feed cost."
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
                                    Text("\(item.animalGroupName) · \(item.amountKg, specifier: "%.1f") kg · \(item.frequency.displayName)")
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
    @State private var amount = ""
    @State private var costPerKg = ""
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
                    TextField("Amount (kg)", text: $amount).keyboardType(.decimalPad)
                    TextField("Cost per kg", text: $costPerKg).keyboardType(.decimalPad)
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
                        guard let amountKg = Double(amount), amountKg > 0, !feedName.isEmpty else { return }
                        let group = animals.first(where: { $0.name == groupName }) ?? animals.first
                        guard let group else { return }
                        if let schedule {
                            schedule.animalGroupName = group.name
                            schedule.feedName = feedName.trimmingCharacters(in: .whitespaces)
                            schedule.amountKg = amountKg
                            schedule.costPerKg = Double(costPerKg) ?? 0
                            schedule.frequency = frequency
                            schedule.timeOfDay = timeOfDay.trimmingCharacters(in: .whitespaces)
                            schedule.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                FeedingSchedule(
                                    animalGroupName: group.name,
                                    feedName: feedName.trimmingCharacters(in: .whitespaces),
                                    amountKg: amountKg,
                                    costPerKg: Double(costPerKg) ?? 0,
                                    frequency: frequency,
                                    timeOfDay: timeOfDay.trimmingCharacters(in: .whitespaces),
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(animals.isEmpty || feedName.isEmpty || Double(amount) == nil)
                }
            }
            .onAppear {
                groupName = schedule?.animalGroupName ?? animals.first?.name ?? ""
                feedName = schedule?.feedName ?? ""
                amount = schedule.map { String($0.amountKg) } ?? ""
                costPerKg = schedule.map { String($0.costPerKg) } ?? ""
                frequency = schedule?.frequency ?? .daily
                timeOfDay = schedule?.timeOfDay ?? "08:00"
                notes = schedule?.notes ?? ""
            }
        }
    }
}