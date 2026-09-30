import Foundation
import SwiftUI

enum QuestionKind: String, CaseIterable {
    case behavioral, technical, experience, logistics, smallTalk = "small_talk", other
    var rubric: String {
        switch self {
        case .behavioral: return "Asks how the candidate handled a past situation, such as 'Tell me about a time…', a conflict, a failure, or leading others"
        case .technical: return "Asks the candidate to solve, design, explain, or reason through a technical or domain problem"
        case .experience: return "Asks about the candidate's background, a specific job or project, their skills, or why they want this role"
        case .logistics: return "Asks about salary, availability, start date, location, work authorization, or the interview process"
        case .smallTalk: return "Greetings, pleasantries, or casual chat that needs no prepared answer"
        case .other: return "Any other question, or no question is being asked"
        }
    }
    var guidance: String? {
        switch self {
        case .behavioral: return "This is a behavioral question: outline one real example as situation, task, action, and result."
        case .technical: return "This is a technical question: lead with the approach, then name the key trade-offs."
        case .experience: return "This is a background question: connect the most relevant experience directly to the role."
        case .logistics: return "This is a logistics question: suggest a plain one- or two-sentence reply and never invent numbers or dates."
        case .smallTalk: return "This is small talk: suggest a short, friendly reply."
        case .other: return nil
        }
    }
}

struct Assessment: Equatable {
    var asksQuestion = 0.0
    var kind = QuestionKind.other
    /// The most relevant résumé lines, in résumé order. Empty means none cleared the bar.
    var evidence: [String] = []
}

enum TypeSafeAPI {
    static let account = "typesafe"
    static let model = "jev-latest"
    // Calibration knobs: tune against real interviews.
    static let questionThreshold = 0.7 // P(interviewer just asked something) before auto-suggesting
    static let kindThreshold = 0.5 // below this, send no question-type guidance
    static let evidenceThreshold = 1.5 // 0 unrelated … 3 direct example
    static let maxEvidence = 8
    static let relevanceLevels = ["Unrelated to the question", "Loosely related background", "Relevant supporting evidence", "A direct, strong example for this answer"]
    private static let session = URLSession(configuration: .ephemeral, delegate: NoRedirectDelegate(), delegateQueue: nil)

    // ponytail: single lines lose their employer heading; score whole sections if answers need that context.
    static func resumeLines(_ resume: String) -> [String] {
        Array(resume.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.filter { $0.count >= 25 }.prefix(80))
    }

    static func makeRequest(key: String, role: String, earlier: String, latest: String, lines: [String], detectQuestion: Bool) throws -> URLRequest {
        let key = key.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else { throw CueError.message("Add your TypeSafe API key in Setup.") }
        var questions: [String: Any] = [
            "question_type": ["type": "choice", "instructions": "What kind of interview question is the most recent question in `latest_speech`?",
                              "criteria": Dictionary(uniqueKeysWithValues: QuestionKind.allCases.map { ($0.rawValue, $0.rubric) })],
        ]
        if detectQuestion {
            questions["asks_question"] = ["type": "noul", "instructions": "Does `latest_speech` end with the interviewer asking the candidate a question, or making a request, that the candidate should answer now?",
                                          "criteria": ["true": "The final sentences pose a complete question or request to the candidate, such as 'Tell me about a time…' or 'How would you design…'",
                                                       "false": "It ends with a statement, an unfinished question, the candidate's own answer, or a quick check that needs no prepared answer, such as 'Can you hear me?'"]]
        }
        for i in lines.indices {
            questions["line_\(i)"] = ["type": "score", "instructions": "How useful is `resume_lines[\(i)]` as evidence for the candidate's answer to the most recent question in `latest_speech`?",
                                      "criteria": relevanceLevels]
        }
        let state: [String: Any] = ["role": String(role.prefix(4000)), "earlier_conversation": earlier, "latest_speech": latest, "resume_lines": lines]
        var request = URLRequest(url: URL(string: "https://api.typesafe.ai/v1/systemone")!)
        request.httpMethod = "POST"; request.timeoutInterval = 30
        request.setValue("Bearer \(key)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["model": model, "state": state, "questions": questions])
        return request
    }

    static func parse(_ data: Data, statusCode: Int, lines: [String], key: String = "") throws -> Assessment {
        let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        guard (200..<300).contains(statusCode), let answers = object?["answers"] as? [String: [String: Any]] else {
            var detail = (object?["error"] as? [String: Any])?["message"] as? String ?? object?["error"] as? String ?? object?["detail"] as? String
                ?? (statusCode == 429 || statusCode == 529 ? "Busy right now; try again shortly." : "Request failed (HTTP \(statusCode)). Check your key.")
            let secret = key.trimmingCharacters(in: .whitespacesAndNewlines)
            if !secret.isEmpty { detail = detail.replacingOccurrences(of: secret, with: "[redacted]") }
            throw CueError.message("TypeSafe: \(detail)")
        }
        let type = answers["question_type"]
        let top = (type?["choice"] as? String).flatMap(QuestionKind.init) ?? .other
        let topProbability = (type?["probabilities"] as? [String: Double])?[top.rawValue] ?? 0
        let scored = lines.indices.compactMap { i in (answers["line_\(i)"]?["score"] as? Double).map { (i, $0) } }
        let evidence = scored.filter { $0.1 >= evidenceThreshold }.sorted { $0.1 > $1.1 }.prefix(maxEvidence).map(\.0).sorted().map { lines[$0] }
        return Assessment(asksQuestion: answers["asks_question"]?["noul"] as? Double ?? 0,
                          kind: topProbability >= kindThreshold ? top : .other, evidence: evidence)
    }

    static func assess(key: String, role: String, earlier: String, latest: String, lines: [String], detectQuestion: Bool) async throws -> Assessment {
        let request = try makeRequest(key: key, role: role, earlier: earlier, latest: latest, lines: lines, detectQuestion: detectQuestion)
        do {
            let (data, response) = try await session.data(for: request)
            return try parse(data, statusCode: (response as? HTTPURLResponse)?.statusCode ?? 0, lines: lines, key: key)
        } catch let error as URLError {
            if error.code == .cancelled { throw CancellationError() }
            throw CueError.message("Could not reach TypeSafe. Check your internet connection.")
        }
    }
}

struct TypeSafeSettingsView: View {
    @AppStorage("typesafe.auto") private var auto = false
    @State private var key = ""
    @State private var message = ""
    @State private var messageIsError = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("TYPESAFE (OPTIONAL)").font(.caption.bold()).foregroundStyle(mint)
            HStack {
                SecureField("TypeSafe API key", text: $key)
                Button("Save key") {
                    do {
                        let value = key.trimmingCharacters(in: .whitespacesAndNewlines)
                        try Credentials.save(value, account: TypeSafeAPI.account)
                        key = value; messageIsError = false
                        message = value.isEmpty ? "TypeSafe key removed." : "TypeSafe key saved in macOS Keychain."
                    } catch { message = error.localizedDescription; messageIsError = true }
                }
            }
            Toggle("Suggest talking points automatically when the interviewer asks a question", isOn: $auto)
            if !message.isEmpty { Text(message).font(.caption).foregroundStyle(messageIsError ? .orange : mint) }
            Text("With a key, Cue asks TypeSafe what kind of question was asked and which résumé lines fit it, so your AI provider receives only the relevant experience. This sends your role, résumé lines, and recent transcript to TypeSafe. Automatic mode checks each pause in the conversation.")
                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
        .onAppear {
            do { key = try Credentials.read(account: TypeSafeAPI.account, label: "TypeSafe") }
            catch { message = error.localizedDescription; messageIsError = true }
        }
    }
}
