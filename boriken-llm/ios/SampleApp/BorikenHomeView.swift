import SwiftUI
import BorikenKit

/// Drop this view into an iOS app target that depends on BorikenKit.
public struct BorikenHomeView: View {
    @State private var query = "our land"
    @State private var resultText = "Taí wey — welcome to Borikén."
    @State private var lessonTitle = ""
    @State private var isLoading = false
    private let client = BorikenClient(baseURL: URL(string: "http://127.0.0.1:8080")!)

    public init() {}

    public var body: some View {
        NavigationStack {
            ZStack {
                LinearGradient(
                    colors: [
                        Color(red: 0.05, green: 0.28, blue: 0.22),
                        Color(red: 0.12, green: 0.45, blue: 0.38),
                        Color(red: 0.95, green: 0.78, blue: 0.35)
                    ],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .ignoresSafeArea()

                VStack(alignment: .leading, spacing: 20) {
                    Text("BORIKÉN")
                        .font(.custom("Georgia", size: 44).weight(.bold))
                        .foregroundStyle(.white)
                        .tracking(2)

                    Text("Rebuild the language of the native land.")
                        .font(.custom("Georgia", size: 18))
                        .foregroundStyle(.white.opacity(0.9))

                    HStack {
                        TextField("English or Spanish", text: $query)
                            .textFieldStyle(.roundedBorder)
                        Button("Translate") { Task { await translate() } }
                            .buttonStyle(.borderedProminent)
                            .tint(Color(red: 0.85, green: 0.55, blue: 0.15))
                    }

                    Text(resultText)
                        .font(.custom("Georgia", size: 20))
                        .foregroundStyle(.white)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding()
                        .background(.black.opacity(0.2))

                    Button("Daily lesson") { Task { await loadLesson() } }
                        .buttonStyle(.bordered)
                        .tint(.white)

                    if !lessonTitle.isEmpty {
                        Text(lessonTitle)
                            .foregroundStyle(.white.opacity(0.95))
                    }

                    if isLoading {
                        ProgressView().tint(.white)
                    }

                    Spacer()
                }
                .padding(24)
            }
            .navigationBarHidden(true)
        }
    }

    @MainActor
    private func translate() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let result = try await client.translate(query)
            resultText = "\(result.boriken)\n\(result.english)\n[\(result.attestation) · \(result.confidence)]"
        } catch {
            resultText = "API unavailable. Start BorikenLLM server on :8080."
        }
    }

    @MainActor
    private func loadLesson() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let lesson = try await client.lesson()
            lessonTitle = "\(lesson.title): \(lesson.practice.prompt)"
        } catch {
            lessonTitle = "Could not load lesson."
        }
    }
}

#Preview {
    BorikenHomeView()
}
