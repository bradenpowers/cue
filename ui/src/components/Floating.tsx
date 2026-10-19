import { useState } from 'react'
import { suggest, useShortcuts } from '../api'
import { useCueState } from '../useCueState'

export default function Floating() {
  const state = useCueState()
  const [question, setQuestion] = useState('')
  useShortcuts(question)
  if (!state) return null
  return (
    <div className="floating">
      <div className="mint"><span className="icon">✦</span>A little help, right here.</div>
      <div className="scroll selectable answer">{state.answer || 'Talking points from your live workspace will appear here.'}</div>
      {state.error && <div className="small warn">{state.error}</div>}
      <input className="field" placeholder="Question or follow-up…" value={question} onChange={(e) => setQuestion(e.target.value)} />
      <button className="prominent" disabled={state.busy} onClick={() => suggest(question.trim() || undefined)}>
        {state.busy ? 'Thinking…' : 'Suggest talking points'}
      </button>
    </div>
  )
}
