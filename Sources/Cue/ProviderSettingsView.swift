import SwiftUI

struct ProviderSettingsView: View {
    @AppStorage("llm.provider") private var provider: LLMProvider = .openai
    @AppStorage("llm.local.baseURL") private var localBaseURL = "http://localhost:11434/v1"
    @State private var model = ""
    @State private var key = ""
    @State private var keyDrafts: [LLMProvider: String] = [:]
    @State private var message = ""
    @State private var messageIsError = false
    @State private var testing = false
    @State private var testTask: Task<Void, Never>?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("AI PROVIDER").font(.caption.bold()).foregroundStyle(mint)
            Picker("Provider", selection: $provider) {
                ForEach(LLMProvider.allCases) { option in Text(option.label).tag(option) }
            }.pickerStyle(.menu).disabled(testing)
            HStack { Text("Model"); TextField(provider == .local ? "Model ID loaded in your local server" : provider.defaultModel, text: $model) }.disabled(testing)
            if provider == .local {
                HStack { Text("Base URL"); TextField("http://localhost:11434/v1", text: $localBaseURL) }.disabled(testing)
                Text("Ollama: http://localhost:11434/v1\nLM Studio: http://localhost:1234/v1\nStart the server and enter a model you have downloaded. Include /v1 in the URL.").font(.caption).foregroundStyle(.secondary)
            }
            HStack {
                SecureField(provider.requiresKey ? "\(provider.label) API key" : "Server API key (optional)", text: $key)
                Button("Save key") {
                    do {
                        let value = key.trimmingCharacters(in: .whitespacesAndNewlines)
                        try Credentials.save(value, for: provider)
                        key = value; keyDrafts[provider] = value
                        message = value.isEmpty ? "Key removed for \(provider.label)." : "\(provider.label) key saved in Cue."
                        messageIsError = false
                    } catch { show(error) }
                }
            }.disabled(testing)
            HStack {
                Button(testing ? "Testing…" : "Test connection") { testConnection() }.disabled(testing)
                if testing { ProgressView().controlSize(.small); Button("Cancel") { testTask?.cancel() } }
                Text("Sends a short test prompt; no résumé or transcript.").font(.caption).foregroundStyle(.secondary)
            }
            if !message.isEmpty { Text(message).font(.caption).foregroundStyle(messageIsError ? .orange : mint).textSelection(.enabled) }
            Text(provider == .local
                 ? "Talking points go to the local server you configure. Keys are optional unless your server requires one. Speech recognition may still use Apple’s service."
                 : "Talking points send your experience, role, question, and recent transcript to \(provider.label). Use a provider API key; keys are saved on this Mac in Cue’s settings. Audio is not sent to this provider.")
                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            Text("Provider and model settings save automatically. Click Save key after entering a key; Test connection can use the unsaved key above.").font(.caption).foregroundStyle(.secondary)
        }
        .onAppear { loadProvider() }
        .onChange(of: provider) { old, _ in
            keyDrafts[old] = key
            message = ""; loadProvider()
        }
        .onChange(of: model) { _, value in UserDefaults.standard.set(value, forKey: provider.modelKey) }
        .onDisappear { testTask?.cancel() }
    }

    private func loadProvider() {
        model = UserDefaults.standard.string(forKey: provider.modelKey) ?? provider.defaultModel
        if let draft = keyDrafts[provider] { key = draft; return }
        do { key = try Credentials.read(for: provider) }
        catch { key = ""; show(error) }
    }
    private func show(_ error: Error) { message = error.localizedDescription; messageIsError = true }
    private func testConnection() {
        testing = true; message = ""
        let configuration = LLMConfiguration(provider: provider, model: model, localBaseURL: localBaseURL)
        let testKey = key
        testTask = Task {
            defer { testing = false }
            do {
                _ = try await AssistantAPI.generate(configuration: configuration, key: testKey, instructions: "Reply briefly.", input: "Reply with OK to confirm this connection works.")
                try Task.checkCancellation()
                message = "Connected to \(configuration.provider.label) using \(configuration.model)."; messageIsError = false
            } catch {
                if Task.isCancelled { message = "Connection test canceled."; messageIsError = false }
                else { show(error) }
            }
        }
    }
}
