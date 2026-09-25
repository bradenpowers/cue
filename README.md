# Cue

Cue is a native macOS interview assistant. It transcribes audio from your Mac or microphone and generates talking points using your résumé, the role you’re applying for, and the conversation so far.

Built with SwiftUI, Apple Speech, ScreenCaptureKit, and your choice of OpenAI, Anthropic, Google Gemini, or a local LLM server. No third-party package dependencies or backend server are required.

## What you can do

- Transcribe the other side of a call using **Mac audio**, or listen through your **Microphone**.
- Import a résumé and review the extracted text before saving it as context.
- Select ChatGPT / OpenAI, Claude / Anthropic, Gemini / Google, or a local LLM.
- Request concise talking points with a button or keyboard shortcut.
- Keep suggestions nearby in a floating assistant window.
- Take notes and export the transcript, suggestions, and notes to Markdown.

## Requirements

- **macOS 14 or later.**
- **Swift 5.9 or later** and the macOS SDK, available through Xcode or Apple’s Command Line Tools.
- An **API key for your selected cloud provider**, or a running **local OpenAI-compatible LLM server**, to generate talking points.
- An internet connection for cloud AI suggestions. Speech recognition may also need a connection when on-device recognition is unavailable.

You can open the app, import your résumé, and use transcription without an AI API key. Cloud talking points require the selected provider’s key; local servers only need a key if their authentication is enabled.

## Build and open Cue

### 1. Install the development tools

If you don’t already have Xcode or Command Line Tools installed, run:

```sh
xcode-select --install
```

Finish the installer, then check that Swift is available:

```sh
swift --version
```

### 2. Get the source

Clone this repository or download and extract its source ZIP. In Terminal, change to the project folder—the folder containing `Package.swift` and `scripts/`.

### 3. Build the app

Run from the project folder:

```sh
zsh scripts/build.sh
open dist/Cue.app
```

The script builds a release executable and packages it as **`dist/Cue.app`**, signed locally for development. Launch this app bundle rather than using `swift run`, so macOS can use the app’s permission descriptions.

To rebuild after changing or updating the code, quit Cue, run the same build command, and reopen the app. This is a local development build, not a notarized download for distribution.

## First-time setup

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
3. Enter the matching provider’s API key and click **Save key**. Each provider has a separate entry in **macOS Keychain**.
4. Click **Test connection** to check the model and key. It sends a small test prompt using the key currently in the field, without your résumé or transcript. Cloud providers may charge for this request. Testing does not save an unsaved key.

| Selection | Connection | Model setting |
| --- | --- | --- |
| ChatGPT / OpenAI | OpenAI Responses API | Starts with `gpt-4.1-mini`; existing OpenAI model settings are retained |
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

Local URLs must use `localhost`, `127.0.0.1`, or `::1`, with HTTP or HTTPS. Remote and LAN model servers are not supported by this setting. There is no automatic fallback to a cloud provider. Local models can take longer to load; Cue allows up to 180 seconds for a local request. Speech recognition may still use Apple’s service even when your talking-point model runs locally.

### Allow audio access

Choose an audio source in the live workspace, then click **Start listening**. Cue asks for the permissions required by that source:

| Permission | When it is used |
| --- | --- |
| Speech Recognition | Transcribing either audio source |
| Microphone | Capturing your voice or nearby speakers |
| Screen & System Audio Recording | Capturing audio played by other Mac apps |

Manage these permissions in **System Settings → Privacy & Security**. The recording category’s wording can vary by macOS version. Quit and reopen Cue if macOS requests a restart after granting access.

## Use Cue during a call

1. Select **Mac audio** to hear the interviewer through your call app, including when you use headphones. Select **Microphone** to hear your voice and nearby speakers instead.
2. Click **Start listening** and check that text appears in **Live transcript**.
3. Leave the question field empty to request help with the latest question in the transcript, or type a specific question or follow-up.
4. Click **Suggest talking points** or press **⌘Return**.
5. Open the floating assistant from the sidebar or Cue’s menu-bar item to keep suggestions visible over other windows.
6. Click **Stop listening** when finished. Use the export button at the top right to save the session.

Suggestions are generated on request, not automatically for every transcript update. Clear a previously typed question when you want Cue to use the latest transcript question again.

| Action | Shortcut or control |
| --- | --- |
| Request talking points | **⌘Return** |
| Toggle floating assistant | **⌘⇧J** |
| Open assistant while using another app | Cue’s menu-bar item |
| Export session | Share/export icon at the top right |
| Start a new session | **+** at the top right, then confirm |

Keyboard shortcuts work while Cue is active; they are not global shortcuts. Export before quitting or starting a new session if you want to keep the transcript, notes, and suggestions.

## Data and privacy

| Data | Where it goes |
| --- | --- |
| API keys | Separate macOS Keychain entries; only the selected provider’s key is used |
| Saved experience, role, provider, model preferences, local URL | Local macOS UserDefaults |
| Imported résumé file | Read locally to extract text; the original file is not uploaded |
| Transcript, suggestions, notes | Kept in memory unless you explicitly export them |
| Audio | Apple Speech recognition, preferably on-device; Apple’s service may process it when on-device recognition is unavailable |
| Talking-point request | Experience, role, question, and the last 12,000 transcript characters are sent to the selected provider or local server |

For OpenAI, Cue sends Responses API requests with `store: false`; this is not a guarantee of zero provider retention. Each cloud provider’s data policies apply to requests sent to that provider. Cue does not send audio to the selected LLM, process screenshots, or save audio recordings. Mac audio captures other applications’ sound, not just one selected call app.

## Troubleshooting

| Problem | What to check |
| --- | --- |
| `swift` is missing or the SDK cannot be found | Finish installing Command Line Tools or Xcode and check `swift --version` and `xcode-select -p`. |
| Build errors mention the old project path after moving the folder | Run `swift package clean`, then rebuild with `zsh scripts/build.sh`. |
| Changes are not showing up | Quit the running app, rebuild, and open `dist/Cue.app` from the current project folder. |
| Mac audio produces no transcript | Check recording and Speech Recognition permissions, confirm that the call app is playing audio, then restart Cue if permissions changed. |
| Microphone produces no transcript | Check Microphone and Speech Recognition permissions and your Mac’s selected sound input. |
| Speech recognition is unavailable or stops | Check your connection and macOS speech settings, then click **Start listening** again. |
| Talking points report a missing key | Select the intended provider and save its key in **Context & setup**; typing it or testing it without clicking **Save key** does not save it. |
| Local connection fails | Start the local server, load/download the specified model, and check the localhost URL and port. Include `/v1`. |
| Talking points report a service error | Check the displayed error, account quota/billing, model access, and internet connection. |
| Résumé import finds no text | Use a searchable PDF, DOCX, RTF, or UTF-8/UTF-16 text file. Scanned PDFs require OCR elsewhere first. |

## Current limitations

- English transcription only.
- One audio source at a time; no simultaneous microphone and Mac audio capture.
- No speaker identification or automatic question detection.
- Speech sessions rotate approximately every 50 seconds; brief gaps can occur during rotation.
- No OCR for scanned résumés, global keyboard shortcuts, or automatic session history.

## Development

Run the tests from the project folder:

```sh
swift test --disable-sandbox
```

Tests cover provider request construction, response parsing, credential separation, settings compatibility, and résumé import, including supported formats and invalid files. Live audio capture, permissions, and real API requests require manual testing on a Mac.

| File | Purpose |
| --- | --- |
| `Sources/Cue/CueApp.swift` | Workspace, setup, import preview, floating assistant, and session state |
| `Sources/Cue/Audio.swift` | Microphone/Mac audio capture and speech recognition |
| `Sources/Cue/AI.swift` | Provider configuration, API adapters, and Keychain access |
| `Sources/Cue/ProviderSettingsView.swift` | Provider dropdown, model/key settings, and connection testing |
| `Sources/Cue/ResumeImporter.swift` | Local résumé text extraction |
| `Info.plist` | App metadata and permission descriptions |
| `scripts/build.sh` | Release build and app packaging |

Generated `.build/` and `dist/` folders are ignored by Git. Never add API keys to source files or commits.

Cue retains its original internal bundle and Keychain identifier, `local.butler.desktop`, so saved context, settings, and API keys from its earlier name remain accessible.

Implementation references: [Apple Speech](https://developer.apple.com/documentation/speech/sfspeechrecognizer), [ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit), [OpenAI text generation](https://developers.openai.com/api/docs/guides/text), [Claude Messages](https://platform.claude.com/docs/en/api/messages/create), and [Gemini Generate Content](https://ai.google.dev/api/generate-content).
