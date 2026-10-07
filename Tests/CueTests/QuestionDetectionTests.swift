import XCTest
@testable import Cue

final class QuestionDetectionTests: XCTestCase {
    private func line(_ text: String, _ speaker: Speaker = .them) -> Line { Line(speaker: speaker, text: text, start: Date()) }

    func testSpotsQuestionsAndPrompts() {
        for text in ["What drew you to this role?", "So, tell me about a time you disagreed with a manager.", "Great. Walk me through your last project",
                     "Okay. How would you design a rate limiter", "Can you share an example of leading without authority"] {
            XCTAssertTrue(line(text).soundsLikeQuestion, text)
        }
    }
    func testIgnoresStatements() {
        for text in ["Thanks, that makes sense.", "Let me explain how the team is structured.", "We use Swift and Kotlin here", ""] {
            XCTAssertFalse(line(text).soundsLikeQuestion, text)
        }
    }
    func testTranscriptLabelsSpeakers() {
        XCTAssertEqual([line("Why Cue?"), line("I love the product.", .me)].transcript, "Interviewer: Why Cue?\nMe: I love the product.")
    }
}
