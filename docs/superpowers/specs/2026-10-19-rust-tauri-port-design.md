# Cue: Rust + Tauri port

Date: 2026-10-19 · Branch: `rust-tauri-port`

## Goal

Rewrite Cue as a Rust + Tauri app that runs on **macOS and Windows**, with feature parity with the current Swift app. When the Rust app is accepted, the Swift app is archived (git tag) and deleted; only the Rust app remains.

## Decisions

| Topic | Decision |
| --- | --- |
| Platforms | macOS 14+ and Windows 10/11. Linux is out of scope. |
| Shell | Tauri 2 |
| Frontend | React + TypeScript, built with Vite |
| Speech-to-text, macOS | Apple Speech (`SFSpeechRecognizer`) through objc2 bindings |
| Speech-to-text, Windows | Local Whisper (whisper-rs) with a rolling re-decode for live partial text |
| System audio, macOS | ScreenCaptureKit through objc2 bindings |
| System audio, Windows | WASAPI loopback through cpal |
| Microphone | cpal on both platforms |
| Layout | Repo root: `src-tauri/` (Rust), `ui/` (React) |

Windows' built-in speech recognizer was rejected because it only reads the default microphone and cannot transcribe a loopback stream.

## Feature parity checklist

These are carried over from the Swift app:

- Two capture modes:
  - **Video call:** the mic is "Me" and system audio is "Interviewer".
  - **In person:** the mic is "Interviewer".
- Live transcript with partial text, merged from both sources in spoken order.
- Utterances commit after 900 ms of silence. On macOS, recognition rolls over every 50 s.
- Question detection:
  - The keyword/`?` heuristic (`sounds_like_question`) triggers an automatic answer.
  - Otherwise, if a TypeSafe key is set and the utterance has at least 3 words, TypeSafe `asks_question` ≥ 0.7 (and not small talk) triggers it.
- Talking points:
  - The four providers (OpenAI Responses, Anthropic Messages, Gemini generateContent, local OpenAI-compatible) use the same prompt, the same 12,000-character transcript suffix, and the same per-provider default models.
  - TypeSafe is optional and adds question kind guidance plus résumé line evidence (threshold 1.5, max 8). Without evidence, the full résumé is used.
  - The newest question wins and cancels any pending request.
- Manual suggest, using the typed question or the latest transcript. Cancel.
- Setup:
  - experience and role text
  - résumé import (PDF, DOCX, RTF, TXT/MD; 10 MB and 100,000-character limits) with a review preview offering Replace, Add, or Use
  - provider, model, and local base URL
  - per-provider API keys with Save and Test connection
  - TypeSafe key with Save and Test
  - new: speech settings on Windows (Whisper model and re-decode interval)
- Past interviews: autosaved 2 s after changes and on quit, then listed, viewed, copied as Markdown, and deleted.
- New session (with confirmation) and Markdown export through a save dialog.
- Floating always-on-top assistant window.
- Tray/menu-bar item: Floating assistant, Show workspace, Quit.
- Shortcuts while focused: Ctrl/⌘+Enter to suggest, Ctrl/⌘+Shift+J to toggle the floating window.
- A five-step Welcome guide, shown until completed and reopenable from "Getting started".
- Dark theme matching the Swift palette (mint `#A6E0BA`, surface `#1B1F21`, sidebar `#111415`, background `#13171A`).

## Architecture

```
src-tauri/src/
  main.rs        Tauri builder: windows, tray, menu, commands, plugins
  session.rs     Session state + logic (port of Swift Session)
  ai.rs          LLMProvider, LLMConfiguration, request build/parse, generate()
  typesafe.rs    QuestionKind, Assessment, request build/parse, assess()
  transcript.rs  Speaker, Line, sounds_like_question, transcript formatting
  store.rs       paths, settings.json, keys.json, Interviews/*.json, Swift prefs import
  resume.rs      résumé text extraction
  audio/mod.rs   Source API: start(kind, sink) -> handle; 16 kHz mono f32 PCM
  audio/mic.rs   cpal microphone (both OSes)
  audio/macos.rs ScreenCaptureKit system audio
  audio/windows.rs WASAPI loopback (cpal)
  stt/mod.rs     Recognizer events: Partial(text), Final(text), Error(msg)
  stt/apple.rs   SFSpeechRecognizer + buffer requests, 50 s rollover (macOS)
  stt/whisper.rs Whisper rolling re-decode + energy VAD (Windows)
ui/src/
  main.tsx, floating.tsx    two entry points (two Vite pages)
  useCueState.ts            listens to `state` events, holds the snapshot
  components/               Workspace, CaptureControls, Transcript, TalkingPoints,
                            Notes, Setup (Experience, Provider, TypeSafe, Speech),
                            ResumePreview, PastInterviews, Welcome, Floating
  app.css
```

Platform code is gated with `#[cfg(target_os = ...)]`, and dependencies are split the same way:
- **macOS only:** objc2, objc2-foundation, objc2-speech, objc2-screen-capture-kit, objc2-av-foundation, objc2-core-media.
- **Windows only:** whisper-rs.
- **Shared:** tauri, tauri plugins (dialog, clipboard-manager, opener), tokio, reqwest (rustls, no redirects), serde/serde_json, cpal, lopdf/pdf-extract, zip, quick-xml, uuid, chrono, dirs, plist (macOS only), sha2.

### Audio → text pipeline

Each `AudioCapture(speaker)` owns one source and one recognizer. The source pushes 16 kHz mono f32 frames into the recognizer, and the recognizer emits events to the session:

- **Partial(text):** sets that capture's in-progress line (keeping its id and start time).
- **Final(text)** or 900 ms of silence: commits the line, clears the partial, and calls `on_utterance` for the interviewer capture.
- **Error(msg):** stops that capture and sets "Transcription stopped: … Start listening to reconnect."

**Apple Speech:** a port of the Swift `recognize()` loop:
- `addsPunctuation` is on, and on-device recognition is required when available.
- A new request starts after each utterance, and the 50 s timer forces a rollover.
- PCM frames are wrapped in `AVAudioPCMBuffer`, or the SCK `CMSampleBuffer` is appended directly.

**Whisper rolling re-decode:**
- An energy-threshold VAD marks speech start/end. Silence of 900 ms ends an utterance, and so does reaching 28 s of speech (Whisper's window is 30 s).
- While speaking, the utterance buffer is re-transcribed every `redecode_interval` (default 1.0 s) on a dedicated thread, and the result is emitted as Partial.
- At utterance end, a final decode is emitted as Final.
- If a decode is still running when the next tick fires, that tick is skipped, so the work never queues up.
- Calibration settings: `VAD_THRESHOLD`, pause length, and re-decode interval.
- Whisper models are `tiny.en`, `base.en` (default), and `small.en`, downloaded from the whisper.cpp Hugging Face repo into `<data>/models/`.
  - The download shows progress and is checked against a SHA-256. A mismatch or failure deletes the partial file.
  - Download starts from Setup, or when you click Start listening without a model.

### State flow

- Rust holds `Mutex<Session>`. Every change emits a `state` event carrying a serialized `Snapshot`. The snapshot includes:
  - answer, busy, error, history count
  - for each capture: running, starting, error, and its lines (partial included)
  - model download progress
- React's `useCueState()` subscribes to `state` and fetches an initial snapshot on mount. Both windows use the same hook.
- The question and notes fields are owned by React and sent through `set_notes` with a debounce. The question is passed as an argument to `suggest`.

Tauri commands:

```
start_listening(call: bool), stop_listening(), suggest(question: Option<String>), cancel(),
clear(), export(), set_notes(text), snapshot(),
get_settings(), save_settings(Settings), read_key(account), save_key(account, value),
test_provider(provider, model, local_base_url, key), test_typesafe(key),
import_resume(path) -> ImportedResume, list_interviews(), delete_interview(id),
download_whisper_model(name), toggle_floating(), show_main()
```

### Storage

The data directory is `~/Library/Application Support/Cue` on macOS and `%APPDATA%\Cue` on Windows. It holds:

- `keys.json`:
  - Format: `{account: key}`, using the Swift account names (`openai`, `claude`, `gemini`, `local`, `typesafe`).
  - On macOS the file mode is 0600. On Windows it lives in the per-user roaming profile.
  - Shared with the Swift app on macOS.
- `Interviews/<UUID>.json`:
  - The same `Interview` shape as Swift: `id` as an uppercase UUID string, `date` as ISO 8601, and the fields `role`, `transcript`, `talkingPoints`, `notes`.
  - Empty interviews are deleted, not saved.
- `settings.json` holds:
  - `provider`, `models{provider: id}`, `localBaseURL`
  - `resume`, `role`
  - `welcomed`
  - `whisperModel`, `redecodeInterval`
- **Swift prefs import (macOS, once):**
  - Runs when `settings.json` doesn't exist and `~/Library/Preferences/local.butler.desktop.plist` does.
  - Reads `resume`, `role`, `welcomed`, `llm.provider`, `model` (OpenAI), `llm.<p>.model`, and `llm.local.baseURL` into `settings.json`.

## Error handling

- `CueError(String)` serializes to the UI as a string. Message text is ported verbatim from Swift.
- Service error messages have the API key replaced by `[redacted]`.
- The HTTP client never follows redirects. Timeouts: cloud 90 s, local 180 s, TypeSafe 30 s.
- The local base URL must be http/https on `localhost`, `127.0.0.1`, or `::1`, with no credentials, query, or fragment.
- Each request has a run ID; a result from a superseded run is dropped. Cancel aborts the tokio task.
- Missing mic permission on Windows: "Allow microphone access for Cue in Settings → Privacy & security → Microphone."

## Testing

Rust unit tests (`cargo test`), porting the Swift tests:

- `ai.rs`: requests for all four providers (URL, headers, body), parse success/error per provider, key redaction, local URL validation, Gemini model ID validation.
- `transcript.rs`: question heuristic cases from `QuestionDetectionTests`.
- `typesafe.rs`: request shape with and without question detection, parse with evidence thresholds and ordering, error detail and redaction.
- `store.rs`:
  - interview round-trip and empty-interview deletion in a temp dir
  - keys save/remove
  - Swift plist import from a fixture plist
- `resume.rs`: one fixture each for PDF, DOCX, RTF, TXT, and UTF-16 TXT; unsupported extension, too large, empty, and folder cases. Fixtures live in `src-tauri/tests/fixtures/`.
- `stt/whisper.rs`: VAD segmentation using synthetic PCM (tone, then silence, then tone) splits into two utterances at the pause. The Whisper model is not loaded.

Manual testing covers live audio, permissions, Apple Speech, Whisper accuracy and CPU load, and real API calls. There is no React test suite at first.

## Build and packaging

- Prerequisites: Rust stable, Node 20+, CMake (Windows, for whisper.cpp), MSVC Build Tools and WebView2 (Windows), Xcode Command Line Tools (macOS).
- `npm install` (in `ui/`), then `cargo tauri dev` or `cargo tauri build`.
- Outputs:
  - macOS: `.app` / `.dmg`, ad-hoc signed.
  - Windows: `.msi` / NSIS `.exe`, unsigned.
- Bundle identifier: `local.butler.desktop`.
- The macOS Info.plist adds `NSMicrophoneUsageDescription` and `NSSpeechRecognitionUsageDescription` (same text as now). The icon comes from `Resources/Cue-master-1024.png` via `cargo tauri icon`.
- A new README describes the Rust app, covering the same user-facing sections as the current README with Windows notes added.

## Swift retirement (final step, after you accept the Rust app)

1. Tag the last commit that has the Swift app: `git tag swift-final`.
2. Delete `Package.swift`, `Sources/`, `Tests/`, `Info.plist`, `scripts/build.sh`, `Resources/Assets.xcassets`, and `Resources/Cue.icns`. Keep `Resources/Cue-master-1024.png` as the icon source.
3. Replace the README with the Rust app README.

## Out of scope

Linux, global shortcuts, auto-update, code signing/notarization, CI, a React test suite, cloud speech-to-text.
