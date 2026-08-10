import SwiftUI
import BorikenKit

/// Fun learning home for the Borikén iOS app.
public struct BorikenHomeView: View {
    @State private var query = "hurricane"
    @State private var headline = "BORIKÉN"
    @State private var subtitle = "Learn the native land's language like an areyto."
    @State private var resultText = "Taí wey — tap Play to start today's island run."
    @State private var funFact = "Coquí chorus loading…"
    @State private var xp = 0
    @State private var streak = 1
    @State private var levelTitle = "Konuko Seedling"
    @State private var matchPrompt: String?
    @State private var matchChoices: [String] = []
    @State private var matchAnswer = ""
    @State private var isLoading = false
    @State private var pulse = false

    private let client = BorikenClient(baseURL: URL(string: "http://127.0.0.1:8080")!)
    private let gold = Color(red: 0.95, green: 0.78, blue: 0.35)
    private let lagoon = Color(red: 0.08, green: 0.42, blue: 0.36)

    public init() {}

    public var body: some View {
        NavigationStack {
            ZStack {
                LinearGradient(
                    colors: [
                        Color(red: 0.03, green: 0.22, blue: 0.18),
                        lagoon,
                        Color(red: 0.18, green: 0.55, blue: 0.42),
                        gold.opacity(0.85)
                    ],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .ignoresSafeArea()
                .hueRotation(.degrees(pulse ? 12 : 0))
                .animation(.easeInOut(duration: 3).repeatForever(autoreverses: true), value: pulse)

                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(headline)
                                    .font(.custom("Georgia", size: 42).weight(.bold))
                                    .foregroundStyle(.white)
                                    .tracking(3)
                                    .scaleEffect(pulse ? 1.02 : 1.0)
                                Text(subtitle)
                                    .font(.custom("Georgia", size: 16))
                                    .foregroundStyle(.white.opacity(0.92))
                            }
                            Spacer()
                            VStack(alignment: .trailing) {
                                Text("\(xp) XP")
                                    .font(.custom("Georgia", size: 18).weight(.bold))
                                    .foregroundStyle(gold)
                                Text(levelTitle)
                                    .font(.caption)
                                    .foregroundStyle(.white.opacity(0.85))
                                Text("streak \(streak)🔥")
                                    .font(.caption2)
                                    .foregroundStyle(.white.opacity(0.8))
                            }
                        }

                        panel {
                            Text(resultText)
                                .font(.custom("Georgia", size: 20))
                                .foregroundStyle(.white)
                            if !funFact.isEmpty {
                                Text("✦ \(funFact)")
                                    .font(.custom("Georgia", size: 14))
                                    .foregroundStyle(gold)
                                    .padding(.top, 6)
                            }
                        }

                        HStack(spacing: 10) {
                            TextField("Ask / translate", text: $query)
                                .textFieldStyle(.roundedBorder)
                            Button("Define") { Task { await defineWord() } }
                                .buttonStyle(.borderedProminent)
                                .tint(gold)
                                .foregroundStyle(.black)
                        }

                        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                            playButton("Word of Day", system: "sun.max.fill") { await loadWordOfDay() }
                            playButton("Batey Match", system: "square.grid.2x2.fill") { await startMatch() }
                            playButton("Mini Lesson", system: "book.fill") { await loadLesson() }
                            playButton("Daily Run", system: "flag.fill") { await loadDaily() }
                        }

                        if let matchPrompt, !matchChoices.isEmpty {
                            panel {
                                Text("Batey Match")
                                    .font(.headline)
                                    .foregroundStyle(gold)
                                Text(matchPrompt)
                                    .font(.custom("Georgia", size: 22).weight(.semibold))
                                    .foregroundStyle(.white)
                                ForEach(matchChoices, id: \.self) { choice in
                                    Button(choice) { Task { await submitMatch(choice) } }
                                        .buttonStyle(.bordered)
                                        .tint(.white)
                                        .frame(maxWidth: .infinity)
                                }
                            }
                        }

                        if isLoading {
                            ProgressView().tint(.white)
                        }
                    }
                    .padding(22)
                }
            }
            .navigationBarHidden(true)
            .onAppear {
                pulse = true
                Task { await refreshProgress() }
            }
        }
    }

    @ViewBuilder
    private func panel<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            content()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.black.opacity(0.22))
        .overlay(
            RoundedRectangle(cornerRadius: 0)
                .stroke(gold.opacity(0.35), lineWidth: 1)
        )
    }

    private func playButton(_ title: String, system: String, action: @escaping () async -> Void) -> some View {
        Button {
            Task { await action() }
        } label: {
            Label(title, systemImage: system)
                .font(.subheadline.weight(.semibold))
                .frame(maxWidth: .infinity, minHeight: 44)
        }
        .buttonStyle(.borderedProminent)
        .tint(Color.white.opacity(0.18))
        .foregroundStyle(.white)
    }

    @MainActor
    private func defineWord() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let word = try await client.define(query)
            resultText = "\(word.boriken)\n\(word.definition_en ?? word.english)"
            funFact = word.fun_fact ?? ""
            xp += 5
            await refreshProgress()
        } catch {
            resultText = "API offline — start BorikenLLM on :8080."
            funFact = ""
        }
    }

    @MainActor
    private func loadWordOfDay() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let w = try await client.wordOfTheDay()
            resultText = "\(w.word.boriken)\n\(w.word.definition_en ?? w.word.english)\nExample: \(w.word.example ?? w.word.boriken)"
            funFact = w.word.fun_fact ?? w.cheer
            xp += 15
            await refreshProgress()
        } catch {
            resultText = "Could not load Word of the Day."
        }
    }

    @MainActor
    private func startMatch() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let game = try await client.matchGame()
            guard let first = game.left.first else { return }
            matchPrompt = first.text
            matchAnswer = game.answer_key[first.text] ?? ""
            matchChoices = Array(game.right.shuffled().prefix(4))
            if !matchChoices.contains(matchAnswer), !matchAnswer.isEmpty {
                matchChoices[0] = matchAnswer
                matchChoices.shuffle()
            }
            funFact = game.fun_tip
            resultText = game.instructions
        } catch {
            resultText = "Match game unavailable."
        }
    }

    @MainActor
    private func submitMatch(_ choice: String) async {
        do {
            let grade = try await client.grade(answer: choice, expected: matchAnswer)
            resultText = grade.message + (grade.correct ? " ✓" : " — answer: \(grade.expected)")
            xp += grade.xp_earned
            if grade.correct { streak += 0 } // streak managed daily in production
            matchPrompt = nil
            matchChoices = []
            await refreshProgress()
        } catch {
            resultText = "Could not grade answer."
        }
    }

    @MainActor
    private func loadLesson() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let lesson = try await client.lesson()
            resultText = "\(lesson.title)\n\(lesson.practice.prompt)\nAnswer: \(lesson.practice.answer)"
            funFact = lesson.fun_hook ?? lesson.cheer ?? ""
            xp += lesson.practice.xp ?? 12
            await refreshProgress()
        } catch {
            resultText = "Lesson unavailable."
        }
    }

    @MainActor
    private func loadDaily() async {
        await loadWordOfDay()
        funFact = "Daily Island Run step 1 complete — next: Batey Match."
        streak = max(streak, 1)
    }

    @MainActor
    private func refreshProgress() async {
        do {
            let p = try await client.progress(xp: xp, streak: streak)
            levelTitle = p.title
        } catch {
            levelTitle = "Konuko Seedling"
        }
    }
}

#Preview {
    BorikenHomeView()
}
