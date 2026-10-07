import SwiftUI

extension LLMProvider {
    var keyURL: URL? {
        switch self {
        case .openai: return URL(string: "https://platform.openai.com/api-keys")
        case .claude: return URL(string: "https://console.anthropic.com/settings/keys")
        case .gemini: return URL(string: "https://aistudio.google.com/app/apikey")
        case .local: return nil
        }
    }
}

struct WelcomeView: View {
    @Environment(\.dismiss) var dismiss
    @AppStorage("llm.provider") private var provider: LLMProvider = .openai
    @State private var step = 0
    @State private var importing = false
    @State private var warning = ""
    private let titles = ["Welcome to Cue", "Your experience", "Connect your AI", "Sharper answers (optional)", "You're ready"]

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                ForEach(titles.indices, id: \.self) { i in Capsule().fill(i <= step ? mint : .white.opacity(0.12)).frame(height: 4) }
            }
            Text("STEP \(step + 1) OF \(titles.count)").font(.system(size: 10, weight: .bold)).tracking(2).foregroundStyle(mint)
            Text(titles[step]).font(.system(size: 26, weight: .medium))
            ScrollView { page.frame(maxWidth: .infinity, alignment: .leading) }
            if !warning.isEmpty { Text(warning).font(.callout).foregroundStyle(.orange) }
            HStack {
                if step > 0 { Button("Back") { warning = ""; step -= 1 }.disabled(importing) }
                Spacer()
                if step == 2 && !warning.isEmpty { Button("Skip for now") { warning = ""; step += 1 } }
                if step == 3 { Button("Skip") { step += 1 } }
                Button(step == titles.count - 1 ? "Start using Cue" : "Continue") { next() }
                    .buttonStyle(.borderedProminent).tint(mint).foregroundStyle(.black).keyboardShortcut(.defaultAction).disabled(importing)
            }
        }.padding(28).frame(width: 620, height: 680).interactiveDismissDisabled(importing)
    }

    @ViewBuilder private var page: some View {
        switch step {
        case 0:
            VStack(alignment: .leading, spacing: 16) {
                Text("Cue listens to your interview and, when you're asked a question, puts short talking points from your own experience in front of you.").lineSpacing(5)
                point("waveform", "Listens live", "Hears the call, knows which voice is yours and which is the interviewer's.")
                point("sparkles", "Answers on its own", "When the interviewer asks a question, talking points appear within seconds. No buttons.")
                point("lock", "Stays on your Mac", "Your résumé, keys, and saved interviews are stored only on this Mac. Text is sent only to the AI services you choose.")
                Text("Setup takes about two minutes. You'll need an API key from an AI provider.").foregroundStyle(.secondary).padding(.top, 6)
            }
        case 1:
            VStack(alignment: .leading, spacing: 12) {
                Text("Cue grounds every answer in your real experience, so it never invents jobs or results. Import your résumé and describe the role you're interviewing for.").foregroundStyle(.secondary).lineSpacing(4)
                ExperienceSettingsView(importing: $importing)
            }
        case 2:
            VStack(alignment: .leading, spacing: 14) {
                Text("Cue uses an AI provider to write your talking points. You need your own API key. It's separate from a ChatGPT, Claude, or Gemini chat subscription, and the provider bills you based on how much you use it.").foregroundStyle(.secondary).lineSpacing(4)
                VStack(alignment: .leading, spacing: 6) {
                    Text("HOW TO GET A KEY").font(.caption.bold()).foregroundStyle(mint)
                    if let url = provider.keyURL {
                        Text("1. Choose your provider below.\n2. Open \(provider.label)'s key page, sign in, and create a new key.\n3. Paste it below, click **Save key**, then **Test connection**.").lineSpacing(4)
                        Link("Get a \(provider.label) key ↗", destination: url).foregroundStyle(mint)
                    } else {
                        Text("Start Ollama or LM Studio, load a model, and enter its model ID and server URL below. A key is only needed if your server requires one.").lineSpacing(4)
                    }
                }.padding(14).background(.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 10))
                ProviderSettingsView()
            }
        case 3:
            VStack(alignment: .leading, spacing: 14) {
                Text("Without TypeSafe, Cue spots most questions by how they're phrased and sends your whole résumé with each one. With a TypeSafe key, Cue also catches questions that don't sound like questions, and sends only the résumé lines that fit, which gives tighter answers.").foregroundStyle(.secondary).lineSpacing(4)
                Link("Get a TypeSafe key at typesafe.ai ↗", destination: URL(string: "https://typesafe.ai")!).foregroundStyle(mint)
                TypeSafeSettingsView()
            }
        default:
            VStack(alignment: .leading, spacing: 16) {
                point("video", "On a video call", "Choose **Video call**, then **Start listening**. Your mic is you; Mac audio is the interviewer. Wear headphones so your mic hears only you.")
                point("person.2", "In person", "Choose **In person**. One microphone hears the room, so Cue treats everything it hears as the interviewer.")
                point("hand.raised", "Permissions", "The first time, macOS asks to allow Speech Recognition, Microphone, and Screen & System Audio Recording. Allow all three. Cue captures audio only.")
                point("rectangle.on.rectangle", "Keep answers in view", "Press ⌘ ⇧ J for a floating window that stays on top of your call.")
                Text("You can change any of this later in **Context & setup**, or reopen this guide from **Getting started**.").foregroundStyle(.secondary).padding(.top, 6)
            }
        }
    }

    private func point(_ icon: String, _ title: String, _ detail: LocalizedStringKey) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: icon).foregroundStyle(mint).frame(width: 22)
            VStack(alignment: .leading, spacing: 3) { Text(title).font(.system(size: 14, weight: .semibold)); Text(detail).foregroundStyle(.secondary).lineSpacing(3) }
        }
    }

    private func next() {
        warning = ""
        // Talking points can't work without a saved provider key, so check before moving on.
        if step == 2, provider.requiresKey, ((try? Credentials.read(for: provider)) ?? "").isEmpty {
            warning = "Paste your \(provider.label) key and click Save key to continue."; return
        }
        if step == titles.count - 1 { dismiss() } else { step += 1 }
    }
}
