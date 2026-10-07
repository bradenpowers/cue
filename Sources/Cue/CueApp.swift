import SwiftUI
import AppKit
import UniformTypeIdentifiers
import Combine

@main
struct CueApp: App {
    @StateObject private var session = Session()
    var body: some Scene {
        Window("Cue", id: "main") {
            Workspace().environmentObject(session)
                .frame(minWidth: 860, minHeight: 650)
                .preferredColorScheme(.dark)
        }
        .defaultSize(width: 1120, height: 780)
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Toggle floating assistant") { session.togglePanel() }.keyboardShortcut("j", modifiers: [.command, .shift])
                Button("Suggest talking points") { session.suggest() }.keyboardShortcut(.return, modifiers: .command)
            }
        }
        MenuBarExtra("Cue", systemImage: "sparkle") {
            Button("Floating assistant") { session.togglePanel() }
            Button("Show workspace") { NSApp.activate(ignoringOtherApps: true); NSApp.windows.first { $0.title == "Cue" }?.makeKeyAndOrderFront(nil) }
            Divider()
            Button("Quit Cue") { NSApp.terminate(nil) }
        }
    }
}

@MainActor
final class Session: ObservableObject {
    @Published var question = ""
    @Published var notes = ""
    @Published var answer = ""
    @Published var busy = false
    @Published var error = ""
    @Published var setup = false
    @Published var history: [String] = []
    let them = AudioCapture(speaker: .them) // Mac audio on a call; the only microphone in person
    let me = AudioCapture(speaker: .me) // microphone on a call
    var lines: [Line] { merged(them, me) }
    private var panel: NSPanel?
    private var pending: Task<Void, Never>?
    private var run = UUID()
    private var autosave: AnyCancellable?
    private var interview = Interview()
    private let store = InterviewStore()
    private var answered = 0 // interviewer lines already covered by an automatic answer

    init() {
        them.onUtterance = { [weak self] _ in self?.heard() }
        autosave = Publishers.CombineLatest4(them.$segments, me.$segments, $history, $notes).dropFirst()
            .debounce(for: .seconds(2), scheduler: RunLoop.main).sink { [weak self] _ in self?.save() }
        NotificationCenter.default.addObserver(forName: NSApplication.willTerminateNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.save() }
        }
    }

    func startListening(call: Bool) async {
        await them.start(system: call)
        if call { await me.start(system: false) }
    }
    func stopListening() async { await them.stop(); await me.stop() }

    private func save() {
        interview.role = UserDefaults.standard.string(forKey: "role") ?? ""
        interview.transcript = lines.transcript; interview.talkingPoints = history; interview.notes = notes
        do { try store.save(interview) } catch { self.error = "Could not save this interview: \(error.localizedDescription)" }
    }

    func suggest() {
        guard !busy else { return }
        guard !question.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !lines.isEmpty else {
            error = "Start listening or type a question first."; return
        }
        start(question: question.isEmpty ? nil : question, assessment: nil)
    }

    // The interviewer finished speaking: answer right away if it was a question.
    private func heard() {
        let unanswered = them.segments.dropFirst(answered)
        guard let first = unanswered.first, let last = unanswered.last else { return }
        let latest = String(unanswered.map(\.text).joined(separator: " ").suffix(800))
        if last.soundsLikeQuestion { return answerAutomatically(latest, assessment: nil) }
        let key = (try? Credentials.read(account: TypeSafeAPI.account, label: "TypeSafe")) ?? ""
        guard !key.isEmpty, latest.split(separator: " ").count >= 3 else { return }
        let heardCount = them.segments.count
        let defaults = UserDefaults.standard
        let earlier = String(lines.filter { $0.start < first.start }.transcript.suffix(1500))
        Task {
            do {
                let assessment = try await TypeSafeAPI.assess(key: key, role: defaults.string(forKey: "role") ?? "", earlier: earlier, latest: latest,
                                                              lines: TypeSafeAPI.resumeLines(defaults.string(forKey: "resume") ?? ""), detectQuestion: true)
                // A newer line is already being checked; let that one decide.
                guard them.segments.count == heardCount, answered < heardCount,
                      assessment.asksQuestion >= TypeSafeAPI.questionThreshold, assessment.kind != .smallTalk else { return }
                answerAutomatically(latest, assessment: assessment)
            } catch is CancellationError {
            } catch { self.error = error.localizedDescription }
        }
    }

    private func answerAutomatically(_ question: String, assessment: Assessment?) {
        answered = them.segments.count
        pending?.cancel() // the newest question wins
        start(question: question, assessment: assessment)
    }

    private func start(question typed: String?, assessment: Assessment?) {
        let id = UUID(); run = id
        busy = true; error = ""
        let q = typed ?? "Help me respond to the latest interview question."
        let transcript = lines.transcript
        let defaults = UserDefaults.standard
        let configuration = LLMConfiguration.load(from: defaults)
        let resume = defaults.string(forKey: "resume") ?? ""
        let role = defaults.string(forKey: "role") ?? ""
        pending = Task {
            defer { if run == id { busy = false } }
            do {
                var assessment = assessment
                var note = ""
                let typeSafeKey = try Credentials.read(account: TypeSafeAPI.account, label: "TypeSafe")
                if assessment == nil, !typeSafeKey.isEmpty {
                    do {
                        assessment = try await TypeSafeAPI.assess(key: typeSafeKey, role: role, earlier: String(transcript.suffix(1500)), latest: typed ?? String(transcript.suffix(800)),
                                                                  lines: TypeSafeAPI.resumeLines(resume), detectQuestion: false)
                    } catch is CancellationError { return } catch { note = "\(error.localizedDescription) Used your full résumé instead." }
                }
                // Fall back to the full résumé when no line clears the relevance bar.
                let experience = assessment.map(\.evidence).flatMap { $0.isEmpty ? nil : $0.joined(separator: "\n") } ?? resume
                let result = try await AssistantAPI.suggest(configuration: configuration, key: Credentials.read(for: configuration.provider), resume: experience, role: role,
                                                            transcript: transcript, question: q, guidance: assessment?.kind.guidance)
                guard !Task.isCancelled else { return }
                answer = result; history.append("## \(q)\n\n\(result)"); error = note
            } catch {
                if !Task.isCancelled { self.error = error.localizedDescription }
            }
        }
    }
    func cancel() { pending?.cancel() }
    func clear() async {
        pending?.cancel(); await stopListening()
        save(); interview = Interview()
        question = ""; notes = ""; answer = ""; history = []; error = ""
        for capture in [them, me] { capture.segments = []; capture.error = "" }
        answered = 0
    }
    func export() {
        let panel = NSSavePanel(); panel.allowedContentTypes = [.plainText]; panel.nameFieldStringValue = "Cue session.md"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        save()
        do { try interview.markdown.write(to: url, atomically: true, encoding: .utf8) } catch { self.error = error.localizedDescription }
    }
    func togglePanel() {
        if let panel { if panel.isVisible { panel.orderOut(nil) } else { panel.makeKeyAndOrderFront(nil) }; return }
        let p = NSPanel(contentRect: NSRect(x: 160, y: 180, width: 390, height: 440), styleMask: [.titled, .closable, .resizable, .utilityWindow], backing: .buffered, defer: false)
        p.title = "Cue · Live assistant"; p.level = .floating; p.isReleasedWhenClosed = false
        p.hidesOnDeactivate = false; p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.contentView = NSHostingView(rootView: FloatingView().environmentObject(self).preferredColorScheme(.dark))
        panel = p; p.makeKeyAndOrderFront(nil)
    }
}

let mint = Color(red: 0.65, green: 0.88, blue: 0.73)
let surface = Color(red: 0.105, green: 0.12, blue: 0.13)

struct Workspace: View {
    @EnvironmentObject var session: Session
    @State private var clearConfirmation = false
    @State private var reviewing = false
    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 28) {
                HStack(spacing: 10) {
                    Image(systemName: "asterisk").font(.system(size: 30, weight: .bold)).foregroundStyle(mint)
                    Text("Cue").font(.system(size: 26, weight: .semibold, design: .rounded))
                }.padding(.top, 14)
                VStack(alignment: .leading, spacing: 10) {
                    Text("YOUR WORKSPACE").font(.system(size: 10, weight: .bold)).tracking(2).foregroundStyle(.secondary)
                    Label("Live interview", systemImage: "waveform").font(.system(size: 13, weight: .semibold)).padding(12).frame(maxWidth: .infinity, alignment: .leading).background(mint.opacity(0.1), in: RoundedRectangle(cornerRadius: 10)).foregroundStyle(mint)
                    Button { session.setup = true } label: { Label("Context & setup", systemImage: "slider.horizontal.3").padding(12) }.buttonStyle(.plain)
                    Button { reviewing = true } label: { Label("Past interviews", systemImage: "clock.arrow.circlepath").padding(12) }.buttonStyle(.plain)
                }
                Spacer()
                VStack(alignment: .leading, spacing: 10) {
                    Image(systemName: "rectangle.on.rectangle").foregroundStyle(mint)
                    Text("A little room to think.").font(.system(size: 14, weight: .medium))
                    Text("Keep your talking points nearby with the floating assistant.").font(.system(size: 12)).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    Button("Open assistant ↗") { session.togglePanel() }.buttonStyle(.plain).foregroundStyle(mint).padding(.top, 5)
                    Text("⌘ ⇧ J  while Cue is active").font(.system(size: 10)).foregroundStyle(.tertiary)
                }.padding(16).background(.white.opacity(0.035), in: RoundedRectangle(cornerRadius: 12))
                Text("BUILT FOR YOUR NEXT CHAPTER").font(.system(size: 8, weight: .medium)).tracking(1.3).foregroundStyle(.tertiary)
            }.padding(22).frame(width: 210).background(Color(red: 0.065, green: 0.078, blue: 0.082))
            VStack(alignment: .leading, spacing: 22) {
                HStack {
                    VStack(alignment: .leading, spacing: 7) {
                        Text("LIVE WORKSPACE").font(.system(size: 10, weight: .bold)).tracking(2).foregroundStyle(mint)
                        Text("Stay in the conversation.").font(.system(size: 28, weight: .medium))
                        Text("Your experience. A clearer way to share it.").font(.system(size: 13)).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button { session.export() } label: { Image(systemName: "square.and.arrow.up") }.help("Export session")
                    Button { clearConfirmation = true } label: { Image(systemName: "plus") }.help("New session")
                }
                CaptureControls(them: session.them, me: session.me)
                if !session.error.isEmpty { Text(session.error).font(.callout).foregroundStyle(.orange).textSelection(.enabled) }
                HStack(alignment: .top, spacing: 16) {
                    TranscriptView(them: session.them, me: session.me).frame(maxWidth: .infinity, maxHeight: .infinity)
                    VStack(alignment: .leading, spacing: 14) {
                        HStack {
                            Label("Talking points", systemImage: "sparkles").font(.system(size: 14, weight: .semibold))
                            Spacer()
                            if !session.answer.isEmpty {
                                Button { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(session.answer, forType: .string) } label: { Image(systemName: "doc.on.doc") }.buttonStyle(.plain).help("Copy talking points")
                            }
                        }
                        ScrollView {
                            if session.answer.isEmpty {
                                VStack(alignment: .leading, spacing: 16) {
                                    Image(systemName: "sparkle").font(.system(size: 32)).foregroundStyle(mint).padding(.top, 28)
                                    Text("Find your next thought.").font(.system(size: 21, weight: .medium))
                                    Text("When the interviewer asks a question, Cue writes a concise starting point grounded in your experience.").foregroundStyle(.secondary).lineSpacing(5)
                                    Text("01   Add your experience in Setup\n\n02   Start listening\n\n03   Talking points appear when you’re asked a question").font(.system(size: 12)).foregroundStyle(.secondary).padding(.top, 12)
                                }.frame(maxWidth: .infinity, alignment: .leading)
                            } else { Text(session.answer).textSelection(.enabled).lineSpacing(6).frame(maxWidth: .infinity, alignment: .leading) }
                        }
                        Divider()
                        TextField("Ask a question, or use the transcript…", text: $session.question, axis: .vertical).lineLimit(2...4).textFieldStyle(.plain)
                        HStack {
                            Button { session.suggest() } label: {
                                HStack { if session.busy { ProgressView().controlSize(.small) }; Text(session.busy ? "Thinking…" : "Suggest talking points"); Spacer(); Text("⌘ ↵").opacity(0.6) }.padding(7)
                            }.buttonStyle(.borderedProminent).tint(mint).foregroundStyle(.black).disabled(session.busy)
                            if session.busy { Button("Cancel") { session.cancel() } }
                        }
                    }.padding(20).background(surface, in: RoundedRectangle(cornerRadius: 16)).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
                VStack(alignment: .leading, spacing: 10) {
                    Label("Your notes", systemImage: "square.and.pencil").font(.system(size: 12, weight: .semibold)).foregroundStyle(.secondary)
                    TextEditor(text: $session.notes).font(.system(size: 13)).scrollContentBackground(.hidden).frame(height: 58).overlay(alignment: .topLeading) {
                        if session.notes.isEmpty { Text("Names, questions to ask, things to come back to…").font(.system(size: 13)).foregroundStyle(.tertiary).padding(.top, 1).padding(.leading, 5).allowsHitTesting(false) }
                    }
                }.padding(16).background(surface, in: RoundedRectangle(cornerRadius: 12))
                HStack { Circle().fill(mint).frame(width: 5, height: 5); Text("Interviews save automatically on this Mac."); Spacer(); Text("macOS · Cue") }.font(.system(size: 10)).foregroundStyle(.secondary)
            }.padding(28).background(Color(red: 0.075, green: 0.088, blue: 0.095))
        }.sheet(isPresented: $session.setup) { SetupView() }
            .sheet(isPresented: $reviewing) { InterviewsView() }
            .confirmationDialog("Start a new session? The current interview is saved to Past interviews.", isPresented: $clearConfirmation) {
                Button("Clear and start new", role: .destructive) { Task { await session.clear() } }
            }
    }
}

struct CaptureControls: View {
    @EnvironmentObject var session: Session
    @ObservedObject var them: AudioCapture
    @ObservedObject var me: AudioCapture
    @State private var call = true
    var body: some View {
        let running = them.running || me.running, starting = them.starting || me.starting
        VStack(alignment: .leading, spacing: 9) {
            HStack(spacing: 14) {
                Image(systemName: running ? "waveform" : "mic").foregroundStyle(mint)
                VStack(alignment: .leading, spacing: 3) {
                    Text(running ? "Listening live" : "Ready when you are").font(.system(size: 13, weight: .semibold))
                    Text(call ? "Video call · your mic is you, Mac audio is the interviewer" : "In person · the microphone hears everyone as the interviewer").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                Spacer()
                Picker("Source", selection: $call) { Text("Video call").tag(true); Text("In person").tag(false) }.labelsHidden().frame(width: 125).disabled(running || starting)
                Button(starting ? "Connecting…" : running ? "Stop listening" : "Start listening") {
                    Task { if running { await session.stopListening() } else { await session.startListening(call: call) } }
                }.disabled(starting).buttonStyle(.bordered).tint(mint)
            }
            ForEach([them.error, me.error].filter { !$0.isEmpty }, id: \.self) { Text($0).font(.caption).foregroundStyle(.orange).textSelection(.enabled) }
        }.padding(16).background(surface, in: RoundedRectangle(cornerRadius: 12))
    }
}

struct TranscriptView: View {
    @ObservedObject var them: AudioCapture
    @ObservedObject var me: AudioCapture
    var body: some View {
        let running = them.running || me.running, lines = merged(them, me)
        VStack(alignment: .leading, spacing: 16) {
            HStack { Label("Live transcript", systemImage: "text.alignleft").font(.system(size: 14, weight: .semibold)); Spacer(); Text(running ? "LIVE" : "IDLE").font(.system(size: 9, weight: .bold)).tracking(1).foregroundStyle(running ? mint : .secondary) }
            ScrollViewReader { proxy in
                ScrollView {
                    if lines.isEmpty {
                        VStack(alignment: .leading, spacing: 12) {
                            Text("Every question,\na little more space.").font(.system(size: 22, weight: .medium)).padding(.top, 28)
                            Text("Your conversation will appear here when you start listening.").foregroundStyle(.secondary).lineSpacing(5)
                            Text("Mac audio requires Screen & System Audio Recording permission. Use headphones on calls so your mic hears only you. Only audio is processed. English transcription may use Apple’s service when on-device recognition is unavailable.").font(.system(size: 11)).foregroundStyle(.tertiary).lineSpacing(4).padding(.top, 12)
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        VStack(alignment: .leading, spacing: 12) {
                            ForEach(lines) { line in
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(line.speaker.label.uppercased()).font(.system(size: 9, weight: .bold)).tracking(1).foregroundStyle(line.speaker == .them ? mint : .secondary)
                                    Text(line.text).textSelection(.enabled).lineSpacing(5)
                                }.frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                    Color.clear.frame(height: 1).id("bottom")
                }.onChange(of: them.partial + me.partial) { _, _ in proxy.scrollTo("bottom", anchor: .bottom) }
            }
        }.padding(20).background(surface, in: RoundedRectangle(cornerRadius: 16))
    }
}

struct FloatingView: View {
    @EnvironmentObject var session: Session
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Label("A little help, right here.", systemImage: "sparkles").foregroundStyle(mint)
            ScrollView { Text(session.answer.isEmpty ? "Talking points from your live workspace will appear here." : session.answer).textSelection(.enabled).lineSpacing(5).frame(maxWidth: .infinity, alignment: .leading) }
            if !session.error.isEmpty { Text(session.error).font(.caption).foregroundStyle(.orange) }
            TextField("Question or follow-up…", text: $session.question).textFieldStyle(.roundedBorder)
            Button(session.busy ? "Thinking…" : "Suggest talking points") { session.suggest() }.buttonStyle(.borderedProminent).tint(mint).foregroundStyle(.black).disabled(session.busy)
        }.padding(20).frame(minWidth: 310, minHeight: 300).background(surface)
    }
}

struct SetupView: View {
    @Environment(\.dismiss) var dismiss
    @AppStorage("resume") private var resume = ""
    @AppStorage("role") private var role = ""
    @State private var choosingResume = false
    @State private var importingResume = false
    @State private var importedResume: ImportedResume?
    @State private var importError = ""
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack { Text("Make Cue yours.").font(.system(size: 25, weight: .medium)); Spacer(); Button("Done") { dismiss() }.disabled(importingResume) }
                Text("Give your assistant the context behind your answers.").foregroundStyle(.secondary)
                HStack {
                    Text("YOUR EXPERIENCE").font(.caption.bold()).foregroundStyle(mint)
                    Spacer()
                    if importingResume { ProgressView().controlSize(.small); Text("Reading résumé…").font(.caption) }
                    Button { choosingResume = true } label: { Label("Import résumé", systemImage: "square.and.arrow.down") }.disabled(importingResume)
                }
                if !importError.isEmpty { Text(importError).font(.caption).foregroundStyle(.orange).fixedSize(horizontal: false, vertical: true) }
                TextEditor(text: $resume).font(.body).frame(height: 130).border(.white.opacity(0.1))
                Text("Import PDF, DOCX, RTF, or text, or paste your experience here. Saved automatically and used for talking points.").font(.caption).foregroundStyle(.secondary)
                Text("ROLE & COMPANY").font(.caption.bold()).foregroundStyle(mint)
                TextEditor(text: $role).font(.body).frame(height: 90).border(.white.opacity(0.1))
                Divider()
                ProviderSettingsView()
                Divider()
                TypeSafeSettingsView()
            }.padding(28)
        }.frame(width: 600, height: 700)
            .interactiveDismissDisabled(importingResume)
            .fileImporter(isPresented: $choosingResume, allowedContentTypes: ResumeImporter.allowedTypes) { result in
                switch result {
                case .success(let url):
                    importingResume = true; importError = ""
                    Task {
                        defer { importingResume = false }
                        do { importedResume = try await Task.detached(priority: .userInitiated) { try ResumeImporter.extract(from: url) }.value }
                        catch { importError = error.localizedDescription }
                    }
                case .failure(let error): importError = error.localizedDescription
                }
            }
            .sheet(item: $importedResume) { imported in
                ResumePreview(imported: imported, hasExperience: !resume.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty) { text, append in
                    resume = append ? resume.trimmingCharacters(in: .whitespacesAndNewlines) + "\n\n" + text : text
                    importedResume = nil
                }
            }
    }
}

struct ResumePreview: View {
    @Environment(\.dismiss) var dismiss
    let imported: ImportedResume
    let hasExperience: Bool
    let save: (String, Bool) -> Void
    @State private var text: String

    init(imported: ImportedResume, hasExperience: Bool, save: @escaping (String, Bool) -> Void) {
        self.imported = imported; self.hasExperience = hasExperience; self.save = save
        _text = State(initialValue: imported.text)
    }
    private var cleaned: String { text.trimmingCharacters(in: .whitespacesAndNewlines) }
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Review your résumé").font(.title2.bold())
            Label(imported.filename, systemImage: "doc.text").foregroundStyle(mint).lineLimit(1).truncationMode(.middle)
            Text("Check the reading order and edit anything that needs fixing. This text becomes context for future talking points.").font(.callout).foregroundStyle(.secondary)
            TextEditor(text: $text).font(.body).frame(minHeight: 260).border(.white.opacity(0.1)).accessibilityLabel("Extracted résumé text")
            Text("Extracted locally · \(text.count.formatted()) characters · Original file unchanged").font(.caption).foregroundStyle(.secondary)
            if hasExperience { Text("Replace overwrites your saved experience. Add keeps your existing experience and appends this text.").font(.caption).foregroundStyle(.secondary) }
            HStack {
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Spacer()
                if hasExperience { Button("Add to experience") { save(cleaned, true) }.disabled(cleaned.isEmpty) }
                Button(hasExperience ? "Replace experience" : "Use résumé") { save(cleaned, false) }.buttonStyle(.borderedProminent).tint(mint).foregroundStyle(.black).disabled(cleaned.isEmpty)
            }
        }.padding(24).frame(width: 600).frame(minHeight: 580)
    }
}
