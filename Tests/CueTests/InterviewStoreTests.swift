import XCTest
@testable import Cue

final class InterviewStoreTests: XCTestCase {
    func testSaveLoadAndDelete() throws {
        let store = InterviewStore(directory: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        defer { try? FileManager.default.removeItem(at: store.directory) }
        XCTAssertEqual(store.load(), [])
        let older = Interview(date: Date(timeIntervalSince1970: 1_000), role: "Staff Engineer\nAcme", transcript: "Tell me about yourself.", talkingPoints: ["## Q\n\nA"], notes: "Ask about team")
        let newer = Interview(date: Date(timeIntervalSince1970: 2_000), notes: "Follow up")
        try store.save(older); try store.save(newer)
        XCTAssertEqual(store.load(), [newer, older])
        XCTAssertEqual(store.load().last?.title, "Staff Engineer")
        // Saving an interview that became empty removes it rather than keeping a blank entry.
        try store.save(Interview(id: newer.id))
        XCTAssertEqual(store.load(), [older])
        try store.delete(older); try store.delete(older)
        XCTAssertEqual(store.load(), [])
    }
}
