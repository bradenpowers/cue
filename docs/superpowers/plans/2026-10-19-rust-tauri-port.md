# Cue Rust + Tauri Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust + Tauri version of Cue for macOS and Windows that matches the Swift app feature for feature, then retire the Swift app.

**Architecture:** All of Cue's logic and state lives in a Tauri 2 Rust crate under `src-tauri/`: providers, TypeSafe, question detection, résumé import, storage, audio, and speech-to-text. A React + TypeScript (Vite) frontend under `ui/` renders a `state` snapshot that Rust pushes as an event, and calls Rust commands. Code that differs by platform is gated with `cfg(target_os)`:
- **macOS:** Apple Speech and ScreenCaptureKit, through objc2.
- **Windows:** Whisper with a rolling re-decode, and WASAPI loopback.

**Tech Stack:**
- **Rust (stable) and Tauri 2:** tokio, reqwest (rustls), serde, cpal, pdf-extract, lopdf, zip, quick-xml, uuid, chrono, dirs, sha2, url.
- **macOS only:** plist and the objc2-* crates.
- **Windows only:** whisper-rs.
- **Frontend:** React 18, TypeScript and Vite 5, plus the `@tauri-apps/api` and `@tauri-apps/plugin-{dialog,clipboard-manager,opener}` packages.

**Spec:** `docs/superpowers/specs/2026-10-19-rust-tauri-port-design.md`. Read it before starting any task.

## Global Constraints

- **Platforms:** macOS 14+ and Windows 10/11. No Linux code paths.
- **Bundle identifier:** `local.butler.desktop`. Product name: `Cue`.
- **Data directory:** `dirs::data_dir()/Cue`. That is `~/Library/Application Support/Cue` on macOS and `%APPDATA%\Cue` on Windows.
- **Keys:** `keys.json` is a flat `{account: key}` map, with mode 0600 on unix. Account names are `openai`, `claude`, `gemini`, `local`, `typesafe`.
- **Interview JSON must stay readable by the Swift app:**
  - Keys: `id` (uppercase UUID string), `date` (ISO 8601, whole seconds, `Z`), `role`, `transcript`, `talkingPoints`, `notes`.
  - Dates with fractional seconds must also load.
- **Copy:** user-facing messages are ported verbatim from the Swift sources. The Swift files are the source of truth for copy; quote them, don't paraphrase.
- **HTTP:**
  - Never follow redirects. The one exception is the Whisper model download in Task 11.
  - Timeouts: cloud 90 s, local 180 s, TypeSafe 30 s.
  - API keys are replaced with `[redacted]` in every surfaced error.
- **Transcript and evidence:** the transcript sent to the LLM is the last 12,000 characters. TypeSafe tuning values:
  - question threshold 0.7
  - kind threshold 0.5
  - evidence threshold 1.5
  - at most 8 evidence lines
  - at most 80 résumé lines, each at least 25 characters
  - `earlier` is the last 1,500 characters
  - `latest` is the last 800 characters
- **Speech timing:**
  - An utterance ends after a 900 ms pause.
  - Apple Speech rolls over every 50 s.
  - Whisper force-splits at 28 s, re-decodes every 1.0 s by default, and uses `base.en` by default.
- **Default models:** openai `gpt-4.1-mini`, claude `claude-sonnet-4-6`, gemini `gemini-3.8-flash`, local empty. The default local base URL is `http://localhost:11434/v1`.
- **Window sizes:**
  - Main window: 1120×780 default, 860×650 minimum.
  - Floating window: 390×440, always on top.
- **Swift sources:** leave `Sources/`, `Tests/`, `Package.swift`, `scripts/` and `Info.plist` untouched until Task 15.
- **Test commands:** run Rust tests with `cargo test --manifest-path src-tauri/Cargo.toml`, and build the frontend with `npm --prefix ui run build`.

## Review Focus

1. **Swift-written interview files:** the Swift app writes `"date":"1970-01-01T00:16:40Z"` with an uppercase id. These must load in Rust, and files Rust saves must look the same. Test: `store::tests::reads_swift_interview_json` (Task 4).
2. **Corrupt or empty `keys.json`:** reading a key returns the Swift error "Could not read the … key from Cue's settings (…)." and never panics. Saving a key with a corrupt file returns an error and does not overwrite the file. Test: `store::tests::corrupt_keys_file_errors` (Task 4).
3. **Interrupted Whisper model download:** a partial file, or one with a checksum mismatch, never ends up at the final model path, so the next launch doesn't load a truncated model. Test: `stt::model::tests::checksum_mismatch_removes_part` (Task 11).
4. **No microphone, or microphone denied on Windows:** Start listening shows the spec's error text, the capture stays stopped, and the other source keeps working. Test: `audio::tests::no_device_error_text` covers the error mapping (Task 10). Live behavior is a manual check in Task 13.
5. **Quit while a suggestion is pending or the 2 s autosave hasn't fired yet:** the interview is still saved. Test: `session::tests::interview_snapshot_includes_unsaved_state` (Task 7). The exit hook is wired and manually checked in Task 8.

---

## File Structure

```
.gitignore                       + src-tauri/target, src-tauri/gen, ui/node_modules, ui/dist
src-tauri/
  Cargo.toml  build.rs  tauri.conf.json  Info.plist  capabilities/default.json  icons/
  src/main.rs                    calls cue_lib::run()
  src/lib.rs                     mod decls; run(): builder, windows, tray, commands, exit hook
  src/error.rs                   CueError, Result, err()
  src/transcript.rs              Speaker, Line, sounds_like_question, transcript(), merged()
  src/ai.rs                      LLMProvider, LLMConfiguration, HttpRequest, make_request, parse, generate, suggest
  src/typesafe.rs                QuestionKind, Assessment, resume_lines, make_request, parse, assess
  src/store.rs                   Store, Settings, Interview, swift prefs import
  src/resume.rs                  ImportedResume, extract (+ rtf stripper)
  src/session.rs                 Session, CaptureState, Snapshot, Heard (pure logic)
  src/app.rs                     AppState, async orchestration, emit, autosave, Tauri commands
  src/audio/mod.rs               Capture handle, start_mic, start_system, to_mono_16k
  src/audio/macos.rs             ScreenCaptureKit system audio
  src/audio/windows.rs           WASAPI loopback
  src/stt/mod.rs                 SttEvent, Recognizer re-export per OS
  src/stt/vad.rs                 Segmenter (compiled on all OSes, unit-tested on mac)
  src/stt/model.rs               Whisper model list, download + checksum (compiled on all OSes)
  src/stt/apple.rs               macOS Recognizer
  src/stt/whisper.rs             Windows Recognizer
  tests/fixtures/                swift-prefs.plist, resume.docx, resume.rtf
ui/
  package.json  vite.config.ts  tsconfig.json  index.html  floating.html
  src/main.tsx  src/floating.tsx  src/types.ts  src/api.ts  src/useCueState.ts  src/app.css
  src/components/{Workspace,Sidebar,CaptureControls,Transcript,TalkingPoints,Notes,
                  Setup,ExperienceSettings,ProviderSettings,TypeSafeSettings,SpeechSettings,
                  ResumePreview,PastInterviews,Welcome,Floating,Modal}.tsx
docs/rust-readme-draft.md        becomes README.md in Task 15
```

---

### Task 1: Scaffold the Tauri + React app

**Files:**
- Create: everything under `src-tauri/` listed above as stubs (`main.rs`, `lib.rs` with a `run()` that only builds the app from `tauri.conf.json`, `build.rs`), plus `tauri.conf.json`, `Info.plist`, `capabilities/default.json` and `icons/`.
- Create: `ui/` with Vite React-TS (`npm create vite@5 ui -- --template react-ts`), then trim the demo content.
- Modify: `.gitignore`

**Interfaces:**
- Produces: the crate `cue` with lib `cue_lib`; `tauri.conf.json` windows labeled `main` (`index.html`) and `floating` (`floating.html`); the Vite multi-page build with entries `index.html` and `floating.html`.

- [ ] **Step 1: Install the CLI and scaffold.** Run `cargo install tauri-cli --version "^2" --locked`. Scaffold `ui/` with Vite, then run `npm --prefix ui i @tauri-apps/api @tauri-apps/plugin-dialog @tauri-apps/plugin-clipboard-manager @tauri-apps/plugin-opener`.
- [ ] **Step 2: Write `src-tauri/Cargo.toml`.** `[package] name="cue"`, `[lib] name="cue_lib", crate-type=["staticlib","cdylib","rlib"]`, plus the shared dependencies from the Tech Stack and the `tauri-plugin-{dialog,clipboard-manager,opener}` crates. Add `[target.'cfg(target_os="macos")'.dependencies]` with plist (objc2 added in Task 12). The Windows target section is added in Task 13. Add `tempfile` as a dev-dependency.
- [ ] **Step 3: Write `tauri.conf.json`.**
  - `identifier: "local.butler.desktop"`, `productName: "Cue"`.
  - `build.frontendDist: "../ui/dist"`, `devUrl: "http://localhost:5173"`.
  - `beforeDevCommand` and `beforeBuildCommand` use the object form `{"script":"npm run dev","cwd":"../ui"}` (and `npm run build`).
  - Window `main`: `{"title":"Cue","width":1120,"height":780,"minWidth":860,"minHeight":650,"theme":"Dark"}`.
  - Window `floating`: `{"url":"floating.html","title":"Cue · Live assistant","width":390,"height":440,"alwaysOnTop":true,"visibleOnAllWorkspaces":true,"skipTaskbar":true,"visible":false,"theme":"Dark"}`.
  - `bundle.targets: ["app","dmg","msi","nsis"]`.
- [ ] **Step 4: Write `src-tauri/Info.plist`** with the `NSMicrophoneUsageDescription` and `NSSpeechRecognitionUsageDescription` strings copied verbatim from the root `Info.plist`. Tauri merges this file into the bundle.
- [ ] **Step 5: Generate the icons** with `cargo tauri icon Resources/Cue-master-1024.png -o src-tauri/icons`.
- [ ] **Step 6: Verify.** `npm --prefix ui run build` and `cargo build --manifest-path src-tauri/Cargo.toml` both succeed. `cargo tauri dev` (run from the repo root) opens a dark window titled "Cue". If `beforeDevCommand` can't find `ui/`, adjust `cwd` relative to `src-tauri/` and note it in the commit.
- [ ] **Step 7: Commit** with the message `Scaffold Tauri + React app`.

### Task 2: Error type and transcript model

**Files:**
- Create: `src-tauri/src/error.rs` and `src-tauri/src/transcript.rs`; register both in `lib.rs`.

**Interfaces:**
- Produces:
  - `pub struct CueError(pub String)`. It implements `Display`, `std::error::Error` and `Serialize` (serialized as a plain string), plus `From<std::io::Error>` and `From<serde_json::Error>` that copy the message.
  - `pub type Result<T> = std::result::Result<T, CueError>` and `pub fn err<T>(msg: impl Into<String>) -> Result<T>`.
  - `#[serde(rename_all="lowercase")] pub enum Speaker { Me, Them }` with `label() -> &'static str`, returning `"Me"` or `"Interviewer"`.
  - `pub struct Line { id: Uuid, speaker: Speaker, text: String, start: DateTime<Utc> }`, deriving Clone, Debug, PartialEq and Serialize, with `Line::new(speaker, text)` (new id, start = now) and `sounds_like_question(&self) -> bool`.
  - `pub fn transcript(lines: &[Line]) -> String` and `pub fn merged(a: &[Line], b: &[Line]) -> Vec<Line>`, which sorts by `start`.

- [ ] **Step 1: Write the failing tests** in `transcript.rs`, ported from `QuestionDetectionTests.swift`:

```rust
#[test] fn spots_questions_and_prompts() {
    for t in ["What drew you to this role?", "So, tell me about a time you disagreed with a manager.", "Great. Walk me through your last project",
              "Okay. How would you design a rate limiter", "Can you share an example of leading without authority"] {
        assert!(Line::new(Speaker::Them, t).sounds_like_question(), "{t}");
    }
}
#[test] fn ignores_statements() {
    for t in ["Thanks, that makes sense.", "Let me explain how the team is structured.", "We use Swift and Kotlin here", ""] {
        assert!(!Line::new(Speaker::Them, t).sounds_like_question(), "{t}");
    }
}
#[test] fn transcript_labels_speakers() {
    assert_eq!(transcript(&[Line::new(Speaker::Them, "Why Cue?"), Line::new(Speaker::Me, "I love the product.")]),
               "Interviewer: Why Cue?\nMe: I love the product.");
}
#[test] fn merged_orders_by_start() // a.start = t+1s, b.start = t  =>  merged(&[a.clone()], &[b.clone()]) == vec![b, a]
```

- [ ] **Step 2: Run** `cargo test --manifest-path src-tauri/Cargo.toml transcript`. Expected: compile failure.
- [ ] **Step 3: Implement.** `sounds_like_question` mirrors Swift `Line.soundsLikeQuestion`:
  - Lowercase the text. If the trimmed text ends with `?`, it's a question.
  - Otherwise, split the text into clauses on any of `.!,;`. It's a question when any trimmed clause starts with one of the 18 `prompts` from `Sources/Cue/Audio.swift`. Copy the list verbatim.
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** with the message `Add error type and transcript model`.

### Task 3: AI providers

**Files:**
- Create: `src-tauri/src/ai.rs`

**Interfaces:**
- Consumes: `CueError`, `Result`, `err` (Task 2).
- Produces:
  - `#[serde(rename_all="lowercase")] pub enum LLMProvider { Openai, Claude, Gemini, Local }`, which serializes as `"openai"`, `"claude"`, `"gemini"` and `"local"`. Methods: `ALL: [LLMProvider;4]`, `label()`, `default_model()`, `requires_key()`, `key_url() -> Option<&'static str>`, `account() -> &'static str`. The label, model and URL strings come from `AI.swift` and `Welcome.swift`.
  - `pub struct LLMConfiguration { provider, model: String, local_base_url: String }`.
  - `pub struct HttpRequest { url: String, headers: Vec<(String,String)>, body: serde_json::Value, timeout: Duration }`.
  - `pub fn http_client() -> reqwest::Client`, built with `redirect(Policy::none())`.
  - `pub fn redact(text: &str, key: &str) -> String`, which replaces the trimmed key when it's non-empty.
  - `pub async fn send(client: &reqwest::Client, req: &HttpRequest) -> std::result::Result<(u16, Vec<u8>), reqwest::Error>`.
  - `pub const INSTRUCTIONS: &str`, copied verbatim from `AssistantAPI.instructions`.
  - `pub fn make_request(cfg: &LLMConfiguration, key: &str, instructions: &str, input: &str) -> Result<HttpRequest>`.
  - `pub fn parse(body: &[u8], status: u16, provider: LLMProvider, key: &str) -> Result<String>`.
  - `pub async fn generate(client: &reqwest::Client, cfg: &LLMConfiguration, key: &str, instructions: &str, input: &str) -> Result<String>`.
  - `pub async fn suggest(client: &reqwest::Client, cfg: &LLMConfiguration, key: &str, resume: &str, role: &str, transcript: &str, question: &str, guidance: Option<&str>) -> Result<String>`.

- [ ] **Step 1: Write the failing tests.** Port every test in `Tests/CueTests/ProviderTests.swift` and `ResponseTests.swift`, except the settings and credentials tests, which move to Task 4. Use helpers `req(provider, model, key, base)` with the Swift defaults (`"example-model"`, `"test-credential"`, `"http://localhost:11434/v1"`, instructions `"Coach instructions"`, input `"Candidate context"`) and `header(&req, name) -> Option<&str>`. Test names:
  - `openai_payload_and_credentials`
  - `claude_uses_messages_and_separate_system_prompt`
  - `gemini_uses_header_and_model_path`: the URL ends `/v1beta/models/gemini-3.8-flash:generateContent` with no query, and `model?key=wrong` is an error.
  - `local_server_allows_optional_key_and_trailing_slash`: covers the three Swift bases, the path is always `/v1/chat/completions`, there's no Authorization header with an empty key, and there's a Bearer header with a key.
  - `validation`: a blank key is rejected for the three cloud providers, a blank model for all four, and the six rejected local bases from Swift are errors.
  - `response_formats_exclude_thinking_and_alternatives`: the four Swift fixtures all parse to `"Answer"`.
  - `openai_joins_output_text_across_messages`: the ResponseTests fixture parses to `"First\nSecond"`.
  - `errors_and_blocked_responses`: for every provider, the 401 error contains `[redacted]` and not `test-credential`. `{}` with 200 is an error, `<html>Bad gateway</html>` with 502 is an error, and Gemini `promptFeedback` with 200 is an error.
- [ ] **Step 2: Run** `cargo test --manifest-path src-tauri/Cargo.toml ai::`. Expected: compile failure.
- [ ] **Step 3: Implement** by porting `makeRequest` and `parse` from `AI.swift`, keeping all messages verbatim.
  - Validate the local URL with `url::Url`:
    - The scheme must be http or https.
    - `host_str()` must be one of `localhost`, `127.0.0.1` or `[::1]`.
    - Reject URLs with a username or password, a query, or a fragment.
  - Append `chat/completions` after trimming a trailing `/` from the path.
  - Map errors in `generate` like this:
    - `is_timeout` gives "`{label}` timed out. Try again; local models may need time to load."
    - Any other request error gives the local or cloud "Could not reach…" message from Swift.
  - `suggest` builds the `EXPERIENCE:\n…\nROLE:\n…\nRECENT TRANSCRIPT (Interviewer = the other side, Me = the candidate):\n…\nQUESTION / REQUEST:\n…` input exactly as in Swift. It takes the 12,000-character transcript suffix by **chars**, and passes `INSTRUCTIONS` plus `" " + guidance` when guidance is present.
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** with the message `Port AI provider adapters`.

### Task 4: Store (keys, settings, interviews, Swift prefs import)

**Files:**
- Create: `src-tauri/src/store.rs` and `src-tauri/tests/fixtures/swift-prefs.plist`

**Interfaces:**
- Consumes: `LLMProvider` and `LLMConfiguration` (Task 3).
- Produces:
  - `pub struct Store { pub dir: PathBuf }` with:
    - `Store::new(dir)`
    - `Store::default_dir() -> PathBuf`, which is `dirs::data_dir().unwrap().join("Cue")`
    - `read_key(&self, account: &str, label: &str) -> Result<String>`, which returns `""` when the file is missing
    - `save_key(&self, value: &str, account: &str) -> Result<()>`, where an empty value removes the entry
    - `load_settings(&self) -> Settings`, which returns the default when the file is missing or unreadable
    - `save_settings(&self, &Settings) -> Result<()>`
    - `save_interview(&self, &Interview) -> Result<()>`, which deletes the file when the interview is empty
    - `load_interviews(&self) -> Vec<Interview>`, newest first, skipping unreadable files
    - `delete_interview(&self, id: Uuid) -> Result<()>`, a no-op when the file is missing
    - `#[cfg(target_os="macos")] import_swift_prefs(&self, plist: &Path) -> Result<bool>`
  - `#[serde(rename_all="camelCase", default)] pub struct Settings { provider: LLMProvider, models: BTreeMap<String,String>, local_base_url: String, resume: String, role: String, welcomed: bool, whisper_model: String, redecode_interval: f32 }`.
    - Defaults: openai, empty models, `http://localhost:11434/v1`, `""`, `""`, false, `"base.en"`, `1.0`.
    - Methods: `model(&self, p) -> String` (the saved model or `p.default_model()`), `set_model(&mut self, p, m)`, and `llm(&self) -> LLMConfiguration`.
  - `#[serde(rename_all="camelCase")] pub struct Interview { id: Uuid, date: DateTime<Utc>, role: String, transcript: String, talking_points: Vec<String>, notes: String }`, with:
    - `Interview::new()`
    - `is_empty()`: transcript and notes are blank and there are no talking points
    - `title()`: the first line of `role`, or `"Interview"`
    - `markdown()`: the same layout as Swift's `Interview.markdown`, with the date formatted as local time using `%b %-d, %Y at %-I:%M %p`
- Every write is atomic: write `<name>.tmp`, then rename. On unix, `keys.json` is set to mode 0600.

- [ ] **Step 1: Write the failing tests** (use a `tempfile::TempDir`):
  - `save_load_and_delete_interviews`: ports `InterviewStoreTests` exactly. It covers the dates at epoch 1000 and 2000, the order newer then older, `title() == "Staff Engineer"`, saving an empty interview with the same id removes it, and deleting twice is fine.
  - `reads_swift_interview_json`: writes a file containing `{"id":"0F8B2C1E-5D6A-4B7C-8D9E-0A1B2C3D4E5F","date":"1970-01-01T00:16:40Z","role":"","transcript":"t","talkingPoints":[],"notes":""}` and asserts it loads. Saving it back produces file text that contains `"date":"1970-01-01T00:16:40Z"` and `"0F8B2C1E-5D6A-4B7C-8D9E-0A1B2C3D4E5F"`. A date of `"1970-01-01T00:16:40.5Z"` also loads.
  - `keys_stored_in_app_file`: ports `testCredentialsStoredInAppFile`, including the `#[cfg(unix)]` 0600 check.
  - `corrupt_keys_file_errors`: writes `not json` to `keys.json`. `read_key("openai","ChatGPT / OpenAI")` returns an error that starts with `Could not read the ChatGPT / OpenAI key from Cue's settings (`. `save_key("x","openai")` returns an error and leaves the file bytes unchanged.
  - `settings_default_and_per_provider_models`: a missing file gives defaults. `set_model(Claude,"x")` round-trips through save and load. `model(Openai)` with nothing saved returns `"gpt-4.1-mini"`.
  - `#[cfg(target_os="macos")] imports_swift_prefs`: copies the fixture into the temp dir and imports it. The result is `provider == Claude`, `model(Openai) == "my-existing-model"`, `model(Claude) == "my-claude-model"`, `resume == "R"`, `role == "Ro"`, `welcomed` and `local_base_url == "http://127.0.0.1:1234/v1"`. It returns `true`, and a second call returns `false` because `settings.json` already exists.
- [ ] **Step 2: Create the fixture** `src-tauri/tests/fixtures/swift-prefs.plist` with `plutil -create xml1`, then `plutil -insert <key> -string <value>` for `model`, `llm.provider`, `llm.claude.model`, `resume`, `role` and `llm.local.baseURL`, plus `-bool YES` for `welcomed`.
- [ ] **Step 3: Run the tests.** Expected: failure.
- [ ] **Step 4: Implement.**
  - Serialize the UUID as uppercase with a small `serialize_with` function. Deserializing parses either case.
  - Serialize the date with `to_rfc3339_opts(SecondsFormat::Secs, true)`; deserializing uses `DateTime::parse_from_rfc3339`.
  - The plist import uses `plist::Value::from_file`. The OpenAI model key is `model`, and every other provider uses `llm.<p>.model`.
- [ ] **Step 5: Run the tests.** Expected: PASS.
- [ ] **Step 6: Commit** with the message `Add store for keys, settings, interviews`.

### Task 5: TypeSafe

**Files:**
- Create: `src-tauri/src/typesafe.rs`

**Interfaces:**
- Consumes: `HttpRequest`, `send` and `redact` (Task 3).
- Produces:
  - `#[serde(rename_all="snake_case")] pub enum QuestionKind { Behavioral, Technical, Experience, Logistics, SmallTalk, Other }`, with `ALL`, `raw()`, `rubric()` and `guidance() -> Option<&'static str>`. Copy the strings verbatim from `TypeSafe.swift`.
  - `#[derive(Debug, Clone, PartialEq)] pub struct Assessment { asks_question: f64, kind: QuestionKind, evidence: Vec<String> }`.
  - Constants: `ACCOUNT="typesafe"`, `MODEL="jev-latest"`, `QUESTION_THRESHOLD`, `KIND_THRESHOLD`, `EVIDENCE_THRESHOLD`, `MAX_EVIDENCE`, `RELEVANCE_LEVELS`.
  - Functions:
    - `pub fn resume_lines(resume: &str) -> Vec<String>`
    - `pub fn make_request(key: &str, role: &str, earlier: &str, latest: &str, lines: &[String], detect_question: bool) -> Result<HttpRequest>`
    - `pub fn parse(body: &[u8], status: u16, lines: &[String], key: &str) -> Result<Assessment>`
    - `pub async fn assess(client: &reqwest::Client, key: &str, role: &str, earlier: &str, latest: &str, lines: &[String], detect_question: bool) -> Result<Assessment>`

- [ ] **Step 1: Write the failing tests**, porting all four `TypeSafeTests.swift` tests with the same JSON fixtures and expected values:
  - `request_asks_one_score_per_line_and_optional_detection`
  - `parse_keeps_relevant_lines_in_resume_order_and_gates_kind`
  - `errors_redact_key`: the error is exactly `"TypeSafe: bad key [redacted]"`.
  - `resume_lines_drop_short_fragments`
- [ ] **Step 2: Run the tests.** Expected: failure.
- [ ] **Step 3: Implement** by porting from `TypeSafe.swift`.
  - Truncate `role` to 4,000 characters.
  - In `assess`, a request error gives "Could not reach TypeSafe. Check your internet connection." Cancellation is a tokio task abort, so there's no separate path for it.
  - Status 429 or 529 with no error detail gives "Busy right now; try again shortly."
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** with the message `Port TypeSafe client`.

### Task 6: Résumé import

**Files:**
- Create: `src-tauri/src/resume.rs`, `src-tauri/tests/fixtures/resume.docx` and `src-tauri/tests/fixtures/resume.rtf`

**Interfaces:**
- Produces:
  - `#[derive(Serialize)] pub struct ImportedResume { filename: String, text: String }`
  - `pub const MAX_BYTES: u64 = 10 * 1024 * 1024`
  - `pub const MAX_CHARS: usize = 100_000`
  - `pub fn extract(path: &Path) -> Result<ImportedResume>`

- [ ] **Step 1: Create the fixtures** once:

```bash
printf 'Jane Doe\nEngineer at Example\nReduced latency by 20%%\n' > /tmp/r.txt
textutil -convert docx /tmp/r.txt -output src-tauri/tests/fixtures/resume.docx
textutil -convert rtf  /tmp/r.txt -output src-tauri/tests/fixtures/resume.rtf
```

- [ ] **Step 2: Write the failing tests**, porting `ResumeImporterTests.swift` and writing files into a TempDir:
  - `plain_text_normalizes_line_endings`: `"  Jane Doe\r\nEngineer — shipped 3 products\rSwift  \n"` gives `"Jane Doe\nEngineer — shipped 3 products\nSwift"`, with filename `Resume.TXT`.
  - `word_and_rtf_extraction`: both fixtures contain `Engineer at Example` and `Reduced latency by 20%`.
  - `pdf_extraction`: builds a one-page PDF in the test with lopdf (Helvetica font resource `F1`, content `BT /F1 14 Tf 40 720 Td (Jane Doe - Software Engineer) Tj ET`). The extracted text contains that string.
  - `rejects_empty_scanned_and_malformed`: covers `empty.txt` with `" \n "`, `scan.pdf` (a lopdf page with an empty content stream), `broken.pdf` with `"not a PDF"`, and `broken.docx` with `"not a Word document"`.
  - `rejects_oversized_and_unsupported`: covers a `MAX_BYTES + 1` byte `large.txt` and `resume.html`.
  - `reads_utf16`: `"Résumé — Jane Doe"` encoded as UTF-16LE with a `FF FE` BOM round-trips.
  - `rejects_folder`: a directory named `x.txt` gives "Choose a résumé file, not a folder."
- [ ] **Step 3: Run the tests.** Expected: failure.
- [ ] **Step 4: Implement.** Port the checks and messages from `ResumeImporter.swift` in the same order (extension, regular file, size, read, parse, normalize, empty, max chars).
  - **pdf:** `lopdf::Document::load_mem`; if it fails, return the "could not be read" message. If `is_encrypted()`, return the password message. Otherwise use `pdf_extract::extract_text_from_mem`.
  - **docx:** read `word/document.xml` from the zip and stream it with quick-xml. Text in `w:t` is kept. The end of each `w:p` and each `w:br` become `\n`, and `w:tab` becomes `\t`.
  - **rtf:** use a hand-written `fn rtf_to_text(&str) -> String`.
    - Track `{` and `}` group depth.
    - Skip groups that start with `\*`, `\fonttbl`, `\colortbl`, `\stylesheet`, `\info` or `\pict`.
    - `\par` and `\line` become `\n`; `\tab` becomes `\t`.
    - `\'hh` becomes the Latin-1 character for that byte.
    - `\uN` becomes `char::from_u32` of N, wrapping negative values by +65536, then skips one fallback character.
    - `\\`, `\{` and `\}` become literal characters. Other control words and their optional numeric parameter plus one trailing space are dropped.
    - Add `// ponytail: minimal RTF reader, swap for a crate if real résumés break it`.
  - **Text files:** UTF-8 first, then UTF-16 (LE or BE by BOM).
  - Normalize `\r\n` and `\r` to `\n`, strip `\0` and trim. Count characters with `chars().count()`.
- [ ] **Step 5: Run the tests.** Expected: PASS.
- [ ] **Step 6: Commit** with the message `Port résumé importer`.

### Task 7: Session logic (pure)

**Files:**
- Create: `src-tauri/src/session.rs`

**Interfaces:**
- Consumes: `Line`, `Speaker`, `merged`, `transcript` (Task 2); `Interview` (Task 4); `Assessment` (Task 5).
- Produces:
  - `#[derive(Default, Clone)] pub struct CaptureState { pub running: bool, pub starting: bool, pub error: String, pub segments: Vec<Line>, pub partial: Option<Line> }` with:
    - `lines(&self) -> Vec<Line>`: segments plus the partial
    - `set_partial(&mut self, speaker: Speaker, text: &str)`: keeps the existing partial's id and start, or creates a new `Line`
    - `commit(&mut self) -> Option<Line>`: takes the partial and trims it. Non-empty text is pushed onto `segments` and returned.
  - `pub struct Session` with public fields `them: CaptureState`, `me: CaptureState`, `answer: String`, `busy: bool`, `error: String`, `history: Vec<String>`, `notes: String`, `download: Option<f32>` and `run: u64`, and private fields `answered: usize` and `interview: Interview`. Methods:
    - `Session::new()`
    - `lines(&self) -> Vec<Line>`: `merged(them.lines(), me.lines())`
    - `snapshot(&self) -> Snapshot`
    - `heard(&mut self, has_typesafe_key: bool) -> Heard`
    - `answered(&self) -> usize`
    - `begin_run(&mut self) -> u64`: increments `run`, sets `busy`, clears `error`
    - `finish_run(&mut self, id: u64, result: Result<(String, String, String)>)`: the tuple is (question, answer, note). It's ignored unless `id == run`. Success sets `answer`, appends `"## {q}\n\n{answer}"` to `history` and sets `error = note`. Failure sets `error`. Both set `busy = false`.
    - `to_interview(&self, role: &str) -> Interview`: the current interview id and date, with fields filled from the session
    - `reset(&mut self)`: clears everything for a new session and starts a new `Interview`
  - `#[derive(Serialize)] #[serde(rename_all="camelCase")] pub struct Snapshot { lines: Vec<Line>, them: CaptureView, me: CaptureView, answer: String, busy: bool, error: String, download: Option<f32> }`, where `#[derive(Serialize)] pub struct CaptureView { running: bool, starting: bool, error: String }`.
  - `#[derive(Debug, PartialEq)] pub enum Heard { Ignore, Answer(String), Check { latest: String, earlier: String, count: usize } }`.
  - `pub fn experience_for(assessment: Option<&Assessment>, resume: &str) -> String`: the evidence joined with `\n` when it's non-empty, otherwise `resume`.
  - `pub fn suffix(s: &str, n: usize) -> String`: the last `n` characters, counted by chars.

- [ ] **Step 1: Write the failing tests:**
  - `heard_answers_question_heuristically`: commits "So, tell me about a time you led a migration." to `them`. `heard(false)` returns `Answer` with that exact text, and a second `heard(false)` returns `Ignore`.
  - `heard_checks_statements_with_typesafe`: "I'd love to hear about your last project" gives `heard(true)` matching `Check { count: 1, .. }`. In a fresh session the same line gives `heard(false) == Ignore`.
  - `heard_ignores_short_fragments`: "Okay great" gives `heard(true) == Ignore`.
  - `heard_latest_joins_unanswered_and_caps_800`: two unanswered statements of 500 'a' and 500 'b' give `latest == suffix(&(a + " " + b), 800)`.
  - `stale_run_is_ignored`: with `a = begin_run()` and `_b = begin_run()`, calling `finish_run(a, Ok(("q","ans","")))` leaves `answer` empty and `busy` true.
  - `experience_falls_back_to_resume`: `None` gives the résumé; empty evidence gives the résumé; `["x","y"]` gives `"x\ny"`.
  - `interview_snapshot_includes_unsaved_state`: with a partial "half a sentence" on `them`, `history = ["## q\n\na"]` and `notes = "n"`, `to_interview("Role")` has a transcript containing "half a sentence", plus `talking_points == history`, `notes == "n"` and `role == "Role"`.
- [ ] **Step 2: Run the tests.** Expected: failure.
- [ ] **Step 3: Implement.** `heard` ports `Session.heard()` from `CueApp.swift`.
  - The unanswered lines are `them.segments[answered..]`.
  - `Answer` sets `answered = them.segments.len()`.
  - `Check.earlier` is `suffix(transcript(lines started before the first unanswered line), 1500)`.
  - `Check` requires at least 3 whitespace-separated words in `latest`.
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** with the message `Add session logic`.

### Task 8: Tauri wiring (commands, events, windows, tray, autosave)

**Files:**
- Create: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/lib.rs` and `src-tauri/capabilities/default.json` (windows `main` and `floating`; permissions `core:default`, `dialog:default`, `clipboard-manager:allow-write-text`, `opener:default`)

**Interfaces:**
- Consumes: everything from Tasks 2–7. Captures go through `start_capture` and `stop_capture` in `app.rs`, which return `err("Live transcription isn't available yet.")` until Task 10 replaces them.
- Produces:
  - `pub struct AppState { session: Mutex<Session>, store: Store, client: reqwest::Client, pending: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>, save_gen: AtomicU64 }`.
  - Tauri commands use these exact names; the UI passes camelCase args:

```
snapshot() -> Snapshot                       start_listening(call: bool)       stop_listening()
suggest(question: Option<String>)            cancel()                          clear()
export(path: String)                         set_notes(text: String)           get_settings() -> Settings
save_settings(settings: Settings)            read_key(account: String) -> String
save_key(account: String, value: String)     test_provider(provider: LLMProvider, model: String, local_base_url: String, key: String) -> String
test_typesafe(key: String)                   import_resume(path: String) -> ImportedResume
list_interviews() -> Vec<Interview>          delete_interview(id: String)
download_whisper_model(name: String)         toggle_floating()                 show_main()
```

  - `fn emit(app: &AppHandle)` locks the session and sends `app.emit("state", snapshot)`.
  - `fn touch(app: &AppHandle)` emits, then saves 2 s later. It increments `save_gen` and only saves if the value is unchanged when the timer fires.
  - `fn save_now(app: &AppHandle)` writes `session.to_interview(settings.role)` with `store.save_interview`, and on failure sets `error = "Could not save this interview: {e}"`.
  - `async fn run_suggestion(app: AppHandle, typed: Option<String>, assessment: Option<Assessment>)` ports `Session.start()` from Swift: the default question text, the TypeSafe assess when there's a key and no assessment, the fallback note "{e} Used your full résumé instead.", then `experience_for` → `ai::suggest` → `finish_run`.
  - `fn on_utterance(app: &AppHandle)` calls `heard(has_key)`:
    - `Answer(q)` starts the suggestion with that question.
    - `Check` spawns `typesafe::assess` with `detect_question = true`. When the result arrives, it re-checks that `them.segments.len() == count`, `answered < count`, `asks_question >= QUESTION_THRESHOLD` and `kind != SmallTalk`, then sets `answered = count` and starts the suggestion.
  - Starting a suggestion aborts any pending task ("newest wins"). `cancel()` aborts it and sets `busy = false`.

- [ ] **Step 1: Implement `app.rs` and register it in `lib.rs::run()`.**
  - Register the plugins and `manage(AppState)`. On macOS, call `store.import_swift_prefs(home/Library/Preferences/local.butler.desktop.plist)` before the first `load_settings`, and ignore errors.
  - Build the tray with `TrayIconBuilder` and the default window icon. Menu: "Floating assistant" (`toggle_floating`), "Show workspace" (`show_main`), "Quit Cue" (`app.exit(0)`).
  - `on_window_event`: `CloseRequested` on `main` or `floating` calls `api.prevent_close()` and hides the window.
  - `RunEvent::Exit` calls `save_now` synchronously.
  - `test_provider` calls `ai::generate(…, "Reply briefly.", "Reply with OK to confirm this connection works.")` and returns `"Connected to {label} using {model}."`.
  - `test_typesafe` calls `assess(key, "", "", "Tell me about yourself.", &[], false)`.
  - `export(path)` calls `save_now`, then writes `interview.markdown()` to `path`.
  - `toggle_floating` shows and focuses the `floating` window, or hides it. `show_main` shows, unminimizes and focuses `main`.
- [ ] **Step 2: Verify that `cargo build --manifest-path src-tauri/Cargo.toml` succeeds** and that the earlier tests still pass.
- [ ] **Step 3: Verify the quit hook manually.** Set `app.withGlobalTauri: true` in `tauri.conf.json`. In `cargo tauri dev`, run `await __TAURI__.core.invoke('set_notes',{text:'quit test'})` in devtools, then choose "Quit Cue" from the tray within 2 s. A new file in `<data>/Interviews/` contains `"notes":"quit test"`.
- [ ] **Step 4: Commit** with the message `Wire Tauri commands, events, tray, autosave`.

### Task 9: React UI

**Files:**
- Create: everything under `ui/src/` from the File Structure, plus `ui/floating.html`
- Modify: `ui/vite.config.ts` to set `build.rollupOptions.input` to `{ main: 'index.html', floating: 'floating.html' }`, `server.strictPort: true` and `server.port: 5173`

**Interfaces:**
- Consumes: the Task 8 commands and the `state` event.
- Produces:
  - `types.ts` mirrors `Snapshot`, `CaptureView`, `Line`, `Settings`, `Interview`, `ImportedResume` and `LLMProvider` (`'openai'|'claude'|'gemini'|'local'`), using the serialized camelCase field names.
  - `api.ts` has one typed wrapper per Task 8 command, e.g. `export const suggest = (question?: string) => invoke<void>('suggest', { question })`. The `export` command is wrapped as `exportSession`.
  - `useCueState(): Snapshot | null` calls `snapshot()` on mount and subscribes with `listen<Snapshot>('state')`, unsubscribing on unmount.
  - `isMac: boolean`, from `navigator.userAgent.includes('Mac')`.
  - `providerLabels` and `defaultModels` in `types.ts` must match the Rust values.

- [ ] **Step 1: Build the main window** (`main.tsx`, `Workspace`, `Sidebar`, `CaptureControls`, `Transcript`, `TalkingPoints`, `Notes`, `Modal`) to match the layout and copy of `CueApp.swift`: `Workspace`, `CaptureControls`, `TranscriptView` and the talking-points panel. Copy all strings verbatim.
  - Use the spec's colors as CSS variables in `app.css`, a dark theme only, and the system font stack.
  - Platform-specific copy:
    - The footer reads "Interviews save automatically on this Mac." / "macOS · Cue" on macOS, and "Interviews save automatically on this PC." / "Windows · Cue" on Windows.
    - On Windows, the transcript empty-state hint is replaced with "Video call captures your computer's audio. Use headphones on calls so your mic hears only you. Speech is transcribed locally with Whisper."
    - Shortcut labels show ⌘ on macOS and Ctrl on Windows.
  - The Video call / In person picker is local state. Its button calls `startListening(call)` or `stopListening()`, and it's disabled while a capture is running or starting.
  - The question field is local state. Suggest calls `suggest(question.trim() || undefined)`.
  - Notes are local state, sent with `setNotes` after a 500 ms debounce.
  - Copy uses `writeText` from the clipboard plugin.
  - Export uses `save({ defaultPath: 'Cue session.md' })`, then `exportSession(path)` when a path was chosen.
  - New session opens a confirmation `Modal` with Swift's text, then calls `clear()`, which also clears the local question and notes.
  - The transcript scrolls to the bottom whenever `lines` changes.
- [ ] **Step 2: Add shortcuts.** In both entry points, a `keydown` listener on `window`: `(metaKey||ctrlKey) && key==='Enter'` calls suggest with the current question, and `(metaKey||ctrlKey) && shiftKey && key.toLowerCase()==='j'` calls `toggleFloating()`.
- [ ] **Step 3: Build the floating window** (`floating.tsx` and `Floating`) as a port of `FloatingView`, with its own question field.
- [ ] **Step 4: Build the Setup modal** (`Setup`, `ExperienceSettings`, `ProviderSettings`, `TypeSafeSettings`, `SpeechSettings`) as a port of `SetupView` and its subviews, including:
  - Per-provider key drafts kept while switching providers.
  - Test connection with Cancel. Cancel ignores the late result by tracking a request counter; nothing is cancelled on the backend.
  - Résumé import: `open({ filters: [{ name: 'Résumé', extensions: ['pdf','docx','rtf','txt','text','md'] }] })` → `importResume(path)` → the `ResumePreview` modal with Replace, Add, or Use, ported from `ResumePreview`.
  - Settings edits call `saveSettings` with a 500 ms debounce.
  - `SpeechSettings` renders only when `!isMac`:
    - A model select (`tiny.en`, `base.en`, `small.en`).
    - A "Download model" button, with a progress bar while `snapshot.download != null`.
    - A "Re-decode interval (seconds)" number input (0.5–3.0, step 0.5).
    - Hint text: "Smaller models and longer intervals use less CPU."
- [ ] **Step 5: Build `PastInterviews` and `Welcome`.**
  - `PastInterviews` ports `InterviewsView`: list, view, copy and delete.
  - `Welcome` ports all five steps of `WelcomeView`, with these changes:
    - The Windows variant of the step-5 Permissions text is: "The first time, Windows may ask to allow microphone access. Cue captures audio only."
    - Step 3 blocks Continue unless `readKey(provider)` is non-empty, and shows "Skip for now".
  - Links open with `openUrl` from the opener plugin.
  - Welcome shows while `!settings.welcomed`; "Start using Cue" saves `welcomed: true`. "Getting started" in the sidebar saves `welcomed: false`.
- [ ] **Step 6: Verify.** `npm --prefix ui run build` passes with no TypeScript errors. In `cargo tauri dev`, manually check that:
  - the Welcome flow completes
  - Setup saves a provider key, and Test connection reports success or a clear error
  - a typed question with Suggest returns talking points
  - export writes a `.md` file
  - Past interviews lists the session
  - Ctrl/⌘+Shift+J toggles the floating window, and the tray items work
  - closing the main window hides it, and "Show workspace" brings it back
- [ ] **Step 7: Commit** with the message `Add React UI`.

### Task 10: Audio plumbing, resampling, VAD segmenter

**Files:**
- Create: `src-tauri/src/audio/mod.rs`, `src-tauri/src/stt/mod.rs` and `src-tauri/src/stt/vad.rs`
- Modify: `src-tauri/src/app.rs`, replacing the `start_capture` and `stop_capture` stubs

**Interfaces:**
- Produces:
  - `pub type Sink = Box<dyn FnMut(&[f32]) + Send>`. Frames are 16 kHz mono f32.
  - `pub struct Capture` is an opaque handle that stops the stream when dropped.
  - `pub fn start_mic(sink: Sink) -> Result<Capture>` uses the cpal default input on both OSes.
  - `pub fn start_system(sink: Sink) -> Result<Capture>` dispatches to `macos::start` or `windows::start`.
  - `pub fn to_mono_16k(input: &[f32], channels: u16, rate: u32) -> Vec<f32>` averages the channels and linearly resamples. Mark it with `// ponytail: linear resample, fine for speech; use rubato if accuracy suffers`.
  - `pub enum MicError { NoDevice, Denied, Other(String) }` and `pub fn mic_error(e: MicError) -> CueError`:
    - `NoDevice` → "No working microphone found."
    - `Denied` → "Allow microphone access for Cue in Settings → Privacy & security → Microphone." on Windows, and the Swift Microphone text on macOS.
    - `Other(s)` → `s`.
  - `#[derive(Debug, Clone, PartialEq)] pub enum SttEvent { Partial(String), Final(String), Error(String) }`.
  - `stt::Recognizer` uses the same API on both OSes, implemented in Tasks 12 and 13 and re-exported by cfg:
    - `Recognizer::start(on_event: Arc<dyn Fn(SttEvent) + Send + Sync>, settings: &Settings, data_dir: &Path) -> Result<Recognizer>`
    - `fn push(&self, pcm: &[f32])`
    - `fn stop(self)`
  - `pub struct Segmenter` with:
    - `Segmenter::new(threshold: f32, pause_ms: u32, max_ms: u32)`
    - `push(&mut self, pcm: &[f32]) -> Option<Vec<f32>>`, which returns the finished utterance audio
    - `current(&self) -> &[f32]`
    - `speaking(&self) -> bool`
    - Constants: `VAD_THRESHOLD: f32 = 0.01` (RMS over 30 ms / 480-sample frames), `PAUSE_MS = 900`, `MAX_MS = 28_000`.
- Wiring in `app.rs`:
  - `start_listening(call)` ports `Session.startListening`: `them` uses system audio when `call`, otherwise the mic; `me` uses the mic only when `call`.
  - For each capture, create a `Recognizer` and a sink that calls `recognizer.push`. Recognizer events lock the session:
    - `Partial(t)` calls `set_partial` and `emit`.
    - `Final(t)` calls `set_partial(t)` then `commit()`, then `touch`. For `them`, it then runs `on_utterance`.
    - `Error(m)` stops that capture and sets its `error` to "Transcription stopped: {m}. Start listening to reconnect."
  - A capture start error sets that capture's `error` and leaves the other capture running.
  - Until Tasks 12 and 13 exist, a stub `Recognizer` that only returns errors is compiled behind `cfg(not(any(target_os="macos", windows)))`, so the crate builds.

- [ ] **Step 1: Write the failing tests:**
  - `to_mono_16k_downsamples_48k_stereo`: 4,800 stereo frames at 48 kHz, every sample 0.5, produce 1,600 ±1 samples, all 0.5.
  - `segmenter_splits_at_pause`: 1 s of a 0.3-amplitude 440 Hz sine, 1 s of zeros, 1 s of sine, then 1 s of zeros, fed in 480-sample chunks, return exactly two utterances, each between 16,000 and 32,000 samples long.
  - `segmenter_force_splits_at_max`: 30 s of continuous sine returns one utterance of 448,000 ±480 samples before the input ends.
  - `segmenter_ignores_silence`: 5 s of zeros returns nothing, and `!speaking()`.
  - `no_device_error_text`: `mic_error(MicError::NoDevice).0 == "No working microphone found."`.
- [ ] **Step 2: Run the tests.** Expected: failure.
- [ ] **Step 3: Implement** `to_mono_16k`, `Segmenter`, `mic_error` and `start_mic`. `start_mic` builds an f32 input stream on the default config with cpal and converts with `to_mono_16k` in the callback. It maps "no default input device" to `NoDevice`, and on Windows maps the access-denied backend error to `Denied`. Then add the `app.rs` wiring.
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** with the message `Add audio plumbing and VAD segmenter`.

### Task 11: Whisper model download

**Files:**
- Create: `src-tauri/src/stt/model.rs`, compiled on every OS so its tests run on the Mac.
- Modify: `src-tauri/src/app.rs` to implement `download_whisper_model`. It sets `session.download = Some(p)` as progress arrives and emits; at the end it sets `None`, and on error sets `error`.

**Interfaces:**
- Produces:
  - `pub struct ModelInfo { pub name: &'static str, pub sha256: &'static str }`
  - `pub const MODELS: [ModelInfo; 3]` for `tiny.en`, `base.en` and `small.en`. URLs are `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{name}.bin`.
  - `pub fn model_path(data_dir: &Path, name: &str) -> PathBuf`, which is `<data>/models/ggml-{name}.bin`
  - `pub async fn download(data_dir: &Path, name: &str, progress: impl Fn(f32)) -> Result<PathBuf>`
  - `fn finish(part: &Path, dest: &Path, expected_sha256: &str) -> Result<()>`

- [ ] **Step 1: Pin the checksums.** For each model, run `curl -L <url> | shasum -a 256` once and paste the hashes into `MODELS`.
- [ ] **Step 2: Write the failing tests:**
  - `checksum_mismatch_removes_part`: write `x.part` containing `"abc"`. `finish(part, dest, &"0".repeat(64))` returns an error containing "checksum", and both `part` and `dest` are missing afterwards.
  - `checksum_match_moves_file`: with the real sha256 of `"abc"` (`ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`), `dest` exists and `part` doesn't.
- [ ] **Step 3: Run the tests.** Expected: failure.
- [ ] **Step 4: Implement.**
  - `download` builds its own `reqwest::Client` with default redirects (Hugging Face redirects to its CDN), carrying no credentials. It streams `bytes_stream()` into `<dest>.part`, reporting `received / content_length` progress, then calls `finish`.
  - On any error, it removes the `.part` file and returns "Could not download the speech model ({detail}). Check your connection and try again."
  - The checksum error message is "The speech model download was corrupted (checksum mismatch). Try again."
- [ ] **Step 5: Run the tests.** Expected: PASS.
- [ ] **Step 6: Commit** with the message `Add Whisper model download`.

### Task 12: macOS: Apple Speech + ScreenCaptureKit

**Files:**
- Create: `src-tauri/src/stt/apple.rs` and `src-tauri/src/audio/macos.rs`
- Modify: the `src-tauri/Cargo.toml` macOS dependencies: `objc2`, `block2`, `objc2-foundation`, `objc2-speech`, `objc2-av-foundation`, `objc2-screen-capture-kit`, `objc2-core-media`, each with the features it needs.

**Interfaces:**
- Implements: `stt::Recognizer` (on macOS) and `audio::macos::start(sink: Sink) -> Result<Capture>` from Task 10.

- [ ] **Step 1: Implement `audio::macos::start`.** Port `AudioCapture.begin(system: true)` from `Audio.swift`.
  - Use `SCShareableContent` to get the first display. If none: "No display is available for Mac audio capture."
  - Configure `SCStreamConfiguration` with `capturesAudio`, `excludesCurrentProcessAudio`, `sampleRate` 16000, `channelCount` 1, a 2×2 frame and a minimum frame interval of 1 s.
  - Define an `SCStreamOutput` class (with `define_class!`) that copies the `CMSampleBuffer` audio buffer list (f32) into the sink.
  - The `didStopWithError` delegate calls an error callback that `start` stores. The capture then reports "Mac audio capture stopped: {desc}" through that capture's `error`.
- [ ] **Step 2: Implement `stt::apple::Recognizer`**, porting `AudioCapture.start`, `recognize()`, `waitForPause()`, `finishUtterance()` and `stop()`.
  - `start` requests speech authorization; on denial it returns "Allow Speech Recognition for Cue in System Settings → Privacy & Security."
  - On macOS, `start_mic` requests `AVCaptureDevice` audio access first. On denial it returns `MicError::Denied`, which uses the Swift Microphone message.
  - Use the `en-US` recognizer; if it's unavailable, use the Swift message.
  - Requests set `shouldReportPartialResults` and `addsPunctuation`, plus `requiresOnDeviceRecognition` when supported.
  - `push` wraps the PCM in an `AVAudioPCMBuffer` (16 kHz, mono, f32) and appends it to the current request, which sits behind a `Mutex`.
  - Each partial result emits `Partial` and restarts the 900 ms pause timer.
  - The pause timer firing, a final result, or the 50 s rollover timer each emit `Final`, then start a fresh request. A generation counter drops stale callbacks.
  - A recognition error emits `Error`.
- [ ] **Step 3: Verify on macOS.** `cargo test` stays green. Run `cargo tauri build --debug` and open `src-tauri/target/debug/bundle/macos/Cue.app`; the permission prompts need the bundle.
  - In person: speak, and partial text appears and then commits after a pause as INTERVIEWER.
  - Video call: play an interview clip. It appears as INTERVIEWER and your voice as ME, and a spoken question produces talking points with no button press.
  - Stop listening ends both captures.
- [ ] **Step 4: Commit** with the message `Add Apple Speech and ScreenCaptureKit capture`.

### Task 13: Windows: WASAPI loopback + Whisper rolling re-decode

**Files:**
- Create: `src-tauri/src/audio/windows.rs` and `src-tauri/src/stt/whisper.rs`
- Modify: `src-tauri/Cargo.toml`, adding `[target.'cfg(windows)'.dependencies] whisper-rs`

**Interfaces:**
- Implements: `stt::Recognizer` (on Windows) and `audio::windows::start(sink: Sink) -> Result<Capture>` from Task 10. Uses `Segmenter` (Task 10) and `model_path` (Task 11).

- [ ] **Step 1: Implement `audio::windows::start`.** Get cpal's WASAPI default **output** device and call `build_input_stream` with its default output config; on WASAPI this gives a loopback stream. Convert with `to_mono_16k` into the sink.
- [ ] **Step 2: Implement `stt::whisper::Recognizer`.**
  - `start` loads a `WhisperContext` from `model_path(data_dir, &settings.whisper_model)`. If the file is missing, it returns "Download the speech model in Setup first."
  - Spawn one worker thread that owns the context, a `Segmenter::new(VAD_THRESHOLD, PAUSE_MS, MAX_MS)` and an `mpsc::Receiver<Vec<f32>>`. Each loop iteration:
    1. Drain the channel into `segmenter.push`.
    2. When an utterance is returned, decode it and emit `Final` if the result is non-empty.
    3. Otherwise, if `speaking()` and `redecode_interval` seconds have passed since the last partial decode, decode `current()` and emit `Partial`.
    4. Sleep 50 ms.

    Decoding runs inline on this thread, so a slow decode delays the next tick instead of queueing work.
  - Decode parameters: `FullParams::new(SamplingStrategy::Greedy { best_of: 1 })`, `set_language(Some("en"))`, `set_no_context(true)`, `set_single_segment(true)`, and every `set_print_*` set to false. Join the segment texts and trim. Drop results that are empty or that match `^\[.*\]$` (for example `[BLANK_AUDIO]`).
  - `push` sends a copy on the channel. `stop` drops the sender and joins the thread.
- [ ] **Step 3: Verify on a Windows machine.**
  - Prerequisites: Rust, Node 20+, CMake, VS Build Tools with C++, and WebView2.
  - `cargo test --manifest-path src-tauri/Cargo.toml` is all green, `cargo tauri build` succeeds, and the `.msi` installs.
  - In Setup, download `base.en`.
  - Video call with a clip playing: INTERVIEWER partial text updates about once a second and commits after a pause, and your mic appears as ME.
  - Turn off mic access in Windows privacy settings: Start listening shows the microphone error, and system audio still transcribes.
  - With a call app running, the machine stays responsive.
- [ ] **Step 4: Commit** with the message `Add WASAPI loopback and Whisper streaming`.

### Task 14: README for the Rust app

**Files:**
- Create: `docs/rust-readme-draft.md`. It becomes `README.md` in Task 15, so the current README stays accurate until then.

- [ ] **Step 1: Write it** by adapting each section of the current `README.md` for the Rust app.
  - **Requirements:** macOS 14+ or Windows 10/11.
  - **Build:** prerequisites per OS, then `npm --prefix ui install` and `cargo tauri build`. Bundle output paths under `src-tauri/target/release/bundle/`.
  - **Setup, providers and local LLM:** keep the content, except that keys are saved "in Cue's settings file" on both OSes.
  - **Audio:** the macOS permissions table stays. For Windows: the microphone privacy setting, no permission needed for system audio, and the one-time speech model download.
  - **Data and privacy:** add the Windows paths. On Windows, speech is transcribed locally with Whisper; on macOS it uses Apple Speech.
  - **Troubleshooting:** add model download failures, high CPU (use `tiny.en` or a longer re-decode interval), and no system audio on Windows (check the default output device).
  - **Development:** `cargo test --manifest-path src-tauri/Cargo.toml`, plus a file table matching this plan's File Structure.
- [ ] **Step 2: Commit** with the message `Draft README for Rust app`.

### Task 15: Retire the Swift app (only after the user accepts the Rust app)

- [ ] **Step 1: Stop and ask the user** to confirm that the Rust app is accepted on macOS and Windows. Do not continue without an explicit yes.
- [ ] **Step 2: Tag the Swift app.** Run `git tag swift-final main` and confirm with `git show swift-final --stat | head`. `main` still has the Swift app because this branch hasn't been merged.
- [ ] **Step 3: Remove the Swift code.** Run `git rm -r Package.swift Sources Tests Info.plist scripts/build.sh Resources/Assets.xcassets Resources/Cue.icns`, then `git mv -f docs/rust-readme-draft.md README.md`.
- [ ] **Step 4: Check for leftovers.** `cargo test --manifest-path src-tauri/Cargo.toml` and `npm --prefix ui run build` both pass. `git grep -n "swift build\|Package.swift"` matches nothing outside `docs/superpowers/`.
- [ ] **Step 5: Commit** with the message `Retire Swift app (archived at tag swift-final)`. Push the tag only when the user asks.
