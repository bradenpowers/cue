import SwiftUI

struct Interview: Codable, Identifiable, Hashable {
    var id = UUID()
    var date = Date()
    var role = ""
    var transcript = ""
    var talkingPoints: [String] = []
    var notes = ""
    var isEmpty: Bool { [transcript, notes].allSatisfy { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty } && talkingPoints.isEmpty }
    var title: String { role.split(whereSeparator: \.isNewline).first.map(String.init) ?? "Interview" }
    var markdown: String { "# Cue session\n\n\(date.formatted())\n\n## Transcript\n\n\(transcript)\n\n## Talking points\n\n\(talkingPoints.joined(separator: "\n\n"))\n\n## Notes\n\n\(notes)" }
}

// One JSON file per interview, stored only on this Mac.
struct InterviewStore {
    var directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Cue/Interviews", isDirectory: true)

    func save(_ interview: Interview) throws {
        guard !interview.isEmpty else { return try delete(interview) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder(); encoder.dateEncodingStrategy = .iso8601
        try encoder.encode(interview).write(to: url(interview), options: .atomic)
    }
    func load() -> [Interview] {
        let decoder = JSONDecoder(); decoder.dateDecodingStrategy = .iso8601
        let files = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        return files.filter { $0.pathExtension == "json" }.compactMap { try? decoder.decode(Interview.self, from: Data(contentsOf: $0)) }.sorted { $0.date > $1.date }
    }
    func delete(_ interview: Interview) throws {
        do { try FileManager.default.removeItem(at: url(interview)) } catch CocoaError.fileNoSuchFile {}
    }
    private func url(_ interview: Interview) -> URL { directory.appendingPathComponent("\(interview.id.uuidString).json") }
}

struct InterviewsView: View {
    @Environment(\.dismiss) var dismiss
    @State private var interviews = InterviewStore().load()
    @State private var selection: Interview?
    @State private var error = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack { Text("Past interviews").font(.system(size: 22, weight: .medium)); Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.cancelAction) }
            if !error.isEmpty { Text(error).font(.caption).foregroundStyle(.orange) }
            if interviews.isEmpty {
                Text("Interviews save automatically as you go. They'll appear here.").foregroundStyle(.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                HStack(spacing: 0) {
                    List(interviews, selection: $selection) { interview in
                        VStack(alignment: .leading, spacing: 3) {
                            Text(interview.title).lineLimit(1)
                            Text(interview.date.formatted(date: .abbreviated, time: .shortened)).font(.caption).foregroundStyle(.secondary)
                        }.tag(interview).contextMenu { Button("Delete", role: .destructive) { delete(interview) } }
                    }.frame(width: 220)
                    Divider()
                    ScrollView {
                        if let selection {
                            VStack(alignment: .leading, spacing: 10) {
                                HStack {
                                    Spacer()
                                    Button { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(selection.markdown, forType: .string) } label: { Image(systemName: "doc.on.doc") }.help("Copy interview")
                                    Button(role: .destructive) { delete(selection) } label: { Image(systemName: "trash") }.help("Delete interview")
                                }
                                Text(selection.markdown).textSelection(.enabled).lineSpacing(5).frame(maxWidth: .infinity, alignment: .leading)
                            }.padding(16)
                        } else { Text("Select an interview to review it.").foregroundStyle(.secondary).padding(.top, 40) }
                    }.frame(maxWidth: .infinity)
                }.background(surface, in: RoundedRectangle(cornerRadius: 12))
            }
        }.padding(24).frame(width: 820, height: 600)
    }
    private func delete(_ interview: Interview) {
        do { try InterviewStore().delete(interview); interviews.removeAll { $0.id == interview.id }; if selection == interview { selection = nil } }
        catch { self.error = error.localizedDescription }
    }
}
