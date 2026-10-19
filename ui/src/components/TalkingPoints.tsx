import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { cancel, keys, suggest } from '../api'
import type { Snapshot } from '../types'

export default function TalkingPoints({ state, question, setQuestion }: { state: Snapshot; question: string; setQuestion: (q: string) => void }) {
  return (
    <section className="panel">
      <div className="row">
        <span className="panel-title grow"><span className="icon">✦</span>Talking points</span>
        {state.answer && (
          <button className="plain" title="Copy talking points" aria-label="Copy talking points" onClick={() => writeText(state.answer)}>⧉</button>
        )}
      </div>
      <div className="scroll">
        {state.answer ? (
          <div className="answer selectable">{state.answer}</div>
        ) : (
          <div className="empty">
            <div className="sparkle mint">✦</div>
            <div className="headline">Find your next thought.</div>
            <p className="muted">When the interviewer asks a question, Cue writes a concise starting point grounded in your experience.</p>
            <p className="small muted steps">
              01   Add your experience in Setup{'\n\n'}02   Start listening{'\n\n'}03   Talking points appear when you’re asked a question
            </p>
          </div>
        )}
      </div>
      <hr />
      <textarea className="bare" rows={2} placeholder="Ask a question, or use the transcript…" value={question} onChange={(e) => setQuestion(e.target.value)} />
      <div className="row">
        <button className="prominent grow suggest" disabled={state.busy} onClick={() => suggest(question.trim() || undefined)}>
          {state.busy && <span className="spinner" />}
          <span className="grow left">{state.busy ? 'Thinking…' : 'Suggest talking points'}</span>
          <span className="dim">{keys.suggest}</span>
        </button>
        {state.busy && <button onClick={() => cancel()}>Cancel</button>}
      </div>
    </section>
  )
}
