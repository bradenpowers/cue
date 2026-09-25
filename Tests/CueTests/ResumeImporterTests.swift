import XCTest
import AppKit
import CoreText
import PDFKit
@testable import Cue

final class ResumeImporterTests: XCTestCase {
    private var directory: URL!
    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }
    override func tearDownWithError() throws { try FileManager.default.removeItem(at: directory) }
    private func file(_ name: String, data: Data) throws -> URL {
        let url = directory.appendingPathComponent(name)
        try data.write(to: url)
        return url
    }
    func testPlainTextPreservesExperienceAndNormalizesLineEndings() throws {
        let url = try file("Resume.TXT", data: Data("  Jane Doe\r\nEngineer — shipped 3 products\rSwift  \n".utf8))
        let result = try ResumeImporter.extract(from: url)
        XCTAssertEqual(result.filename, "Resume.TXT")
        XCTAssertEqual(result.text, "Jane Doe\nEngineer — shipped 3 products\nSwift")
    }
    func testWordAndRTFExtraction() throws {
        let source = NSAttributedString(string: "Jane Doe\nEngineer at Example\nReduced latency by 20%")
        for (ext, type) in [("docx", NSAttributedString.DocumentType.officeOpenXML), ("rtf", .rtf)] {
            let data = try source.data(from: NSRange(location: 0, length: source.length), documentAttributes: [.documentType: type])
            let result = try ResumeImporter.extract(from: file("resume.\(ext)", data: data))
            XCTAssertTrue(result.text.contains("Engineer at Example"))
            XCTAssertTrue(result.text.contains("Reduced latency by 20%"))
        }
    }
    private func pdf(withText: Bool) throws -> Data {
        let data = NSMutableData()
        var box = CGRect(x: 0, y: 0, width: 612, height: 792)
        let consumer = try XCTUnwrap(CGDataConsumer(data: data))
        let context = try XCTUnwrap(CGContext(consumer: consumer, mediaBox: &box, nil))
        context.beginPDFPage(nil)
        if withText {
            let text = NSAttributedString(string: "Jane Doe - Software Engineer", attributes: [.font: CTFontCreateWithName("Helvetica" as CFString, 14, nil)])
            context.textPosition = CGPoint(x: 40, y: 720)
            CTLineDraw(CTLineCreateWithAttributedString(text), context)
        }
        context.endPDFPage(); context.closePDF()
        return data as Data
    }
    func testPDFExtraction() throws {
        let result = try ResumeImporter.extract(from: file("resume.pdf", data: pdf(withText: true)))
        XCTAssertTrue(result.text.contains("Jane Doe - Software Engineer"))
    }
    func testRejectsEmptyScannedAndMalformedDocuments() throws {
        for (name, data) in [("empty.txt", Data(" \n ".utf8)), ("scan.pdf", try pdf(withText: false)), ("broken.pdf", Data("not a PDF".utf8)), ("broken.docx", Data("not a Word document".utf8))] {
            XCTAssertThrowsError(try ResumeImporter.extract(from: file(name, data: data)), name)
        }
    }
    func testRejectsOversizedAndUnsupportedFiles() throws {
        XCTAssertThrowsError(try ResumeImporter.extract(from: file("large.txt", data: Data(repeating: 65, count: ResumeImporter.maximumBytes + 1))))
        XCTAssertThrowsError(try ResumeImporter.extract(from: file("resume.html", data: Data("<p>Resume</p>".utf8))))
    }
    func testReadsUTF16() throws {
        let data = try XCTUnwrap("Résumé — Jane Doe".data(using: .utf16))
        XCTAssertEqual(try ResumeImporter.extract(from: file("resume.txt", data: data)).text, "Résumé — Jane Doe")
    }
}
