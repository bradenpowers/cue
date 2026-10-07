import XCTest
@testable import Cue

final class ProviderTests: XCTestCase {
    private func request(_ provider: LLMProvider, model: String = "example-model", key: String = "test-credential", base: String = "http://localhost:11434/v1") throws -> URLRequest {
        try AssistantAPI.makeRequest(configuration: LLMConfiguration(provider: provider, model: model, localBaseURL: base), key: key, instructions: "Coach instructions", input: "Candidate context")
    }
    private func body(_ request: URLRequest) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: XCTUnwrap(request.httpBody)) as? [String: Any])
    }
    func testOpenAIPayloadAndCredentials() throws {
        let req = try request(.openai)
        XCTAssertEqual(req.url?.absoluteString, "https://api.openai.com/v1/responses")
        XCTAssertEqual(req.value(forHTTPHeaderField: "Authorization"), "Bearer test-credential")
        let json = try body(req)
        XCTAssertEqual(json["store"] as? Bool, false)
        XCTAssertEqual(json["input"] as? String, "Candidate context")
        XCTAssertEqual(json["instructions"] as? String, "Coach instructions")
        XCTAssertNil(req.value(forHTTPHeaderField: "x-api-key"))
    }
    func testClaudeUsesMessagesAndSeparateSystemPrompt() throws {
        let req = try request(.claude)
        XCTAssertEqual(req.url?.host, "api.anthropic.com")
        XCTAssertEqual(req.value(forHTTPHeaderField: "x-api-key"), "test-credential")
        XCTAssertEqual(req.value(forHTTPHeaderField: "anthropic-version"), "2023-06-01")
        XCTAssertNil(req.value(forHTTPHeaderField: "Authorization"))
        let json = try body(req)
        XCTAssertEqual(json["system"] as? String, "Coach instructions")
        XCTAssertEqual(json["max_tokens"] as? Int, 1024)
        XCTAssertEqual((json["messages"] as? [[String: String]])?.first?["content"], "Candidate context")
    }
    func testGeminiUsesHeaderAndModelPath() throws {
        let req = try request(.gemini, model: "models/gemini-3.8-flash")
        XCTAssertEqual(req.url?.path, "/v1beta/models/gemini-3.8-flash:generateContent")
        XCTAssertNil(req.url?.query)
        XCTAssertEqual(req.value(forHTTPHeaderField: "x-goog-api-key"), "test-credential")
        let json = try body(req)
        XCTAssertNotNil(json["systemInstruction"])
        let contents = try XCTUnwrap(json["contents"] as? [[String: Any]])
        XCTAssertEqual((contents.first?["parts"] as? [[String: String]])?.first?["text"], "Candidate context")
        XCTAssertThrowsError(try request(.gemini, model: "model?key=wrong"))
    }
    func testLocalServerAllowsOptionalKeyAndTrailingSlash() throws {
        for base in ["http://localhost:11434/v1", "http://127.0.0.1:1234/v1/", "http://[::1]:1234/v1"] {
            let req = try request(.local, key: "", base: base)
            XCTAssertEqual(req.url?.path, "/v1/chat/completions")
            XCTAssertNil(req.value(forHTTPHeaderField: "Authorization"))
            let json = try body(req)
            XCTAssertEqual(json["stream"] as? Bool, false)
            XCTAssertEqual((json["messages"] as? [[String: String]])?.count, 2)
        }
        XCTAssertEqual(try request(.local).value(forHTTPHeaderField: "Authorization"), "Bearer test-credential")
    }
    func testValidation() throws {
        for provider in [LLMProvider.openai, .claude, .gemini] { XCTAssertThrowsError(try request(provider, key: "  ")) }
        for provider in LLMProvider.allCases { XCTAssertThrowsError(try request(provider, model: "  ")) }
        for base in ["ftp://localhost/v1", "https://remote.example/v1", "http://localhost.evil.example/v1", "http://user:secret@localhost/v1", "http://localhost/v1?key=secret", "not a URL"] {
            XCTAssertThrowsError(try request(.local, base: base))
        }
    }
    func testProviderResponseFormatsExcludeThinkingAndAlternativeCandidates() throws {
        let fixtures: [(LLMProvider, String)] = [
            (.openai, #"{"output":[{"type":"reasoning"},{"content":[{"type":"output_text","text":"Answer"}]}]}"#),
            (.claude, #"{"content":[{"type":"thinking","thinking":"private"},{"type":"text","text":"Answer"}]}"#),
            (.gemini, #"{"candidates":[{"content":{"parts":[{"thought":true,"text":"private"},{"text":"Answer"}]}},{"content":{"parts":[{"text":"Alternative"}]}}]}"#),
            (.local, #"{"choices":[{"message":{"content":"Answer","reasoning_content":"private"}},{"message":{"content":"Alternative"}}]}"#)
        ]
        for (provider, json) in fixtures {
            XCTAssertEqual(try AssistantAPI.parse(Data(json.utf8), statusCode: 200, provider: provider), "Answer")
        }
    }
    func testErrorsAndBlockedResponses() throws {
        for provider in LLMProvider.allCases {
            XCTAssertThrowsError(try AssistantAPI.parse(Data(#"{"error":{"message":"Invalid test-credential"}}"#.utf8), statusCode: 401, provider: provider, key: "test-credential")) { error in
                XCTAssertFalse(error.localizedDescription.contains("test-credential"))
                XCTAssertTrue(error.localizedDescription.contains("[redacted]"))
            }
            XCTAssertThrowsError(try AssistantAPI.parse(Data("{}".utf8), statusCode: 200, provider: provider))
            XCTAssertThrowsError(try AssistantAPI.parse(Data("<html>Bad gateway</html>".utf8), statusCode: 502, provider: provider))
        }
        XCTAssertThrowsError(try AssistantAPI.parse(Data(#"{"promptFeedback":{"blockReason":"SAFETY"}}"#.utf8), statusCode: 200, provider: .gemini))
    }
    func testSettingsPreserveExistingOpenAIModelAndProviderModels() throws {
        let name = "CueTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        defaults.set("my-existing-model", forKey: "model")
        XCTAssertEqual(LLMConfiguration.load(from: defaults).provider, .openai)
        XCTAssertEqual(LLMConfiguration.load(from: defaults).model, "my-existing-model")
        defaults.set("claude", forKey: "llm.provider")
        defaults.set("my-claude-model", forKey: LLMProvider.claude.modelKey)
        XCTAssertEqual(LLMConfiguration.load(from: defaults).model, "my-claude-model")
        defaults.set("openai", forKey: "llm.provider")
        XCTAssertEqual(LLMConfiguration.load(from: defaults).model, "my-existing-model")
    }
    func testCredentialsStoredInAppFile() throws {
        let original = Credentials.file
        Credentials.file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).appendingPathComponent("keys.json")
        defer { try? FileManager.default.removeItem(at: Credentials.file.deletingLastPathComponent()); Credentials.file = original }
        XCTAssertEqual(try Credentials.read(for: .openai), "")
        try Credentials.save("sk-openai", for: .openai)
        try Credentials.save("ts-key", account: "typesafe")
        XCTAssertEqual(try Credentials.read(for: .openai), "sk-openai")
        XCTAssertEqual(try Credentials.read(for: .claude), "")
        XCTAssertEqual(try Credentials.read(account: "typesafe", label: "TypeSafe"), "ts-key")
        XCTAssertEqual(try FileManager.default.attributesOfItem(atPath: Credentials.file.path)[.posixPermissions] as? Int, 0o600)
        try Credentials.save("", for: .openai)
        XCTAssertEqual(try Credentials.read(for: .openai), "")
        XCTAssertEqual(try Credentials.read(account: "typesafe", label: "TypeSafe"), "ts-key")
    }
}
