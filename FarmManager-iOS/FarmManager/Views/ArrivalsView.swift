import SwiftUI
import SwiftData

struct ArrivalsView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \AnimalArrival.eventDate, order: .reverse) private var arrivals: [AnimalArrival]
    @Query(sort: \AnimalGroup.name) private var animals: [AnimalGroup]
    @State private var showEditor = false
    @State private var editing: AnimalArrival?
    @State private var pendingOnly = false

    private var visible: [AnimalArrival] {
        if pendingOnly {
            return arrivals.filter { $0.registrationStatus == .pending || $0.registrationStatus == .expired }
        }
        return arrivals
    }

    private var pendingCount: Int {
        arrivals.filter { $0.registrationStatus == .pending }.count
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                ScreenHeader(
                    brand: farmName,
                    title: "New animal arrivals",
                    subtitle: "Record acquire or birth date, registration, and name."
                )

                HStack {
                    VStack(alignment: .leading) {
                        Text("Pending registration").font(.headline)
                        Text("\(pendingCount)")
                            .font(.system(.largeTitle, design: .serif).weight(.semibold))
                            .foregroundStyle(FarmTheme.softTeal)
                    }
                    Spacer()
                    Toggle(pendingOnly ? "Needs attention" : "All arrivals", isOn: $pendingOnly)
                        .labelsHidden()
                    Text(pendingOnly ? "Needs attention" : "All")
                        .font(.caption)
                }
                .padding(.horizontal, 16)

                if visible.isEmpty {
                    Text("Log a purchase, birth, or transfer to start the arrival record.")
                        .foregroundStyle(.secondary)
                        .padding()
                }

                ForEach(visible) { item in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(item.displayName).font(.title3.weight(.semibold))
                        Text("\(item.type.displayName) · \(item.origin.displayName)")
                            .foregroundStyle(.secondary)
                        Text("\(item.origin.dateLabel) date · \(item.eventDate.mediumString)")
                            .foregroundStyle(.secondary)
                        Text("Registration · \(item.registrationStatus.displayName)" + (item.registrationId.isEmpty ? "" : " · \(item.registrationId)"))
                            .foregroundStyle(FarmTheme.softTeal)
                        if !item.groupName.isEmpty {
                            Text("Group · \(item.groupName)")
                        }
                        if !item.notes.isEmpty {
                            Text(item.notes)
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
                        .padding(.top, 4)
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
            }
            .padding(24)
        }
        .sheet(isPresented: $showEditor) {
            ArrivalEditor(arrival: editing, groups: animals.map(\.name))
        }
    }
}

struct ArrivalEditor: View {
    @Environment(\.modelContext) private var context
    @Environment(\.dismiss) private var dismiss
    var arrival: AnimalArrival?
    var groups: [String]

    @State private var name = ""
    @State private var type: AnimalType = .cattle
    @State private var origin: ArrivalOrigin = .purchased
    @State private var eventDate = Date.now
    @State private var registrationStatus: RegistrationStatus = .pending
    @State private var registrationId = ""
    @State private var groupName = "None"
    @State private var notes = ""

    private var groupOptions: [String] { ["None"] + groups }

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name (optional)", text: $name)
                ChoicePicker(label: "Animal type", options: AnimalType.allCases, selection: $type) { $0.displayName }
                ChoicePicker(label: "How they arrived", options: ArrivalOrigin.allCases, selection: $origin) { $0.displayName }
                DatePicker("\(origin.dateLabel) date", selection: $eventDate, displayedComponents: .date)
                ChoicePicker(label: "Registration status", options: RegistrationStatus.allCases, selection: $registrationStatus) { $0.displayName }
                TextField("Registration / tag ID (optional)", text: $registrationId)
                if groupOptions.count > 5 {
                    ChoicePicker(label: "Livestock group", options: groupOptions.map { StringID($0) }, selection: Binding(
                        get: { StringID(groupName) },
                        set: { groupName = $0.value }
                    )) { $0.value }
                } else {
                    Picker("Livestock group", selection: $groupName) {
                        ForEach(groupOptions, id: \.self) { Text($0) }
                    }
                }
                TextField("Notes", text: $notes, axis: .vertical)
            }
            .navigationTitle(arrival == nil ? "New animal arrival" : "Edit arrival")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        let group = groupName == "None" ? "" : groupName
                        if let arrival {
                            arrival.name = name.trimmingCharacters(in: .whitespaces)
                            arrival.type = type
                            arrival.origin = origin
                            arrival.eventDate = eventDate
                            arrival.registrationStatus = registrationStatus
                            arrival.registrationId = registrationId.trimmingCharacters(in: .whitespaces)
                            arrival.groupName = group
                            arrival.notes = notes.trimmingCharacters(in: .whitespaces)
                        } else {
                            context.insert(
                                AnimalArrival(
                                    name: name.trimmingCharacters(in: .whitespaces),
                                    type: type,
                                    origin: origin,
                                    eventDate: eventDate,
                                    registrationStatus: registrationStatus,
                                    registrationId: registrationId.trimmingCharacters(in: .whitespaces),
                                    groupName: group,
                                    notes: notes.trimmingCharacters(in: .whitespaces)
                                )
                            )
                        }
                        try? context.save()
                        dismiss()
                    }
                }
            }
            .onAppear {
                if let arrival {
                    name = arrival.name
                    type = arrival.type
                    origin = arrival.origin
                    eventDate = arrival.eventDate
                    registrationStatus = arrival.registrationStatus
                    registrationId = arrival.registrationId
                    groupName = arrival.groupName.isEmpty ? "None" : arrival.groupName
                    notes = arrival.notes
                }
            }
        }
    }
}
