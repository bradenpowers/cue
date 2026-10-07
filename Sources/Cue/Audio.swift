import AVFoundation
import Speech
import ScreenCaptureKit

enum Speaker: String { case me, them
    var label: String { self == .me ? "Me" : "Interviewer" }
}

struct Line: Identifiable, Equatable {
    var id = UUID()
    let speaker: Speaker
    let text: String
    let start: Date

    // ponytail: keyword heuristic so answers start instantly; TypeSafe catches the questions it misses.
    static let prompts = ["tell me", "walk me", "talk me through", "take me through", "describe", "explain", "give me an example", "share an example",
                          "how would you", "how do you", "how did you", "what would you", "why do you", "why did you", "can you", "could you", "what's your", "what is your"]
    var soundsLikeQuestion: Bool {
        let text = text.lowercased()
        if text.trimmingCharacters(in: .whitespaces).hasSuffix("?") { return true }
        // Check each clause so "So, tell me about…" counts but "let me explain" does not.
        return text.split(whereSeparator: { ".!,;".contains($0) }).contains { clause in
            let clause = clause.trimmingCharacters(in: .whitespaces)
            return Line.prompts.contains { clause.hasPrefix($0) }
        }
    }
}

/// Every captured line in the order it was spoken.
@MainActor func merged(_ captures: AudioCapture...) -> [Line] { captures.flatMap(\.lines).sorted { $0.start < $1.start } }
extension Array where Element == Line {
    var transcript: String { map { "\($0.speaker.label): \($0.text)" }.joined(separator: "\n") }
}

// Audio arrives on capture threads; the recognition request it feeds is swapped after each utterance.
final class RecognitionFeed: NSObject, SCStreamOutput, @unchecked Sendable {
    private let lock = NSLock()
    private var current: SFSpeechAudioBufferRecognitionRequest?
    var request: SFSpeechAudioBufferRecognitionRequest? {
        get { lock.withLock { current } }
        set { lock.withLock { current = newValue } }
    }
    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sample.isValid else { return }
        request?.appendAudioSampleBuffer(sample)
    }
}

@MainActor
final class AudioCapture: NSObject, ObservableObject, SCStreamDelegate {
    static let pause: Duration = .milliseconds(900) // calibration knob: silence that ends an utterance
    let speaker: Speaker
    @Published var running = false
    @Published var starting = false
    @Published var partial = ""
    @Published var segments: [Line] = []
    @Published var error = ""
    /// Called with each finished utterance.
    var onUtterance: ((Line) -> Void)?
    var lines: [Line] { segments + (partial.isEmpty ? [] : [Line(id: partialID, speaker: speaker, text: partial, start: partialStart)]) }
    private let engine = AVAudioEngine()
    private let feed = RecognitionFeed()
    private var recognizer: SFSpeechRecognizer?
    private var task: SFSpeechRecognitionTask?
    private var stream: SCStream?
    private var timer: Task<Void, Never>?
    private var silence: Task<Void, Never>?
    private var tapped = false
    private var recognition = UUID()
    private var partialID = UUID()
    private var partialStart = Date()

    init(speaker: Speaker) { self.speaker = speaker }

    func start(system: Bool) async {
        guard !running && !starting else { return }
        starting = true; error = ""
        defer { starting = false }
        let speech = await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { continuation.resume(returning: $0 == .authorized) }
        }
        guard speech else { error = "Allow Speech Recognition for Cue in System Settings → Privacy & Security."; return }
        if !system {
            let allowed = await AVCaptureDevice.requestAccess(for: .audio)
            guard allowed else { error = "Allow Microphone access for Cue in System Settings → Privacy & Security."; return }
        }
        do { try await begin(system: system) } catch { await stop(); self.error = error.localizedDescription }
    }

    private func begin(system: Bool) async throws {
        guard let recognizer = SFSpeechRecognizer(locale: Locale(identifier: "en-US")), recognizer.isAvailable else {
            throw CueError.message("English speech recognition is unavailable. Check your connection and macOS speech settings.")
        }
        self.recognizer = recognizer
        if system {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            guard let display = content.displays.first else { throw CueError.message("No display is available for Mac audio capture.") }
            let config = SCStreamConfiguration()
            config.capturesAudio = true; config.excludesCurrentProcessAudio = true
            config.sampleRate = 16000; config.channelCount = 1
            config.width = 2; config.height = 2
            config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
            let filter = SCContentFilter(display: display, excludingWindows: [])
            let capture = SCStream(filter: filter, configuration: config, delegate: self)
            stream = capture
            try capture.addStreamOutput(feed, type: .audio, sampleHandlerQueue: DispatchQueue(label: "cue.audio.\(speaker.rawValue)"))
            try await capture.startCapture()
        } else {
            let node = engine.inputNode
            let format = node.outputFormat(forBus: 0)
            guard format.sampleRate > 0 && format.channelCount > 0 else { throw CueError.message("No working microphone found.") }
            node.installTap(onBus: 0, bufferSize: 1024, format: format) { [feed] buffer, _ in feed.request?.append(buffer) }
            tapped = true; engine.prepare(); try engine.start()
        }
        running = true
        recognize()
    }

    // Starts a fresh recognition request; audio keeps flowing into whichever request is current.
    private func recognize() {
        guard running, let recognizer else { return }
        let id = UUID(); recognition = id
        let req = SFSpeechAudioBufferRecognitionRequest()
        req.shouldReportPartialResults = true; req.addsPunctuation = true
        if recognizer.supportsOnDeviceRecognition { req.requiresOnDeviceRecognition = true }
        let old = feed.request; feed.request = req; old?.endAudio(); task?.cancel()
        task = recognizer.recognitionTask(with: req) { [weak self] result, failure in
            let text = result?.bestTranscription.formattedString
            let final = result?.isFinal ?? false
            let message = failure?.localizedDescription
            Task { @MainActor [weak self] in
                guard let self, self.recognition == id else { return }
                if let text, !text.isEmpty {
                    if self.partial.isEmpty { self.partialID = UUID(); self.partialStart = Date() }
                    self.partial = text; self.waitForPause()
                }
                if final { self.finishUtterance() }
                else if let message { await self.stop(); self.error = "Transcription stopped: \(message). Start listening to reconnect." }
            }
        }
        // Recognition requests are limited to about a minute, so roll over before that.
        timer?.cancel()
        timer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(50))
            guard !Task.isCancelled, let self, self.recognition == id else { return }
            self.finishUtterance()
        }
    }

    private func waitForPause() {
        silence?.cancel()
        silence = Task { [weak self] in
            try? await Task.sleep(for: AudioCapture.pause)
            guard !Task.isCancelled else { return }
            self?.finishUtterance()
        }
    }

    // Commits what was heard as one utterance and starts listening for the next.
    private func finishUtterance() {
        let line = commit()
        recognize()
        if let line { onUtterance?(line) }
    }

    private func commit() -> Line? {
        silence?.cancel()
        let text = partial.trimmingCharacters(in: .whitespacesAndNewlines)
        partial = ""
        guard !text.isEmpty else { return nil }
        let line = Line(id: partialID, speaker: speaker, text: text, start: partialStart)
        segments.append(line)
        return line
    }

    func stop() async {
        recognition = UUID(); timer?.cancel(); timer = nil
        running = false
        engine.stop()
        if tapped { engine.inputNode.removeTap(onBus: 0); tapped = false }
        let old = stream; stream = nil
        feed.request?.endAudio(); feed.request = nil; task?.cancel(); task = nil
        _ = commit()
        if let old { try? await old.stopCapture() }
    }

    nonisolated func stream(_ stream: SCStream, didStopWithError error: Error) {
        Task { @MainActor in
            guard self.stream === stream else { return }
            await self.stop(); self.error = "Mac audio capture stopped: \(error.localizedDescription)"
        }
    }
}
