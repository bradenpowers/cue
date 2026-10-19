import { openUrl } from '@tauri-apps/plugin-opener'
import { useState, type ReactNode } from 'react'
import { isMac, keys, readKey } from '../api'
import { keyUrls, providerLabels, type Settings } from '../types'
import ExperienceSettings from './ExperienceSettings'
import Modal from './Modal'
import ProviderSettings from './ProviderSettings'
import TypeSafeSettings from './TypeSafeSettings'
import type { UpdateSettings } from './Workspace'

const titles = ['Welcome to Cue', 'Your experience', 'Connect your AI', 'Sharper answers (optional)', "You're ready"]

function Point({ icon, title, children }: { icon: string; title: string; children: ReactNode }) {
  return (
    <div className="point">
      <span className="icon mint">{icon}</span>
      <div className="stack-tight">
        <div className="strong">{title}</div>
        <div className="muted">{children}</div>
      </div>
    </div>
  )
}

function Link({ href, children }: { href: string; children: ReactNode }) {
  return <button className="link" onClick={() => openUrl(href)}>{children}</button>
}

export default function Welcome({ settings, update }: { settings: Settings; update: UpdateSettings }) {
  const [step, setStep] = useState(0)
  const [importing, setImporting] = useState(false)
  const [warning, setWarning] = useState('')
  const provider = settings.provider
  const label = providerLabels[provider]
  const last = titles.length - 1

  const next = async () => {
    setWarning('')
    // Talking points can't work without a saved provider key, so check before moving on.
    if (step === 2 && provider !== 'local' && !(await readKey(provider).catch(() => ''))) {
      return setWarning(`Paste your ${label} key and click Save key to continue.`)
    }
    if (step === last) update({ welcomed: true })
    else setStep(step + 1)
  }

  const pages = [
    <div className="stack-loose">
      <p>Cue listens to your interview and, when you're asked a question, puts short talking points from your own experience in front of you.</p>
      <Point icon="∿" title="Listens live">Hears the call, knows which voice is yours and which is the interviewer's.</Point>
      <Point icon="✦" title="Answers on its own">When the interviewer asks a question, talking points appear within seconds. No buttons.</Point>
      <Point icon="⚿" title="Stays on your Mac">Your résumé, keys, and saved interviews are stored only on this Mac. Text is sent only to the AI services you choose.</Point>
      <p className="muted">Setup takes about two minutes. You'll need an API key from an AI provider.</p>
    </div>,
    <div className="stack">
      <p className="muted">Cue grounds every answer in your real experience, so it never invents jobs or results. Import your résumé and describe the role you're interviewing for.</p>
      <ExperienceSettings settings={settings} update={update} importing={importing} setImporting={setImporting} />
    </div>,
    <div className="stack-loose">
      <p className="muted">Cue uses an AI provider to write your talking points. You need your own API key. It's separate from a ChatGPT, Claude, or Gemini chat subscription, and the provider bills you based on how much you use it.</p>
      <div className="callout stack">
        <span className="section-title">HOW TO GET A KEY</span>
        {keyUrls[provider] ? (
          <>
            <div>1. Choose your provider below.<br />2. Open {label}'s key page, sign in, and create a new key.<br />3. Paste it below, click <strong>Save key</strong>, then <strong>Test connection</strong>.</div>
            <Link href={keyUrls[provider]!}>Get a {label} key ↗</Link>
          </>
        ) : (
          <div>Start Ollama or LM Studio, load a model, and enter its model ID and server URL below. A key is only needed if your server requires one.</div>
        )}
      </div>
      <ProviderSettings settings={settings} update={update} />
    </div>,
    <div className="stack-loose">
      <p className="muted">Without TypeSafe, Cue spots most questions by how they're phrased and sends your whole résumé with each one. With a TypeSafe key, Cue also catches questions that don't sound like questions, and sends only the résumé lines that fit, which gives tighter answers.</p>
      <Link href="https://typesafe.ai">Get a TypeSafe key at typesafe.ai ↗</Link>
      <TypeSafeSettings />
    </div>,
    <div className="stack-loose">
      <Point icon="▭" title="On a video call">Choose <strong>Video call</strong>, then <strong>Start listening</strong>. Your mic is you; Mac audio is the interviewer. Wear headphones so your mic hears only you.</Point>
      <Point icon="⚇" title="In person">Choose <strong>In person</strong>. One microphone hears the room, so Cue treats everything it hears as the interviewer.</Point>
      <Point icon="✋" title="Permissions">
        {isMac
          ? 'The first time, macOS asks to allow Speech Recognition, Microphone, and Screen & System Audio Recording. Allow all three. Cue captures audio only.'
          : 'The first time, Windows may ask to allow microphone access. Cue captures audio only.'}
      </Point>
      <Point icon="❐" title="Keep answers in view">Press {keys.floating} for a floating window that stays on top of your call.</Point>
      <p className="muted">You can change any of this later in <strong>Context &amp; setup</strong>, or reopen this guide from <strong>Getting started</strong>.</p>
    </div>,
  ]

  return (
    <Modal width={620} height={680}>
      <div className="progress-steps">
        {titles.map((t, i) => <span key={t} className={i <= step ? 'on' : ''} />)}
      </div>
      <div className="eyebrow mint">STEP {step + 1} OF {titles.length}</div>
      <h2 className="title-lg">{titles[step]}</h2>
      <div className="scroll">{pages[step]}</div>
      {warning && <div className="warn">{warning}</div>}
      <div className="row">
        {step > 0 && <button disabled={importing} onClick={() => (setWarning(''), setStep(step - 1))}>Back</button>}
        <span className="grow" />
        {step === 2 && warning && <button onClick={() => (setWarning(''), setStep(3))}>Skip for now</button>}
        {step === 3 && <button onClick={() => setStep(4)}>Skip</button>}
        <button className="prominent" disabled={importing} onClick={next}>{step === last ? 'Start using Cue' : 'Continue'}</button>
      </div>
    </Modal>
  )
}
