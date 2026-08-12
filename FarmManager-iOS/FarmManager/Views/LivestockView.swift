import SwiftUI
import SwiftData

struct LivestockView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \AnimalGroup.name) private var animals: [AnimalGroup]
    @State private var showEditor = false
    @State private var editing: AnimalGroup?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 12) {
                    ScreenHeader(
                        brand: farmName,
                        title: "Livestock",
                        subtitle: "Groups and herds across the farm."
                    )

                    NavigationLink {
                        ArrivalsView(farmName: farmName)
                    } label: {
                        Text("New animal arrivals")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(FarmTheme.forest)
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                    .padding(.horizontal, 16)

                    if animals.isEmpty {
                        Text("Add your first animal group to get started.")
                            .foregroundStyle(.secondary)
                            .padding()
                    }

                    ForEach(animals) { animal in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(animal.name).font(.title3.weight(.semibold))
                            Text("\(animal.type.displayName) · \(animal.count) head")
                                .foregroundStyle(.secondary)
                            if animal.purchaseCost > 0 {
                                Text("Purchase cost \(animal.purchaseCost.asCurrency)")
                                    .foregroundStyle(.secondary)
                            }
                            if !animal.notes.isEmpty {
                                Text(animal.notes)
                            }
                            HStack {
                                Button("Edit") {
                                    editing = animal
                                    showEditor = true
                                }
                                Spacer()
                                Button("Delete", role: .destructive) {
                                    context.delete(animal)
                                    try? context.save()
                                }
                            }
                            .font(.subheadline.weight(.semibold))
                        }
                        .padding(16)
                        .frame(maxWidth: .infinity, alignment: .leading)
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
                        .background(FarmTheme.forest)
                        .clipShape(Circle())
                        .shadow(radius: 4)
                }
                .padding(24)
            }
            .sheet(isPresented: $showEditor) {
                AnimalEditor(animal: editing)
            }
        }
    }
}

struct AnimalEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var animal: AnimalGroup?

    @State private var name = ""
    @State private var type: AnimalType = .cattle
    @State private var count = "1"
    @State private var cost = ""
    @State private var notes = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Group name", text: $name)
                ChoicePicker(label: "Type", options: AnimalType.allCases, selection: $type) { $0.displayName }
                TextField("Head count", text: $count)
                    .keyboardType(.numberPad)
                TextField("Purchase cost", text: $cost)
                    .keyboardType(.decimalPad)
                TextField("Notes", text: $notes, axis: .vertical)
            }
            .navigationTitle(animal == nil ? "Add livestock" : "Edit livestock")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        guard let parsed = Int(count), parsed > 0, !name.trimmingCharacters(in: .whitespaces).isEmpty else { return }
                        if let animal {
                            animal.name = name.trimmingCharacters(in: .whitespaces)
                            animal.type = type
                            animal.count = parsed
                            animal.purchaseCost = Double(cost) ?? 0
                            animal.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                AnimalGroup(
                                    name: name.trimmingCharacters(in: .whitespaces),
                                    type: type,
                                    count: parsed,
                                    notes: notes.trimmingCharacters(in: .whitespaces),
                                    purchaseCost: Double(cost) ?? 0
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || Int(count) == nil)
                }
            }
            .onAppear {
                if let animal {
                    name = animal.name
                    type = animal.type
                    count = "\(animal.count)"
                    cost = animal.purchaseCost > 0 ? String(animal.purchaseCost) : ""
                    notes = animal.notes
                }
            }
        }
    }
}
