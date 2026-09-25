import AppKit
import PDFKit
import UniformTypeIdentifiers

struct ImportedResume: Identifiable, Sendable {
    let id = UUID()
    let filename: String
    var text: String
}

enum ResumeImporter {
    static let allowedTypes: [UTType] = [.pdf, .rtf, .plainText, UTType(filenameExtension: "docx")!]
    static let maximumBytes = 10 * 1024 * 1024
    static let maximumCharacters = 100_000

    static func extract(from url: URL) throws -> ImportedResume {
        let access = url.startAccessingSecurityScopedResource()
        defer { if access { url.stopAccessingSecurityScopedResource() } }
        let ext = url.pathExtension.lowercased()
        guard ["pdf", "docx", "rtf", "txt", "text", "md"].contains(ext) else {
            throw CueError.message("Choose a PDF, Word (.docx), RTF, or plain-text résumé.")
        }
        let values = try url.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
        guard values.isRegularFile == true else { throw CueError.message("Choose a résumé file, not a folder.") }
        guard (values.fileSize ?? 0) <= maximumBytes else { throw CueError.message("This file is too large. Choose a résumé smaller than 10 MB.") }
        let data = try Data(contentsOf: url)
        guard data.count <= maximumBytes else { throw CueError.message("This file is too large. Choose a résumé smaller than 10 MB.") }
        let raw: String
        switch ext {
        case "pdf":
            guard let pdf = PDFDocument(data: data) else { throw CueError.message("This PDF could not be read. Try exporting it again.") }
            guard !pdf.isLocked else { throw CueError.message("This PDF is password-protected. Export an unlocked copy, then import it.") }
            raw = pdf.string ?? ""
        case "docx", "rtf":
            let type: NSAttributedString.DocumentType = ext == "docx" ? .officeOpenXML : .rtf
            do { raw = try NSAttributedString(data: data, options: [.documentType: type], documentAttributes: nil).string }
            catch { throw CueError.message("This document could not be read. Try exporting it as PDF or plain text.") }
        default:
            if let text = String(data: data, encoding: .utf8) { raw = text }
            else if data.starts(with: [0xFF, 0xFE]) || data.starts(with: [0xFE, 0xFF]), let text = String(data: data, encoding: .utf16) { raw = text }
            else { throw CueError.message("This text file uses an unsupported encoding. Save it as UTF-8 and try again.") }
        }
        let text = raw.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n").replacingOccurrences(of: "\u{0}", with: "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            throw CueError.message(ext == "pdf" ? "No readable text was found. This may be a scanned PDF. Export a searchable PDF or import a Word or text version." : "No readable text was found in this document.")
        }
        guard text.count <= maximumCharacters else { throw CueError.message("This document has too much text for a résumé. Import a shorter version (under 100,000 characters).") }
        return ImportedResume(filename: url.lastPathComponent, text: text)
    }
}
