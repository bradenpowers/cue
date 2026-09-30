import XCTest
@testable import Cue

final class TypeSafeTests: XCTestCase {
    let lines = ["Led the payments migration to Kubernetes across 40 services", "Mentored four junior engineers through promotion", "Built the company softball league schedule app"]

    func testRequestAsksOneScorePerLineAndOptionalDetection() throws {
        let req = try TypeSafeAPI.makeRequest(key: " ts-key ", role: "Staff engineer", earlier: "", latest: "Tell me about a migration you led.", lines: lines, detectQuestion: true)
        XCTAssertEqual(req.url?.absoluteString, "https://api.typesafe.ai/v1/systemone")
        XCTAssertEqual(req.value(forHTTPHeaderField: "Authorization"), "Bearer ts-key")
        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: XCTUnwrap(req.httpBody)) as? [String: Any])
        let questions = try XCTUnwrap(body["questions"] as? [String: [String: Any]])
        XCTAssertEqual(Set(questions.keys), ["question_type", "asks_question", "line_0", "line_1", "line_2"])
        XCTAssertEqual(questions["line_2"]?["type"] as? String, "score")
        XCTAssertEqual((body["state"] as? [String: Any])?["resume_lines"] as? [String], lines)
        let manual = try TypeSafeAPI.makeRequest(key: "k", role: "", earlier: "", latest: "q", lines: [], detectQuestion: false)
        let manualQuestions = try XCTUnwrap((JSONSerialization.jsonObject(with: XCTUnwrap(manual.httpBody)) as? [String: Any])?["questions"] as? [String: Any])
        XCTAssertEqual(Set(manualQuestions.keys), ["question_type"])
        XCTAssertThrowsError(try TypeSafeAPI.makeRequest(key: "  ", role: "", earlier: "", latest: "q", lines: [], detectQuestion: false))
    }

    func testParseKeepsRelevantLinesInResumeOrderAndGatesKind() throws {
        let json = #"{"model":"jev-1.13.0","answers":{"asks_question":{"type":"noul","noul":0.93},"question_type":{"type":"choice","choice":"behavioral","probabilities":{"behavioral":0.8,"experience":0.2},"confidence":0.7},"line_0":{"type":"score","score":2.9},"line_1":{"type":"score","score":1.6},"line_2":{"type":"score","score":0.1}}}"#
        let a = try TypeSafeAPI.parse(Data(json.utf8), statusCode: 200, lines: lines)
        XCTAssertEqual(a, Assessment(asksQuestion: 0.93, kind: .behavioral, evidence: [lines[0], lines[1]]))

        let unsure = #"{"answers":{"question_type":{"type":"choice","choice":"technical","probabilities":{"technical":0.4,"experience":0.35,"other":0.25}}}}"#
        XCTAssertEqual(try TypeSafeAPI.parse(Data(unsure.utf8), statusCode: 200, lines: lines), Assessment(asksQuestion: 0, kind: .other, evidence: []))
    }

    func testErrorsRedactKey() {
        XCTAssertThrowsError(try TypeSafeAPI.parse(Data(#"{"detail":"bad key sk-123"}"#.utf8), statusCode: 401, lines: [], key: "sk-123")) {
            XCTAssertEqual($0.localizedDescription, "TypeSafe: bad key [redacted]")
        }
    }

    func testResumeLinesDropShortFragments() {
        XCTAssertEqual(TypeSafeAPI.resumeLines("ACME\n  Led the payments migration to Kubernetes  \n\n2020–2023"), ["Led the payments migration to Kubernetes"])
    }
}
