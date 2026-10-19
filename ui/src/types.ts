export type LLMProvider = 'openai' | 'claude' | 'gemini' | 'local'

export const providers: LLMProvider[] = ['openai', 'claude', 'gemini', 'local']

// Must match LLMProvider::label / default_model / key_url in src-tauri/src/ai.rs.
export const providerLabels: Record<LLMProvider, string> = {
  openai: 'ChatGPT / OpenAI',
  claude: 'Claude / Anthropic',
  gemini: 'Gemini / Google',
  local: 'Local LLM',
}

export const defaultModels: Record<LLMProvider, string> = {
  openai: 'gpt-4.1-mini',
  claude: 'claude-sonnet-4-6',
  gemini: 'gemini-3.8-flash',
  local: '',
}

export const keyUrls: Partial<Record<LLMProvider, string>> = {
  openai: 'https://platform.openai.com/api-keys',
  claude: 'https://console.anthropic.com/settings/keys',
  gemini: 'https://aistudio.google.com/app/apikey',
}

export interface Line {
  id: string
  speaker: 'me' | 'them'
  text: string
  start: string
}

export interface CaptureView {
  running: boolean
  starting: boolean
  error: string
}

export interface Snapshot {
  lines: Line[]
  them: CaptureView
  me: CaptureView
  answer: string
  busy: boolean
  error: string
  download: number | null
}

export interface Settings {
  provider: LLMProvider
  models: Record<string, string>
  localBaseUrl: string
  resume: string
  role: string
  welcomed: boolean
  whisperModel: string
  redecodeInterval: number
}

export interface Interview {
  id: string
  date: string
  role: string
  transcript: string
  talkingPoints: string[]
  notes: string
}

export interface ImportedResume {
  filename: string
  text: string
}

export const modelFor = (s: Settings, p: LLMProvider) => s.models[p] ?? defaultModels[p]
