// BorikenKit — Swift client for the BorikenLLM API (iOS)
import Foundation

public struct BorikenLexeme: Codable, Sendable, Identifiable {
    public let id: String
    public let boriken: String
    public let english: String
    public let spanish: String
    public let pos: String
    public let attested: Bool
    public let confidence: String
    public let tags: [String]
    public let etymology: String?
    public let source: String?
    public let definition_en: String?
    public let definition_es: String?
    public let fun_fact: String?
    public let example: String?
}

public struct BorikenResult: Codable, Sendable {
    public let boriken: String
    public let english: String
    public let spanish: String
    public let confidence: String
    public let attestation: String
    public let morphology: [String]
    public let notes: String
    public let definition_en: String?
    public let definition_es: String?
    public let fun_fact: String?
    public let example: String?
}

public struct TranslateResponse: Codable, Sendable {
    public let ok: Bool
    public let result: BorikenResult
}

public struct ChatResponse: Codable, Sendable {
    public let reply_boriken: String
    public let reply_english: String
    public let reply_spanish: String
    public let fun_fact: String?
}

public struct LessonResponse: Codable, Sendable {
    public let skill: String
    public let title: String
    public let explanation: String
    public let examples: [LessonExample]
    public let practice: PracticePrompt
    public let cheer: String?
    public let fun_hook: String?
}

public struct LessonExample: Codable, Sendable {
    public let boriken: String
    public let english: String
    public let spanish: String
}

public struct PracticePrompt: Codable, Sendable {
    public let prompt: String
    public let answer: String
    public let xp: Int?
}

public struct HealthResponse: Codable, Sendable {
    public let status: String
    public let language: String
    public let lexicon_size: Int
    public let version: String
}

public struct WordOfDayResponse: Codable, Sendable {
    public let date: String
    public let title: String
    public let cheer: String
    public let word: BorikenLexeme
}

public struct MatchGameResponse: Codable, Sendable {
    public let mode: String
    public let title: String
    public let instructions: String
    public let left: [MatchItem]
    public let right: [String]
    public let answer_key: [String: String]
    public let xp_reward: Int
    public let fun_tip: String
}

public struct MatchItem: Codable, Sendable, Identifiable {
    public var id: String
    public let text: String
}

public struct GradeResponse: Codable, Sendable {
    public let correct: Bool
    public let expected: String
    public let answer: String
    public let xp_earned: Int
    public let message: String
}

public struct ProgressResponse: Codable, Sendable {
    public let xp: Int
    public let streak: Int
    public let level: Int
    public let title: String
    public let next_level_xp: Int
    public let tip: String
}

public enum BorikenAPIError: Error, Sendable {
    case invalidURL
    case badResponse(Int)
    case decoding
}

public actor BorikenClient {
    public let baseURL: URL
    private let session: URLSession

    public init(baseURL: URL = URL(string: "http://127.0.0.1:8080")!, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.session = session
    }

    public func health() async throws -> HealthResponse {
        try await get("/health")
    }

    public func translate(_ text: String, sourceLang: String = "auto") async throws -> BorikenResult {
        let body: [String: String] = ["text": text, "source_lang": sourceLang]
        let response: TranslateResponse = try await post("/v1/translate", body: body)
        return response.result
    }

    public func define(_ term: String) async throws -> BorikenLexeme {
        struct Envelope: Codable {
            let found: Bool
            let spotlight: BorikenLexeme?
        }
        let encoded = term.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? term
        let envelope: Envelope = try await get("/v1/define/\(encoded)")
        guard let spot = envelope.spotlight else { throw BorikenAPIError.badResponse(404) }
        return spot
    }

    public func reconstruct(concept: String) async throws -> BorikenResult {
        struct Envelope: Codable { let ok: Bool; let result: BorikenResult }
        let envelope: Envelope = try await post("/v1/reconstruct", body: ["concept": concept])
        return envelope.result
    }

    public func chat(_ message: String) async throws -> ChatResponse {
        try await post("/v1/chat", body: ["message": message])
    }

    public func lesson(skill: String? = nil) async throws -> LessonResponse {
        var body: [String: String] = [:]
        if let skill { body["skill"] = skill }
        return try await post("/v1/lesson", body: body)
    }

    public func wordOfTheDay() async throws -> WordOfDayResponse {
        try await get("/v1/fun/word-of-the-day")
    }

    public func matchGame() async throws -> MatchGameResponse {
        try await get("/v1/fun/match")
    }

    public func grade(answer: String, expected: String, mode: String = "match") async throws -> GradeResponse {
        let url = baseURL.appendingPathComponent("v1/fun/grade")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: [
            "mode": mode,
            "answer": answer,
            "expected": expected
        ])
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            throw BorikenAPIError.badResponse((response as? HTTPURLResponse)?.statusCode ?? -1)
        }
        return try JSONDecoder().decode(GradeResponse.self, from: data)
    }

    public func progress(xp: Int, streak: Int) async throws -> ProgressResponse {
        let url = baseURL.appendingPathComponent("v1/fun/progress")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["xp": xp, "streak": streak])
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            throw BorikenAPIError.badResponse((response as? HTTPURLResponse)?.statusCode ?? -1)
        }
        return try JSONDecoder().decode(ProgressResponse.self, from: data)
    }

    public func lookup(_ query: String) async throws -> [BorikenLexeme] {
        struct Envelope: Codable { let count: Int; let items: [BorikenLexeme] }
        let encoded = query.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? query
        let envelope: Envelope = try await get("/v1/lexicon?q=\(encoded)")
        return envelope.items
    }

    private func get<T: Decodable>(_ path: String) async throws -> T {
        guard let url = URL(string: path, relativeTo: baseURL)?.absoluteURL else {
            throw BorikenAPIError.invalidURL
        }
        let (data, response) = try await session.data(from: url)
        guard let http = response as? HTTPURLResponse else { throw BorikenAPIError.badResponse(-1) }
        guard (200..<300).contains(http.statusCode) else { throw BorikenAPIError.badResponse(http.statusCode) }
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw BorikenAPIError.decoding
        }
    }

    private func post<T: Decodable>(_ path: String, body: [String: String]) async throws -> T {
        guard let url = URL(string: path, relativeTo: baseURL)?.absoluteURL else {
            throw BorikenAPIError.invalidURL
        }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(body)
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw BorikenAPIError.badResponse(-1) }
        guard (200..<300).contains(http.statusCode) else { throw BorikenAPIError.badResponse(http.statusCode) }
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw BorikenAPIError.decoding
        }
    }
}
