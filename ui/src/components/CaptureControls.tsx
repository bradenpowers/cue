import { useState } from 'react'
import { isMac, startListening, stopListening } from '../api'
import type { Snapshot } from '../types'

export default function CaptureControls({ state }: { state: Snapshot }) {
  const [call, setCall] = useState(true)
  const running = state.them.running || state.me.running
  const starting = state.them.starting || state.me.starting
  return (
    <section className="card capture">
      <div className="row">
        <span className="icon mint">{running ? '∿' : '●'}</span>
        <div className="grow">
          <div className="strong">{running ? 'Listening live' : 'Ready when you are'}</div>
          <div className="small muted">
            {call ? `Video call · your mic is you, ${isMac ? 'Mac' : 'computer'} audio is the interviewer` : 'In person · the microphone hears everyone as the interviewer'}
          </div>
        </div>
        <select aria-label="Source" value={call ? 'call' : 'person'} onChange={(e) => setCall(e.target.value === 'call')} disabled={running || starting}>
          <option value="call">Video call</option>
          <option value="person">In person</option>
        </select>
        <button className="bordered" disabled={starting} onClick={() => (running ? stopListening() : startListening(call))}>
          {starting ? 'Connecting…' : running ? 'Stop listening' : 'Start listening'}
        </button>
      </div>
      {[state.them.error, state.me.error].filter(Boolean).map((e) => (
        <div key={e} className="small warn selectable">{e}</div>
      ))}
    </section>
  )
}
