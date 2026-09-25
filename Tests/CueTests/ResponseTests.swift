import XCTest
@testable import Cue
final class ResponseTests: XCTestCase {
    func testExtractsTextAfterReasoningAndAcrossMessages() throws {
        let json = #"{"status":"completed","output":[{"type":"reasoning"},{"content":[{"type":"output_text","text":"First"},{"type":"refusal","text":"Ignore"}]},{"content":[{"type":"output_text","text":"Second"}]}]}"#
        let response = try JSONDecoder().decode(AIResponse.self, from: Data(json.utf8))
        XCTAssertEqual(response.text, "First\nSecond")
    }
    func testEmptyResponseHasNoFabricatedText() throws {
        let response = try JSONDecoder().decode(AIResponse.self, from: Data(#"{"status":"failed"}"#.utf8))
        XCTAssertEqual(response.text, "")
    }
}
