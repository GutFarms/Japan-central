import SwiftUI
import SwiftData

struct RecordsHubView: View {
    let farmName: String
    @Query private var health: [HealthRecord]
    @Query private var inventory: [InventoryItem]
    @Query private var journal: [JournalEntry]
    @Query private var contacts: [FarmContact]

    private var lowStock: Int { inventory.filter(\.needsReorder).count }

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                ScreenHeader(
                    brand: farmName,
                    title: "Farm records",
                    subtitle: "Health, inventory, journal, and contacts."
                )

                NavigationLink {
                    FarmInfoView(farmName: farmName)
                } label: {
                    recordCard(title: "Farm info", detail: "Location, owner, phone, notes")
                }
                NavigationLink {
                    HealthRecordsView(farmName: farmName)
                } label: {
                    recordCard(title: "Health log", detail: "\(health.count) records")
                }
                NavigationLink {
                    InventoryView(farmName: farmName)
                } label: {
                    recordCard(
                        title: "Inventory",
                        detail: lowStock > 0
                            ? "\(inventory.count) items · \(lowStock) low stock"
                            : "\(inventory.count) items"
                    )
                }
                NavigationLink {
                    JournalView(farmName: farmName)
                } label: {
                    recordCard(title: "Farm journal", detail: "\(journal.count) entries")
                }
                NavigationLink {
                    ContactsView(farmName: farmName)
                } label: {
                    recordCard(title: "Contacts", detail: "\(contacts.count) people & vendors")
                }
            }
            .padding(.bottom, 24)
        }
        .background(FarmTheme.cream.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
    }

    private func recordCard(title: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.title3.weight(.semibold)).foregroundStyle(FarmTheme.ink)
            Text(detail).font(.subheadline).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(Color.white)
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .padding(.horizontal, 16)
    }
}

struct FarmInfoView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    @Query private var profiles: [FarmProfile]

    @State private var name = ""
    @State private var location = ""
    @State private var owner = ""
    @State private var phone = ""
    @State private var notes = ""

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                ScreenHeader(brand: farmName, title: "Farm information", subtitle: "Core details for this operation.")
                VStack(spacing: 12) {
                    TextField("Farm name", text: $name)
                    TextField("Location", text: $location)
                    TextField("Owner / manager", text: $owner)
                    TextField("Phone", text: $phone)
                    TextField("Notes", text: $notes, axis: .vertical)
                    Button("Save farm info") {
                        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !trimmed.isEmpty else { return }
                        if let profile = profiles.first {
                            profile.farmName = trimmed
                            profile.location = location.trimmingCharacters(in: .whitespaces)
                            profile.ownerName = owner.trimmingCharacters(in: .whitespaces)
                            profile.phone = phone.trimmingCharacters(in: .whitespaces)
                            profile.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                FarmProfile(
                                    farmName: trimmed,
                                    location: location.trimmingCharacters(in: .whitespaces),
                                    ownerName: owner.trimmingCharacters(in: .whitespaces),
                                    phone: phone.trimmingCharacters(in: .whitespaces),
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(FarmTheme.forest)
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                .padding(16)
                .background(Color.white)
                .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                .padding(.horizontal, 16)
            }
        }
        .background(FarmTheme.cream.ignoresSafeArea())
        .onAppear {
            let profile = profiles.first
            name = profile?.farmName ?? farmName
            location = profile?.location ?? ""
            owner = profile?.ownerName ?? ""
            phone = profile?.phone ?? ""
            notes = profile?.notes ?? ""
        }
    }
}

struct HealthRecordsView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \HealthRecord.date, order: .reverse) private var records: [HealthRecord]
    @Query(sort: \AnimalGroup.name) private var animals: [AnimalGroup]
    @State private var showEditor = false
    @State private var editing: HealthRecord?

    var body: some View {
        recordsList(
            brand: farmName,
            title: "Health records",
            subtitle: "Vaccinations, treatments, and checkups.",
            empty: "Log vaccines, illnesses, and vet visits.",
            accent: FarmTheme.softTeal,
            items: records,
            showEditor: $showEditor,
            onAdd: { editing = nil; showEditor = true },
            onDelete: { context.delete($0); try? context.save() }
        ) { record in
            Text(record.title).font(.title3.weight(.semibold))
            Text("\(record.type.displayName) · \(record.date.mediumString)").foregroundStyle(.secondary)
            let who = [record.animalLabel, record.animalGroupName].filter { !$0.isEmpty }.joined(separator: " · ")
            if !who.isEmpty { Text(who) }
            if !record.provider.isEmpty || record.cost > 0 {
                Text([record.provider, record.cost > 0 ? record.cost.asCurrency : nil].compactMap { $0 }.joined(separator: " · "))
                    .font(.subheadline)
                    .foregroundStyle(FarmTheme.softTeal)
            }
            Button("Edit") { editing = record; showEditor = true }
                .font(.subheadline.weight(.semibold))
        }
        .sheet(isPresented: $showEditor) {
            HealthEditor(record: editing, animals: animals)
        }
    }
}

struct HealthEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var record: HealthRecord?
    var animals: [AnimalGroup]

    @State private var title = ""
    @State private var type: HealthRecordType = .checkup
    @State private var groupName = ""
    @State private var animalLabel = ""
    @State private var date = Date.now
    @State private var provider = ""
    @State private var cost = ""
    @State private var notes = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Title", text: $title)
                ChoicePicker(label: "Type", options: HealthRecordType.allCases, selection: $type) { $0.displayName }
                ChoicePicker(
                    label: "Livestock group",
                    options: ([""] + animals.map(\.name)).map(StringID.init),
                    selection: Binding(
                        get: { StringID(groupName) },
                        set: { groupName = $0.value }
                    )
                ) { $0.value.isEmpty ? "None" : $0.value }
                TextField("Animal / tag label", text: $animalLabel)
                DatePicker("Date", selection: $date, displayedComponents: .date)
                TextField("Provider", text: $provider)
                TextField("Cost", text: $cost).keyboardType(.decimalPad)
                TextField("Notes", text: $notes, axis: .vertical)
            }
            .navigationTitle(record == nil ? "Add health record" : "Edit health record")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        let trimmed = title.trimmingCharacters(in: .whitespaces)
                        guard !trimmed.isEmpty else { return }
                        if let record {
                            record.title = trimmed
                            record.type = type
                            record.animalGroupName = groupName
                            record.animalLabel = animalLabel.trimmingCharacters(in: .whitespaces)
                            record.date = date
                            record.provider = provider.trimmingCharacters(in: .whitespaces)
                            record.cost = Double(cost) ?? 0
                            record.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                HealthRecord(
                                    animalGroupName: groupName,
                                    animalLabel: animalLabel.trimmingCharacters(in: .whitespaces),
                                    type: type,
                                    title: trimmed,
                                    date: date,
                                    provider: provider.trimmingCharacters(in: .whitespaces),
                                    cost: Double(cost) ?? 0,
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .onAppear {
                title = record?.title ?? ""
                type = record?.type ?? .checkup
                groupName = record?.animalGroupName ?? ""
                animalLabel = record?.animalLabel ?? ""
                date = record?.date ?? .now
                provider = record?.provider ?? ""
                cost = record.map { $0.cost > 0 ? String($0.cost) : "" } ?? ""
                notes = record?.notes ?? ""
            }
        }
    }
}

struct InventoryView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \InventoryItem.name) private var items: [InventoryItem]
    @State private var showEditor = false
    @State private var editing: InventoryItem?

    var body: some View {
        recordsList(
            brand: farmName,
            title: "Inventory",
            subtitle: "Stock levels for feed, meds, and supplies.",
            empty: "Track feed bags, medicine, and equipment.",
            accent: FarmTheme.forest,
            items: items,
            showEditor: $showEditor,
            onAdd: { editing = nil; showEditor = true },
            onDelete: { context.delete($0); try? context.save() }
        ) { item in
            Text(item.name).font(.title3.weight(.semibold))
            Text("\(item.category.displayName) · \(trimQty(item.quantity)) \(item.unit)")
                .foregroundStyle(.secondary)
            if item.needsReorder {
                Text("Reorder soon").foregroundStyle(FarmTheme.softRed).font(.subheadline.weight(.semibold))
            }
            if !item.location.isEmpty { Text(item.location).font(.caption) }
            Button("Edit") { editing = item; showEditor = true }
                .font(.subheadline.weight(.semibold))
        }
        .sheet(isPresented: $showEditor) {
            InventoryEditor(item: editing)
        }
    }
}

struct InventoryEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var item: InventoryItem?

    @State private var name = ""
    @State private var category: InventoryCategory = .supplies
    @State private var quantity = ""
    @State private var unit = "units"
    @State private var reorder = ""
    @State private var unitCost = ""
    @State private var location = ""
    @State private var notes = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Item name", text: $name)
                ChoicePicker(label: "Category", options: InventoryCategory.allCases, selection: $category) { $0.displayName }
                TextField("Quantity", text: $quantity).keyboardType(.decimalPad)
                TextField("Unit", text: $unit)
                TextField("Reorder level", text: $reorder).keyboardType(.decimalPad)
                TextField("Unit cost", text: $unitCost).keyboardType(.decimalPad)
                TextField("Location", text: $location)
                TextField("Notes", text: $notes, axis: .vertical)
            }
            .navigationTitle(item == nil ? "Add inventory" : "Edit inventory")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        let trimmed = name.trimmingCharacters(in: .whitespaces)
                        guard !trimmed.isEmpty else { return }
                        if let item {
                            item.name = trimmed
                            item.category = category
                            item.quantity = Double(quantity) ?? 0
                            item.unit = unit.trimmingCharacters(in: .whitespaces).ifEmpty("units")
                            item.reorderLevel = Double(reorder) ?? 0
                            item.unitCost = Double(unitCost) ?? 0
                            item.location = location.trimmingCharacters(in: .whitespaces)
                            item.notes = notes.trimmingCharacters(in: .whitespaces)
                            item.updatedAt = .now
                        } else {
                            context.insert(
                                InventoryItem(
                                    name: trimmed,
                                    category: category,
                                    quantity: Double(quantity) ?? 0,
                                    unit: unit.trimmingCharacters(in: .whitespaces).ifEmpty("units"),
                                    reorderLevel: Double(reorder) ?? 0,
                                    unitCost: Double(unitCost) ?? 0,
                                    location: location.trimmingCharacters(in: .whitespaces),
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .onAppear {
                name = item?.name ?? ""
                category = item?.category ?? .supplies
                quantity = item.map { String($0.quantity) } ?? ""
                unit = item?.unit ?? "units"
                reorder = item.flatMap { $0.reorderLevel > 0 ? String($0.reorderLevel) : nil } ?? ""
                unitCost = item.flatMap { $0.unitCost > 0 ? String($0.unitCost) : nil } ?? ""
                location = item?.location ?? ""
                notes = item?.notes ?? ""
            }
        }
    }
}

struct JournalView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \JournalEntry.date, order: .reverse) private var entries: [JournalEntry]
    @State private var showEditor = false
    @State private var editing: JournalEntry?

    var body: some View {
        recordsList(
            brand: farmName,
            title: "Farm journal",
            subtitle: "Daily logs, weather, pasture notes, and tasks.",
            empty: "Capture observations and day-to-day farm notes.",
            accent: FarmTheme.softTeal,
            items: entries,
            showEditor: $showEditor,
            onAdd: { editing = nil; showEditor = true },
            onDelete: { context.delete($0); try? context.save() }
        ) { entry in
            Text(entry.title).font(.title3.weight(.semibold))
            Text("\(entry.category.displayName) · \(entry.date.mediumString)").foregroundStyle(.secondary)
            if !entry.body.isEmpty { Text(entry.body) }
            Button("Edit") { editing = entry; showEditor = true }
                .font(.subheadline.weight(.semibold))
        }
        .sheet(isPresented: $showEditor) {
            JournalEditor(entry: editing)
        }
    }
}

struct JournalEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var entry: JournalEntry?

    @State private var title = ""
    @State private var category: JournalCategory = .dailyLog
    @State private var bodyText = ""
    @State private var date = Date.now
    @State private var tags = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Title", text: $title)
                ChoicePicker(label: "Category", options: JournalCategory.allCases, selection: $category) { $0.displayName }
                DatePicker("Date", selection: $date, displayedComponents: .date)
                TextField("Notes", text: $bodyText, axis: .vertical)
                TextField("Tags", text: $tags)
            }
            .navigationTitle(entry == nil ? "Add journal entry" : "Edit journal entry")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        let trimmed = title.trimmingCharacters(in: .whitespaces)
                        guard !trimmed.isEmpty else { return }
                        if let entry {
                            entry.title = trimmed
                            entry.category = category
                            entry.body = bodyText.trimmingCharacters(in: .whitespaces)
                            entry.date = date
                            entry.tags = tags.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                JournalEntry(
                                    title: trimmed,
                                    category: category,
                                    body: bodyText.trimmingCharacters(in: .whitespaces),
                                    date: date,
                                    tags: tags.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .onAppear {
                title = entry?.title ?? ""
                category = entry?.category ?? .dailyLog
                bodyText = entry?.body ?? ""
                date = entry?.date ?? .now
                tags = entry?.tags ?? ""
            }
        }
    }
}

struct ContactsView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \FarmContact.name) private var contacts: [FarmContact]
    @State private var showEditor = false
    @State private var editing: FarmContact?

    var body: some View {
        recordsList(
            brand: farmName,
            title: "Contacts",
            subtitle: "Vets, suppliers, buyers, and workers.",
            empty: "Save people and vendors you work with.",
            accent: FarmTheme.forest,
            items: contacts,
            showEditor: $showEditor,
            onAdd: { editing = nil; showEditor = true },
            onDelete: { context.delete($0); try? context.save() }
        ) { contact in
            Text(contact.name).font(.title3.weight(.semibold))
            Text(
                contact.role.displayName +
                (contact.organization.isEmpty ? "" : " · \(contact.organization)")
            )
            .foregroundStyle(.secondary)
            let reach = [contact.phone, contact.email].filter { !$0.isEmpty }.joined(separator: " · ")
            if !reach.isEmpty { Text(reach).font(.subheadline) }
            Button("Edit") { editing = contact; showEditor = true }
                .font(.subheadline.weight(.semibold))
        }
        .sheet(isPresented: $showEditor) {
            ContactEditor(contact: editing)
        }
    }
}

struct ContactEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var contact: FarmContact?

    @State private var name = ""
    @State private var role: ContactRole = .other
    @State private var organization = ""
    @State private var phone = ""
    @State private var email = ""
    @State private var notes = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name)
                ChoicePicker(label: "Role", options: ContactRole.allCases, selection: $role) { $0.displayName }
                TextField("Organization", text: $organization)
                TextField("Phone", text: $phone)
                TextField("Email", text: $email)
                TextField("Notes", text: $notes, axis: .vertical)
            }
            .navigationTitle(contact == nil ? "Add contact" : "Edit contact")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        let trimmed = name.trimmingCharacters(in: .whitespaces)
                        guard !trimmed.isEmpty else { return }
                        if let contact {
                            contact.name = trimmed
                            contact.role = role
                            contact.organization = organization.trimmingCharacters(in: .whitespaces)
                            contact.phone = phone.trimmingCharacters(in: .whitespaces)
                            contact.email = email.trimmingCharacters(in: .whitespaces)
                            contact.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                FarmContact(
                                    name: trimmed,
                                    role: role,
                                    phone: phone.trimmingCharacters(in: .whitespaces),
                                    email: email.trimmingCharacters(in: .whitespaces),
                                    organization: organization.trimmingCharacters(in: .whitespaces),
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .onAppear {
                name = contact?.name ?? ""
                role = contact?.role ?? .other
                organization = contact?.organization ?? ""
                phone = contact?.phone ?? ""
                email = contact?.email ?? ""
                notes = contact?.notes ?? ""
            }
        }
    }
}

private func recordsList<T: Identifiable>(
    brand: String,
    title: String,
    subtitle: String,
    empty: String,
    accent: Color,
    items: [T],
    showEditor: Binding<Bool>,
    onAdd: @escaping () -> Void,
    onDelete: @escaping (T) -> Void,
    @ViewBuilder row: @escaping (T) -> some View
) -> some View {
    ScrollView {
        VStack(spacing: 12) {
            ScreenHeader(brand: brand, title: title, subtitle: subtitle)
            if items.isEmpty {
                Text(empty).foregroundStyle(.secondary).padding()
            }
            ForEach(items) { item in
                VStack(alignment: .leading, spacing: 4) {
                    row(item)
                    HStack {
                        Spacer()
                        Button("Delete", role: .destructive) { onDelete(item) }
                            .font(.subheadline.weight(.semibold))
                    }
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
    .overlay(alignment: .bottomTrailing) {
        Button(action: onAdd) {
            Image(systemName: "plus")
                .font(.title2.weight(.bold))
                .foregroundStyle(.white)
                .padding(18)
                .background(accent)
                .clipShape(Circle())
        }
        .padding(24)
    }
}

private func trimQty(_ value: Double) -> String {
    value == Double(Int(value)) ? String(Int(value)) : String(format: "%.2f", value)
}

private extension String {
    func ifEmpty(_ fallback: String) -> String { isEmpty ? fallback : self }
}
