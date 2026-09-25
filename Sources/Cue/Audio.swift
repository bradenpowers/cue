import AVFoundation
import Speech
import ScreenCaptureKit

final class SystemAudioOutput: NSObject, SCStreamOutput, @unchecked Sendable {
    let request: SFSpeechAudioBufferRecognitionRequest
    init(_ request: SFSpeechAudioBufferRecognitionRequest) { self.request = request }
    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sample.isValid else { return }
        request.appendAudioSampleBuffer(sample)
    }
}

@MainActor
final class AudioCapture: NSObject, ObservableObject, SCStreamDelegate {
    @Published var running = false
    @Published var starting = false
    @Published var partial = ""
    @Published var segments: [String] = []
    @Published var error = ""
    var transcript: String { (segments + (partial.isEmpty ? [] : [partial])).joined(separator: "\n") }
    private let engine = AVAudioEngine()
    private var task: SFSpeechRecognitionTask?
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var stream: SCStream?
    private var output: SystemAudioOutput?
    private var timer: Task<Void, Never>?
    private var tapped = false
    private var generation = UUID()
    private var system = false

    func start(system: Bool) async {
        guard !running && !starting else { return }
        starting = true; error = ""; self.system = system
        defer { starting = false }
        let speech = await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { continuation.resume(returning: $0 == .authorized) }
        }
        guard speech else { error = "Allow Speech Recognition for Cue in System Settings → Privacy & Security."; return }
        if !system {
            let allowed = await AVCaptureDevice.requestAccess(for: .audio)
            guard allowed else { error = "Allow Microphone access for Cue in System Settings → Privacy & Security."; return }
        }
        do { try await begin() } catch { await stop(); self.error = error.localizedDescription }
    }

    private func begin() async throws {
        guard let recognizer = SFSpeechRecognizer(locale: Locale(identifier: "en-US")), recognizer.isAvailable else {
            throw CueError.message("English speech recognition is unavailable. Check your connection and macOS speech settings.")
        }
        let id = UUID(); generation = id
        let req = SFSpeechAudioBufferRecognitionRequest()
        req.shouldReportPartialResults = true
        if recognizer.supportsOnDeviceRecognition { req.requiresOnDeviceRecognition = true }
        request = req
        task = recognizer.recognitionTask(with: req) { [weak self] result, failure in
            let text = result?.bestTranscription.formattedString
            let final = result?.isFinal ?? false
            let message = failure?.localizedDescription
            Task { @MainActor [weak self] in
                guard let self, self.generation == id else { return }
                if let text { self.partial = text }
                if final || message != nil {
                    await self.stop()
                    if let message { self.error = "Transcription stopped: \(message). Start listening to reconnect." }
                    else { await self.start(system: self.system) }
                }
            }
        }
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
            let handler = SystemAudioOutput(req); output = handler; stream = capture
            try capture.addStreamOutput(handler, type: .audio, sampleHandlerQueue: DispatchQueue(label: "cue.audio"))
            try await capture.startCapture()
        } else {
            let node = engine.inputNode
            let format = node.outputFormat(forBus: 0)
            guard format.sampleRate > 0 && format.channelCount > 0 else { throw CueError.message("No working microphone found.") }
            node.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in req.append(buffer) }
            tapped = true; engine.prepare(); try engine.start()
        }
        running = true
        timer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(50))
            guard !Task.isCancelled, let self, self.generation == id else { return }
            await self.stop()
            await self.start(system: self.system)
        }
    }

    func stop() async {
        generation = UUID(); timer?.cancel(); timer = nil
        running = false
        engine.stop()
        if tapped { engine.inputNode.removeTap(onBus: 0); tapped = false }
        let old = stream; stream = nil
        request?.endAudio(); task?.cancel(); task = nil; request = nil
        if !partial.isEmpty { segments.append(partial); partial = "" }
        if let old { try? await old.stopCapture() }
        output = nil
    }

    nonisolated func stream(_ stream: SCStream, didStopWithError error: Error) {
        Task { @MainActor in
            guard self.stream === stream else { return }
            await self.stop(); self.error = "Mac audio capture stopped: \(error.localizedDescription)"
        }
    }
}
