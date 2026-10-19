import { useEffect, useRef } from 'react'
import { isMac } from '../api'
import type { Snapshot } from '../types'

const hint = isMac
  ? 'Mac audio requires Screen & System Audio Recording permission. Use headphones on calls so your mic hears only you. Only audio is processed. English transcription may use Apple’s service when on-device recognition is unavailable.'
  : "Video call captures your computer's audio. Use headphones on calls so your mic hears only you. Speech is transcribed locally with Whisper."

export default function Transcript({ state }: { state: Snapshot }) {
  const bottom = useRef<HTMLDivElement>(null)
  const running = state.them.running || state.me.running
  useEffect(() => {
    bottom.current?.scrollIntoView({ block: 'end' })
  }, [state.lines])
  return (
    <section className="panel">
      <div className="row">
        <span className="panel-title grow"><span className="icon">≡</span>Live transcript</span>
        <span className={`badge ${running ? 'mint' : 'muted'}`}>{running ? 'LIVE' : 'IDLE'}</span>
      </div>
      <div className="scroll">
        {state.lines.length === 0 ? (
          <div className="empty">
            <div className="headline">Every question,<br />a little more space.</div>
            <p className="muted">Your conversation will appear here when you start listening.</p>
            <p className="small faint">{hint}</p>
          </div>
        ) : (
          state.lines.map((l) => (
            <div key={l.id} className="line">
              <div className={`badge ${l.speaker === 'them' ? 'mint' : 'muted'}`}>{l.speaker === 'them' ? 'INTERVIEWER' : 'ME'}</div>
              <div className="selectable">{l.text}</div>
            </div>
          ))
        )}
        <div ref={bottom} />
      </div>
    </section>
  )
}
