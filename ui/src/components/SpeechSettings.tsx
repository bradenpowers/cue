import { useState } from 'react'
import { downloadWhisperModel } from '../api'
import type { Settings } from '../types'
import type { UpdateSettings } from './Workspace'

export default function SpeechSettings({ settings, update, download }: { settings: Settings; update: UpdateSettings; download: number | null }) {
  const [error, setError] = useState('')
  const fetchModel = () => {
    setError('')
    downloadWhisperModel(settings.whisperModel).catch((e) => setError(String(e)))
  }
  return (
    <section className="stack">
      <span className="section-title">SPEECH RECOGNITION</span>
      <div className="row">
        <select aria-label="Speech model" value={settings.whisperModel} disabled={download != null} onChange={(e) => update({ whisperModel: e.target.value })}>
          {['tiny.en', 'base.en', 'small.en'].map((m) => <option key={m}>{m}</option>)}
        </select>
        <button disabled={download != null} onClick={fetchModel}>Download model</button>
        {download != null && <progress className="grow" value={download} max={1} />}
      </div>
      {error && <div className="small warn">{error}</div>}
      <label className="row">
        Re-decode interval (seconds)
        <input className="field" type="number" min={0.5} max={3} step={0.5} value={settings.redecodeInterval}
          onChange={(e) => {
            const v = e.target.valueAsNumber
            if (v >= 0.5 && v <= 3) update({ redecodeInterval: v })
          }} />
      </label>
      <div className="small muted">Smaller models and longer intervals use less CPU.</div>
    </section>
  )
}
