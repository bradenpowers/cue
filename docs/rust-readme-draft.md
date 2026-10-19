# Cue

Cue is an interview assistant for macOS and Windows. It transcribes audio from your computer or microphone and generates talking points using your résumé, the role you’re applying for, and the conversation so far.

Built with Rust, [Tauri 2](https://tauri.app), and a React interface. Speech recognition uses Apple Speech on macOS and a local [Whisper](https://github.com/ggml-org/whisper.cpp) model on Windows. You choose OpenAI, Anthropic, Google Gemini, or a local LLM server for talking points. No backend server is required.

Windows support is new. Cue started as a macOS-only Swift app, and the Windows build has not had the same mileage; expect rough edges and please report them.

## What you can do

- Transcribe the other side of a call using **Mac audio** (macOS) or your computer’s audio (Windows), or listen through your **microphone**.
- Import a résumé and review the extracted text before saving it as context.
- Select ChatGPT / OpenAI, Claude / Anthropic, Gemini / Google, or a local LLM.
- Get concise talking points automatically when Cue hears a question, or request them with a button or keyboard shortcut.
- Optionally add a TypeSafe key so Cue can classify questions and send only the relevant résumé lines.
- Keep suggestions nearby in a floating assistant window.
- Take notes, review past interviews, and export the transcript, suggestions, and notes to Markdown.

## Requirements

- **macOS 14 or later**, or **Windows 10 or 11**.
- An **API key for your selected cloud provider**, or a running **local OpenAI-compatible LLM server**, to generate talking points.
- An internet connection for cloud AI suggestions. On macOS, speech recognition may also need a connection when on-device recognition is unavailable. On Windows, a one-time download of the speech model needs a connection; after that, transcription is offline.

You can open the app, import your résumé, and use transcription without an AI API key. Cloud talking points require the selected provider’s key; local servers only need a key if their authentication is enabled.

## Build and open Cue

### 1. Install the development tools

Both platforms need [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) with npm, and the Tauri CLI:

```sh
cargo install tauri-cli --version "^2"
```

**macOS:** install Xcode or the Command Line Tools:

```sh
xcode-select --install
```

**Windows:**

- Microsoft C++ Build Tools (the “Desktop development with C++” workload).
- Microsoft Edge WebView2 (preinstalled on current Windows 10 and 11).
- [CMake](https://cmake.org/download/) and [LLVM](https://releases.llvm.org) (for `libclang`), both on your `PATH`. They are needed to compile the Whisper library.

See [Tauri’s prerequisites](https://tauri.app/start/prerequisites/) for details.

### 2. Get the source

Clone this repository or download and extract its source ZIP. In a terminal, change to the project folder, the one containing `src-tauri/` and `ui/`.

### 3. Build the app

```sh
npm --prefix ui install
cargo tauri build
```

The build bundles the interface and compiles the app in release mode. Installers and app bundles are written under `src-tauri/target/release/bundle/`:

| OS | Output |
| --- | --- |
| macOS | `macos/Cue.app` and `dmg/` |
| Windows | `msi/` and `nsis/` installers |

On macOS, open `src-tauri/target/release/bundle/macos/Cue.app` rather than the bare executable, so macOS can use the app’s permission descriptions. These are local development builds, not signed or notarized downloads for distribution.

To run with hot reload while developing, use `cargo tauri dev` from the project folder.

## First-time setup

On first launch, Cue shows a short **Getting started** guide. You can reopen it from **Getting started** in the sidebar.

### Add your résumé and role

1. Open **Context & setup** in the sidebar.
2. Click **Import résumé** and select a **PDF, DOCX, RTF, or plain-text** file.
3. Review the extracted text and fix any formatting or reading-order issues.
4. Choose **Use résumé**. If you already have experience saved, choose **Replace experience** or **Add to experience** instead.
5. Paste the job description and relevant company details into **Role & company**.

You can also paste or edit your experience directly. Experience and role edits save automatically and are included in future talking-point requests. Canceling the import preview leaves existing context unchanged.

Files must be no larger than 10 MB, with no more than 100,000 extracted characters. Import is local; the original file is not uploaded or modified. Scanned PDFs need a searchable text layer, and password-protected PDFs must be unlocked before import. Review multi-column PDFs carefully because their extracted reading order can differ from their visual layout.

### Choose your AI provider

In **Context & setup**, scroll to **AI provider**:

1. Choose **ChatGPT / OpenAI**, **Claude / Anthropic**, **Gemini / Google**, or **Local LLM** from the provider dropdown.
2. Enter the model ID you want to use. Cloud providers start with a suggested model ID; you can edit it to any compatible text model available to your account.
3. Enter the matching provider’s API key and click **Save key**. Each provider has a separate entry in Cue’s settings file.
4. Click **Test connection** to check the model and key. It sends a small test prompt using the key currently in the field, without your résumé or transcript. Cloud providers may charge for this request. Testing does not save an unsaved key.

| Selection | Connection | Model setting |
| --- | --- | --- |
| ChatGPT / OpenAI | OpenAI Responses API | Starts with `gpt-4.1-mini` |
| Claude / Anthropic | Anthropic Messages API | Starts with `claude-sonnet-4-6` |
| Gemini / Google | Gemini Generate Content API | Starts with `gemini-3.8-flash` |
| Local LLM | Local OpenAI-compatible Chat Completions API | Enter the exact model ID served by your local server |

Provider, model, and local URL settings save automatically. Each provider remembers its own model and saved key. Selecting a different provider changes the destination for future talking-point requests; requests already running keep their original provider.

Use a provider API credential, not your chat account password. Cue calls provider APIs directly; it does not sign into their chat apps. You do not need an `.env` file or a key in the source code. To remove a provider’s saved key, select that provider, clear its key field, and click **Save key**.

Cloud requests use your own provider account. If a request reports a billing, quota, or model-access error, resolve that in the provider account or select a model you can access.

#### Connect a local model

Cue connects to an existing local server; it does not install a model or start a server for you.

- **Ollama:** start Ollama, download a model, and use `http://localhost:11434/v1` as the base URL. Enter the exact downloaded model ID, including its tag. See [Ollama’s OpenAI compatibility documentation](https://docs.ollama.com/api/openai-compatibility).
- **LM Studio:** load a model and start its server in the Developer tab. Use `http://localhost:1234/v1` unless you configured a different port, and enter the model ID served by that instance. See [LM Studio’s API server documentation](https://lmstudio.ai/docs/developer/core/server).

Select **Local LLM**, enter the base URL and model ID, and leave the key empty unless your server requires one. Include `/v1` in the base URL, not `/chat/completions`; Cue appends the Chat Completions path. Click **Test connection** before requesting talking points.

Local URLs must use `localhost`, `127.0.0.1`, or `::1`, with HTTP or HTTPS, and no credentials or query parameters. Remote and LAN model servers are not supported by this setting. There is no automatic fallback to a cloud provider. Local models can take longer to load; Cue allows up to 180 seconds for a local request (90 seconds for cloud providers). On macOS, speech recognition may still use Apple’s service even when your talking-point model runs locally.

#### TypeSafe (optional)

**TypeSafe (optional)** in **Context & setup** takes a [TypeSafe](https://typesafe.ai) API key. With a key, Cue asks TypeSafe what kind of question was asked and which résumé lines fit it, so your AI provider receives only the relevant experience, and Cue can catch questions that don’t sound like questions. This sends your role, résumé lines, and recent transcript to TypeSafe. Without a key, Cue spots most questions by how they’re phrased and sends your whole résumé with each request. Use **Test key** to check it.

### Windows: download the speech model

On Windows, Cue transcribes speech locally with Whisper. In **Context & setup**, scroll to **SPEECH RECOGNITION**:

1. Choose a **Speech model**: `tiny.en`, `base.en` (the default), or `small.en`. Larger models are more accurate and use more CPU.
2. Click **Download model**. Cue downloads the model once from Hugging Face (about 75 MB, 140 MB, and 470 MB respectively), verifies its checksum, and stores it in Cue’s data folder.
3. Adjust **Re-decode interval (seconds)** (0.5 to 3, default 1) if needed. Cue re-transcribes the sentence in progress at this interval to show live text; smaller models and longer intervals use less CPU.

This section does not appear on macOS, which uses Apple Speech.

### Allow audio access

Choose a source in the live workspace, then click **Start listening**.

- **Video call** listens to your computer’s audio as the interviewer, and your microphone as you. Use headphones so your microphone hears only you.
- **In person** uses the microphone alone and treats everything it hears as the interviewer.

**macOS** asks for the permissions required by that source:

| Permission | When it is used |
| --- | --- |
| Speech Recognition | Transcribing either audio source |
| Microphone | Capturing your voice or nearby speakers |
| Screen & System Audio Recording | Capturing audio played by other Mac apps |

Manage these permissions in **System Settings → Privacy & Security**. The recording category’s wording can vary by macOS version. Quit and reopen Cue if macOS requests a restart after granting access.

**Windows** needs no permission for computer audio, which Cue captures with WASAPI loopback from your default output device. The microphone is governed by **Settings → Privacy & security → Microphone**; make sure microphone access is allowed for desktop apps.

## Use Cue during a call

1. Choose **Video call** to hear the interviewer through your call app, including when you use headphones. Choose **In person** to use the microphone alone.
2. Click **Start listening** and check that text appears in **Live transcript**.
3. When the interviewer asks a question, Cue can suggest talking points on its own. To ask for help yourself, leave the question field empty to use the latest question in the transcript, or type a specific question or follow-up, then click **Suggest talking points** or press **Ctrl/⌘+Enter**.
4. Open the floating assistant from the sidebar (**Open assistant**) or Cue’s tray (menu-bar) item to keep suggestions visible over other windows.
5. Click **Stop listening** when finished. Use the export button at the top right to save the session.

Clear a previously typed question when you want Cue to use the latest transcript question again.

| Action | Shortcut or control |
| --- | --- |
| Request talking points | **⌘Enter** on macOS, **Ctrl+Enter** on Windows |
| Toggle floating assistant | **⌘⇧J** on macOS, **Ctrl+Shift+J** on Windows |
| Open assistant or workspace while using another app | Cue’s tray item: **Floating assistant**, **Show workspace**, **Quit Cue** |
| Export session | Export icon at the top right |
| Start a new session | **+** at the top right, then confirm |
| Review earlier sessions | **Past interviews** in the sidebar |

Keyboard shortcuts work only while Cue is focused; they are not global shortcuts. Closing a window hides it and Cue keeps running in the tray; choose **Quit Cue** from the tray item to exit. Interviews save automatically, so you can review them later in **Past interviews**, but export a session if you want a Markdown copy.

## Data and privacy

Cue keeps its data in one folder:

| OS | Data folder |
| --- | --- |
| macOS | `~/Library/Application Support/Cue` |
| Windows | `%APPDATA%\Cue` |

| Data | Where it goes |
| --- | --- |
| API keys (including TypeSafe) | `keys.json` in the data folder (readable only by you on macOS). Only the selected provider’s key is used |
| Saved experience, role, provider, model preferences, local URL, speech settings | `settings.json` in the data folder |
| Past interviews (transcript, suggestions, notes) | One JSON file each in the `Interviews` folder, saved automatically; delete them in **Past interviews** |
| Whisper speech models (Windows) | `models` folder in the data folder |
| Imported résumé file | Read locally to extract text; the original file is not uploaded |
| Audio on macOS | Apple Speech recognition, preferably on-device; Apple’s service may process it when on-device recognition is unavailable |
| Audio on Windows | Transcribed locally with Whisper; never leaves your PC |
| Talking-point request | Experience, role, question, and the last 12,000 transcript characters are sent to the selected provider or local server |

Keys are stored as plain text in Cue’s settings file, not in the macOS Keychain or Windows Credential Manager. Protect your user account accordingly. If you used the earlier Swift version of Cue, your experience, role, provider, and model settings are copied from its preferences on first launch; re-enter API keys, because Keychain items are not migrated.

For OpenAI, Cue sends Responses API requests with `store: false`; this is not a guarantee of zero provider retention. Each cloud provider’s data policies apply to requests sent to that provider. If you add a TypeSafe key, TypeSafe’s policies apply to the data described above. Cue does not send audio to the selected LLM, process screenshots, or save audio recordings. Computer audio captures all sound your computer plays, not just one selected call app. The only other network request Cue makes is the one-time Whisper model download on Windows.

## Troubleshooting

| Problem | What to check |
| --- | --- |
| `cargo tauri` is not found | Run `cargo install tauri-cli --version "^2"`. |
| Windows build fails compiling Whisper | Install CMake and LLVM, add both to `PATH`, and open a new terminal. |
| Changes are not showing up | Quit the running app (use **Quit Cue** in the tray), rebuild, and open the newly built app. |
| Computer audio produces no transcript on macOS | Check recording and Speech Recognition permissions, confirm that the call app is playing audio, then restart Cue if permissions changed. |
| No system audio on Windows | Cue records the default output device. Make sure the call app plays through it, and that it is not muted or switched to another device after Cue started; then stop and start listening again. |
| Microphone produces no transcript | Check Microphone permission (and Speech Recognition on macOS) and your computer’s selected sound input. |
| Speech recognition is unavailable or stops (macOS) | Check your connection and macOS speech settings, then click **Start listening** again. |
| The speech model download fails (Windows) | Check your internet connection and click **Download model** again. A corrupted download is detected by checksum and discarded. |
| Transcription uses a lot of CPU (Windows) | Choose `tiny.en` as the **Speech model**, or raise the **Re-decode interval** in **Context & setup**. |
| Talking points report a missing key | Select the intended provider and save its key in **Context & setup**; typing it or testing it without clicking **Save key** does not save it. |
| Local connection fails | Start the local server, load/download the specified model, and check the localhost URL and port. Include `/v1`. |
| Talking points report a service error | Check the displayed error, account quota/billing, model access, and internet connection. |
| Résumé import finds no text | Use a searchable PDF, DOCX, RTF, or UTF-8/UTF-16 text file. Scanned PDFs require OCR elsewhere first. |

## Current limitations

- English transcription only.
- Windows support is new and has had limited testing.
- No speaker identification beyond the source: computer audio is the interviewer, the microphone is you (or everyone, in person mode). Question detection is a phrasing heuristic unless you add a TypeSafe key.
- On macOS, speech sessions rotate approximately every 50 seconds; brief gaps can occur during rotation.
- No OCR for scanned résumés and no global keyboard shortcuts.

## Development

Run the Rust tests from the project folder:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
```

Check the interface build with `npm --prefix ui run build`. Tests cover provider request construction, response parsing, key storage, settings, session and transcript logic, and résumé import, including supported formats and invalid files. Live audio capture, permissions, speech recognition, and real API requests require manual testing on each OS.

| File | Purpose |
| --- | --- |
| `src-tauri/src/app.rs` | App state, commands, windows, tray, and suggestion flow |
| `src-tauri/src/session.rs`, `transcript.rs` | Session lines, question detection, and transcript formatting |
| `src-tauri/src/ai.rs` | Provider configuration, request building, and API calls |
| `src-tauri/src/typesafe.rs` | Optional TypeSafe question classification and résumé line selection |
| `src-tauri/src/store.rs` | Settings, API keys, and past interviews on disk |
| `src-tauri/src/resume.rs` | Local résumé text extraction |
| `src-tauri/src/audio/` | Microphone and computer audio capture (ScreenCaptureKit on macOS, WASAPI loopback on Windows) |
| `src-tauri/src/stt/` | Speech recognition: Apple Speech on macOS, Whisper with voice activity detection and model download on Windows |
| `src-tauri/tauri.conf.json` | Windows, bundle targets, and app identifier |
| `src-tauri/Info.plist` | macOS permission descriptions |
| `ui/src/` | React interface: workspace, setup, floating assistant, and settings |

Generated `src-tauri/target/`, `ui/dist/`, and `ui/node_modules/` folders are ignored by Git. Never add API keys to source files or commits.

Cue retains its original bundle identifier, `local.butler.desktop`, so settings from its earlier name remain accessible.

Implementation references: [Tauri](https://tauri.app), [whisper.cpp](https://github.com/ggml-org/whisper.cpp), [Apple Speech](https://developer.apple.com/documentation/speech/sfspeechrecognizer), [ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit), [OpenAI text generation](https://developers.openai.com/api/docs/guides/text), [Claude Messages](https://platform.claude.com/docs/en/api/messages/create), and [Gemini Generate Content](https://ai.google.dev/api/generate-content).
