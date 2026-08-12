import SwiftUI
import SwiftData
import UniformTypeIdentifiers

struct DataImportView: View {
    let farmName: String
    @Environment(\.modelContext) private var context
    @Query(sort: \FarmImportFile.importedAt, order: .reverse) private var files: [FarmImportFile]
    @Query(sort: \ApiFeedSource.name) private var sources: [ApiFeedSource]

    @State private var showImporter = false
    @State private var showApiForm = false
    @State private var editingSource: ApiFeedSource?
    @State private var previewText = ""
    @State private var showPreview = false
    @State private var statusMessage = ""
    @State private var draftName = ""
    @State private var draftUrl = ""
    @State private var draftMethod: ApiHttpMethod = .get
    @State private var draftAuth = ""
    @State private var draftNotes = ""
    @State private var isPulling = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                ScreenHeader(
                    brand: farmName,
                    title: "Upload files & pull site APIs",
                    subtitle: "Import KMZ / KML / CSV / JSON maps and wire external data feeds."
                )

                sectionCard(title: "Farm files") {
                    Text("Upload pasture boundaries, field maps, herd spreadsheets, and other farm documents.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Button {
                        showImporter = true
                    } label: {
                        Label("Upload KMZ or other file", systemImage: "square.and.arrow.up")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(FarmTheme.forest)
                }

                if files.isEmpty {
                    Text("No files imported yet.")
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 16)
                } else {
                    ForEach(files) { file in
                        card {
                            HStack(alignment: .top) {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(file.displayName).font(.headline)
                                    Text("\(file.kind.displayName) · \(byteLabel(file.byteSize)) · \(file.importedAt.mediumString)")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                    if !file.summary.isEmpty {
                                        Text(file.summary)
                                            .font(.subheadline)
                                            .foregroundStyle(.secondary)
                                    }
                                }
                                Spacer()
                                Button(role: .destructive) {
                                    deleteFile(file)
                                } label: {
                                    Image(systemName: "trash")
                                }
                            }
                        }
                        .padding(.horizontal, 16)
                    }
                }

                sectionCard(title: "External API feeds") {
                    Text("Add HTTPS endpoints to pull weather, markets, registries, or your own farm services.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Button {
                        editingSource = nil
                        draftName = ""
                        draftUrl = ""
                        draftMethod = .get
                        draftAuth = ""
                        draftNotes = ""
                        showApiForm = true
                    } label: {
                        Label("Add API source", systemImage: "cloud")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(FarmTheme.forest)
                }

                if sources.isEmpty {
                    Text("No API sources yet.")
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 16)
                } else {
                    ForEach(sources) { source in
                        card {
                            VStack(alignment: .leading, spacing: 8) {
                                HStack {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(source.name).font(.headline)
                                        Text("\(source.method.displayName) · \(source.baseUrl)")
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                            .lineLimit(2)
                                    }
                                    Spacer()
                                    Toggle("", isOn: Binding(
                                        get: { source.enabled },
                                        set: { source.enabled = $0; try? context.save() }
                                    ))
                                    .labelsHidden()
                                }
                                if !source.lastStatus.isEmpty {
                                    Text(source.lastStatus)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                HStack {
                                    Button("Pull now") { Task { await pull(source) } }
                                        .disabled(!source.enabled || isPulling)
                                    Button("Edit") {
                                        editingSource = source
                                        draftName = source.name
                                        draftUrl = source.baseUrl
                                        draftMethod = source.method
                                        draftAuth = source.authHeader
                                        draftNotes = source.notes
                                        showApiForm = true
                                    }
                                    Button("Preview") {
                                        previewText = source.lastPreview.isEmpty ? "(no preview)" : source.lastPreview
                                        showPreview = true
                                    }
                                    .disabled(source.lastPreview.isEmpty)
                                    Button("Delete", role: .destructive) {
                                        context.delete(source)
                                        try? context.save()
                                    }
                                }
                                .buttonStyle(.bordered)
                            }
                        }
                        .padding(.horizontal, 16)
                    }
                }

                if !statusMessage.isEmpty {
                    Text(statusMessage)
                        .font(.footnote)
                        .foregroundStyle(FarmTheme.forest)
                        .padding(.horizontal, 16)
                        .padding(.bottom, 24)
                }
            }
        }
        .background(
            LinearGradient(colors: [FarmTheme.cream, FarmTheme.mist, FarmTheme.cream], startPoint: .top, endPoint: .bottom)
                .ignoresSafeArea()
        )
        .navigationTitle("Data & maps")
        .navigationBarTitleDisplayMode(.inline)
        .fileImporter(
            isPresented: $showImporter,
            allowedContentTypes: [
                UTType(filenameExtension: "kmz") ?? .data,
                UTType(filenameExtension: "kml") ?? .xml,
                .json,
                .commaSeparatedText,
                .plainText,
                .image,
                .data
            ],
            allowsMultipleSelection: false
        ) { result in
            switch result {
            case .success(let urls):
                if let url = urls.first { importFile(from: url) }
            case .failure(let error):
                statusMessage = "Import failed: \(error.localizedDescription)"
            }
        }
        .sheet(isPresented: $showApiForm) {
            NavigationStack {
                Form {
                    TextField("Name", text: $draftName)
                    TextField("API URL", text: $draftUrl)
                        .textInputAutocapitalization(.never)
                        .keyboardType(.URL)
                    Picker("Method", selection: $draftMethod) {
                        ForEach(ApiHttpMethod.allCases) { method in
                            Text(method.displayName).tag(method)
                        }
                    }
                    TextField("Auth header (optional)", text: $draftAuth)
                        .textInputAutocapitalization(.never)
                    TextField("Notes", text: $draftNotes, axis: .vertical)
                }
                .navigationTitle(editingSource == nil ? "Add API source" : "Edit API source")
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { showApiForm = false }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Save") {
                            saveApiSource()
                            showApiForm = false
                        }
                        .disabled(draftName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                  || !(draftUrl.hasPrefix("http://") || draftUrl.hasPrefix("https://")))
                    }
                }
            }
        }
        .alert("API preview", isPresented: $showPreview) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(previewText)
        }
    }

    @ViewBuilder
    private func sectionCard<Content: View>(title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline).foregroundStyle(FarmTheme.forest)
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(Color.white)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .padding(.horizontal, 16)
    }

    private func card<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        content()
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background(Color.white)
            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    private func byteLabel(_ bytes: Int64) -> String {
        if bytes < 1024 { return "\(bytes) B" }
        let kb = Double(bytes) / 1024.0
        if kb < 1024 { return String(format: "%.1f KB", kb) }
        return String(format: "%.2f MB", kb / 1024.0)
    }

    private func importFile(from url: URL) {
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        do {
            let data = try Data(contentsOf: url)
            let name = url.lastPathComponent
            let kind = classify(name: name)
            let folder = try FileManager.default.url(
                for: .documentDirectory,
                in: .userDomainMask,
                appropriateFor: nil,
                create: true
            ).appendingPathComponent("imports", isDirectory: true)
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let dest = folder.appendingPathComponent("\(Int(Date().timeIntervalSince1970))_\(name)")
            try data.write(to: dest)
            let summary: String
            switch kind {
            case .kmz, .kml:
                summary = summarizeGeo(data: data, kind: kind)
            case .csv:
                summary = "CSV spreadsheet stored for later import mapping."
            case .json, .geojson:
                summary = "JSON document stored for later mapping."
            case .image:
                summary = "Image stored on device."
            case .other:
                summary = "File stored on device."
            }
            let row = FarmImportFile(
                displayName: name,
                kind: kind,
                mimeType: "",
                storedPath: dest.path,
                byteSize: Int64(data.count),
                summary: summary
            )
            context.insert(row)
            try context.save()
            statusMessage = "Imported \(name)"
        } catch {
            statusMessage = "Import failed: \(error.localizedDescription)"
        }
    }

    private func classify(name: String) -> FarmFileKind {
        let lower = name.lowercased()
        if lower.hasSuffix(".kmz") { return .kmz }
        if lower.hasSuffix(".kml") { return .kml }
        if lower.hasSuffix(".geojson") { return .geojson }
        if lower.hasSuffix(".json") { return .json }
        if lower.hasSuffix(".csv") { return .csv }
        if lower.hasSuffix(".png") || lower.hasSuffix(".jpg") || lower.hasSuffix(".jpeg") || lower.hasSuffix(".webp") {
            return .image
        }
        return .other
    }

    private func summarizeGeo(data: Data, kind: FarmFileKind) -> String {
        let text: String
        if kind == .kmz {
            // Lightweight fallback: store note; full unzip can be added later.
            text = String(data: data, encoding: .utf8) ?? ""
            if text.isEmpty {
                return "KMZ stored on device."
            }
        } else {
            text = String(data: data, encoding: .utf8) ?? ""
        }
        let placemarks = text.components(separatedBy: "<Placemark").count - 1
        let folders = text.components(separatedBy: "<Folder").count - 1
        return "\(max(placemarks, 0)) placemark(s), \(max(folders, 0)) folder(s)"
    }

    private func deleteFile(_ file: FarmImportFile) {
        try? FileManager.default.removeItem(atPath: file.storedPath)
        context.delete(file)
        try? context.save()
    }

    private func saveApiSource() {
        let name = draftName.trimmingCharacters(in: .whitespacesAndNewlines)
        let url = draftUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        if let existing = editingSource {
            existing.name = name
            existing.baseUrl = url
            existing.method = draftMethod
            existing.authHeader = draftAuth.trimmingCharacters(in: .whitespacesAndNewlines)
            existing.notes = draftNotes.trimmingCharacters(in: .whitespacesAndNewlines)
        } else {
            context.insert(
                ApiFeedSource(
                    name: name,
                    baseUrl: url,
                    method: draftMethod,
                    authHeader: draftAuth.trimmingCharacters(in: .whitespacesAndNewlines),
                    notes: draftNotes.trimmingCharacters(in: .whitespacesAndNewlines)
                )
            )
        }
        try? context.save()
    }

    private func pull(_ source: ApiFeedSource) async {
        guard let url = URL(string: source.baseUrl) else {
            statusMessage = "Invalid URL"
            return
        }
        isPulling = true
        defer { isPulling = false }
        var request = URLRequest(url: url, timeoutInterval: 20)
        request.httpMethod = source.method.displayName
        request.setValue("application/json, text/plain, */*", forHTTPHeaderField: "Accept")
        request.setValue("GutFarms-FarmManager-iOS/1.3", forHTTPHeaderField: "User-Agent")
        let auth = source.authHeader.trimmingCharacters(in: .whitespacesAndNewlines)
        if !auth.isEmpty {
            if let idx = auth.firstIndex(of: ":") {
                let key = String(auth[..<idx]).trimmingCharacters(in: .whitespaces)
                let value = String(auth[auth.index(after: idx)...]).trimmingCharacters(in: .whitespaces)
                request.setValue(value, forHTTPHeaderField: key)
            } else {
                request.setValue(auth, forHTTPHeaderField: "Authorization")
            }
        }
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            let code = (response as? HTTPURLResponse)?.statusCode ?? 0
            let preview = String(data: data, encoding: .utf8)?
                .prefix(4000)
                .description ?? "(binary response)"
            await MainActor.run {
                source.lastStatus = (200...299).contains(code) ? "HTTP \(code) OK" : "HTTP \(code)"
                source.lastPulledAt = .now
                source.lastPreview = preview
                try? context.save()
                statusMessage = "\(source.name): \(source.lastStatus)"
            }
        } catch {
            await MainActor.run {
                source.lastStatus = "Error: \(error.localizedDescription)"
                source.lastPulledAt = .now
                try? context.save()
                statusMessage = "\(source.name): \(source.lastStatus)"
            }
        }
    }
}
