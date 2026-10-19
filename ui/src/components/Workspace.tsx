import { save } from '@tauri-apps/plugin-dialog'
import { useEffect, useRef, useState } from 'react'
import * as api from '../api'
import type { Settings } from '../types'
import { useCueState } from '../useCueState'
import CaptureControls from './CaptureControls'
import Modal from './Modal'
import Notes from './Notes'
import PastInterviews from './PastInterviews'
import Setup from './Setup'
import Sidebar from './Sidebar'
import TalkingPoints from './TalkingPoints'
import Transcript from './Transcript'
import Welcome from './Welcome'

export type UpdateSettings = (patch: Partial<Settings>) => void

export default function Workspace() {
  const state = useCueState()
  const [question, setQuestion] = useState('')
  const [notes, setNotes] = useState('')
  const [settings, setSettings] = useState<Settings | null>(null)
  const [saveError, setSaveError] = useState('')
  const [setup, setSetup] = useState(false)
  const [reviewing, setReviewing] = useState(false)
  const [confirmClear, setConfirmClear] = useState(false)
  const sentNotes = useRef('')
  const settingsDirty = useRef(false)
  api.useShortcuts(question)

  useEffect(() => {
    api.getSettings().then(setSettings)
  }, [])

  useEffect(() => {
    if (notes === sentNotes.current) return
    const t = setTimeout(() => {
      sentNotes.current = notes
      api.setNotes(notes)
    }, 500)
    return () => clearTimeout(t)
  }, [notes])

  useEffect(() => {
    if (!settings || !settingsDirty.current) return
    const t = setTimeout(() => {
      api.saveSettings(settings).then(() => setSaveError(''), (e) => setSaveError(String(e)))
    }, 500)
    return () => clearTimeout(t)
  }, [settings])

  const update: UpdateSettings = (patch) => {
    settingsDirty.current = true
    setSettings((s) => s && { ...s, ...patch })
  }

  const exportSession = async () => {
    const path = await save({ defaultPath: 'Cue session.md' })
    if (path) await api.exportSession(path).catch((e) => setSaveError(String(e)))
  }

  const clear = async () => {
    setConfirmClear(false)
    await api.clear()
    setQuestion('')
    setNotes('')
    sentNotes.current = ''
  }

  if (!state || !settings) return null
  return (
    <div className="workspace">
      <Sidebar onSetup={() => setSetup(true)} onPast={() => setReviewing(true)} onWelcome={() => update({ welcomed: false })} />
      <main className="main">
        <header className="row">
          <div className="grow stack-tight">
            <div className="eyebrow mint">LIVE WORKSPACE</div>
            <h1>Stay in the conversation.</h1>
            <div className="muted">Your experience. A clearer way to share it.</div>
          </div>
          <button title="Export session" aria-label="Export session" onClick={exportSession}>⇪</button>
          <button title="New session" aria-label="New session" onClick={() => setConfirmClear(true)}>+</button>
        </header>
        <CaptureControls state={state} />
        {[state.error, saveError].filter(Boolean).map((e) => (
          <div key={e} className="warn selectable">{e}</div>
        ))}
        <div className="columns">
          <Transcript state={state} />
          <TalkingPoints state={state} question={question} setQuestion={setQuestion} />
        </div>
        <Notes notes={notes} setNotes={setNotes} />
        <footer className="row tiny muted">
          <span className="dot" />
          <span className="grow">Interviews save automatically on this {api.isMac ? 'Mac' : 'PC'}.</span>
          <span>{api.isMac ? 'macOS' : 'Windows'} · Cue</span>
        </footer>
      </main>
      {setup && <Setup settings={settings} update={update} download={state.download} onDone={() => setSetup(false)} />}
      {reviewing && <PastInterviews onDone={() => setReviewing(false)} />}
      {!settings.welcomed && <Welcome settings={settings} update={update} />}
      {confirmClear && (
        <Modal width={420}>
          <p>Start a new session? The current interview is saved to Past interviews.</p>
          <div className="row end">
            <button onClick={() => setConfirmClear(false)}>Cancel</button>
            <button className="destructive" onClick={clear}>Clear and start new</button>
          </div>
        </Modal>
      )}
    </div>
  )
}
