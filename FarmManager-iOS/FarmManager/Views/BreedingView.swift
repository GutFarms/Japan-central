import SwiftUI
import SwiftData

struct BreedingView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \BreedingSchedule.expectedDueDate) private var schedules: [BreedingSchedule]
    @Query(sort: \AnimalGroup.name) private var animals: [AnimalGroup]
    @State private var showEditor = false
    @State private var editing: BreedingSchedule?
    @State private var onlyActive = true

    private var visible: [BreedingSchedule] {
        onlyActive ? schedules.filter(\.active) : schedules
    }

    private var dueSoon: Int {
        schedules.filter { $0.active && (0...14).contains($0.daysUntilDue) }.count
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 12) {
                    ScreenHeader(
                        brand: farmName,
                        title: "Breeding schedules",
                        subtitle: "Plan matings and track expected due dates."
                    )

                    HStack {
                        VStack(alignment: .leading) {
                            Text("Due within 2 weeks").font(.headline)
                            Text("\(dueSoon)")
                                .font(.system(.largeTitle, design: .serif).weight(.semibold))
                                .foregroundStyle(FarmTheme.forest)
                        }
                        Spacer()
                        Toggle("Active only", isOn: $onlyActive).labelsHidden()
                        Text(onlyActive ? "Active only" : "All").font(.caption)
                    }
                    .padding(.horizontal, 16)

                    if visible.isEmpty {
                        Text("Add a breeding record to track gestation and due dates.")
                            .foregroundStyle(.secondary)
                            .padding()
                    }

                    ForEach(visible) { item in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack(alignment: .top) {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text("\(item.femaleLabel) · \(item.animalGroupName)")
                                        .font(.title3.weight(.semibold))
                                    Text("\(item.status.displayName) · \(item.method.displayName)")
                                        .foregroundStyle(.secondary)
                                    Text("Bred \(item.breedingDate.mediumString) · Expected \(item.expectedDueDate.mediumString)")
                                        .foregroundStyle(.secondary)
                                    let due: String = {
                                        if item.daysUntilDue < 0 { return "Overdue by \(-item.daysUntilDue) days" }
                                        if item.daysUntilDue == 0 { return "Due today" }
                                        return "Due in \(item.daysUntilDue) days"
                                    }()
                                    Text("\(due) · \(item.expectedOffspring) expected")
                                        .foregroundStyle(FarmTheme.softTeal)
                                    if !item.sireName.isEmpty {
                                        Text("Sire: \(item.sireName)")
                                    }
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
                BreedingEditor(schedule: editing, animals: animals)
            }
        }
    }
}

struct BreedingEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var schedule: BreedingSchedule?
    var animals: [AnimalGroup]

    @State private var groupName = ""
    @State private var femaleLabel = ""
    @State private var sireName = ""
    @State private var method: BreedingMethod = .natural
    @State private var status: BreedingStatus = .planned
    @State private var breedingDate = Date.now
    @State private var dueDate = Date.now
    @State private var offspring = "1"
    @State private var notes = ""
    @State private var autoDue = true

    private var selectedAnimal: AnimalGroup? {
        animals.first(where: { $0.name == groupName }) ?? animals.first
    }

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
                            set: { selected in
                                groupName = selected.value
                                if autoDue, let animal = animals.first(where: { $0.name == selected.value }) {
                                    dueDate = Calendar.current.date(byAdding: .day, value: animal.type.gestationDays, to: breedingDate) ?? dueDate
                                }
                            }
                        )
                    ) { id in
                        let animal = animals.first(where: { $0.name == id.value })
                        if let animal {
                            return "\(animal.name) (\(animal.type.displayName.lowercased()))"
                        }
                        return id.value
                    }

                    TextField("Female / dam label", text: $femaleLabel)
                    TextField("Sire / bull / rooster", text: $sireName)
                    ChoicePicker(label: "Method", options: BreedingMethod.allCases, selection: $method) { $0.displayName }
                    ChoicePicker(label: "Status", options: BreedingStatus.allCases, selection: $status) { $0.displayName }
                    DatePicker("Breeding date", selection: $breedingDate, displayedComponents: .date)
                        .onChange(of: breedingDate) { _, newValue in
                            if autoDue, let animal = selectedAnimal {
                                dueDate = Calendar.current.date(byAdding: .day, value: animal.type.gestationDays, to: newValue) ?? dueDate
                            }
                        }
                    DatePicker(
                        "Expected due (\(selectedAnimal?.type.gestationDays ?? 120) day default)",
                        selection: $dueDate,
                        displayedComponents: .date
                    )
                    .onChange(of: dueDate) { _, _ in autoDue = false }
                    TextField("Expected offspring", text: $offspring).keyboardType(.numberPad)
                    TextField("Notes", text: $notes, axis: .vertical)
                }
            }
            .navigationTitle(schedule == nil ? "Add breeding" : "Edit breeding")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        guard let animal = selectedAnimal, !femaleLabel.trimmingCharacters(in: .whitespaces).isEmpty else { return }
                        if let schedule {
                            schedule.animalGroupName = animal.name
                            schedule.animalType = animal.type
                            schedule.femaleLabel = femaleLabel.trimmingCharacters(in: .whitespaces)
                            schedule.sireName = sireName.trimmingCharacters(in: .whitespaces)
                            schedule.method = method
                            schedule.status = status
                            schedule.breedingDate = breedingDate
                            schedule.expectedDueDate = dueDate
                            schedule.expectedOffspring = Int(offspring) ?? 1
                            schedule.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                BreedingSchedule(
                                    animalGroupName: animal.name,
                                    animalType: animal.type,
                                    femaleLabel: femaleLabel.trimmingCharacters(in: .whitespaces),
                                    sireName: sireName.trimmingCharacters(in: .whitespaces),
                                    method: method,
                                    status: status,
                                    breedingDate: breedingDate,
                                    expectedDueDate: dueDate,
                                    expectedOffspring: Int(offspring) ?? 1,
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(animals.isEmpty || femaleLabel.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .onAppear {
                groupName = schedule?.animalGroupName ?? animals.first?.name ?? ""
                femaleLabel = schedule?.femaleLabel ?? ""
                sireName = schedule?.sireName ?? ""
                method = schedule?.method ?? .natural
                status = schedule?.status ?? .planned
                breedingDate = schedule?.breedingDate ?? .now
                if let schedule {
                    dueDate = schedule.expectedDueDate
                    autoDue = false
                } else if let animal = animals.first {
                    dueDate = Calendar.current.date(byAdding: .day, value: animal.type.gestationDays, to: breedingDate) ?? .now
                }
                offspring = schedule.map { String($0.expectedOffspring) } ?? "1"
                notes = schedule?.notes ?? ""
            }
        }
    }
}
