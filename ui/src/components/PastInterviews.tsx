import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { useEffect, useState } from 'react'
import { deleteInterview, listInterviews } from '../api'
import type { Interview } from '../types'
import Modal from './Modal'

// Same format as Interview::markdown in src-tauri/src/store.rs ("Oct 19, 2026 at 3:04 PM").
const formatDate = (iso: string) => {
  const d = new Date(iso)
  return `${d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })} at ${d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' })}`
}
const title = (i: Interview) => i.role.split(/\r?\n/)[0] || 'Interview'
const markdown = (i: Interview) =>
  `# Cue session\n\n${formatDate(i.date)}\n\n## Transcript\n\n${i.transcript}\n\n## Talking points\n\n${i.talkingPoints.join('\n\n')}\n\n## Notes\n\n${i.notes}`

export default function PastInterviews({ onDone }: { onDone: () => void }) {
  const [interviews, setInterviews] = useState<Interview[]>([])
  const [selection, setSelection] = useState<Interview | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    listInterviews().then(setInterviews)
  }, [])

  const remove = async (i: Interview) => {
    try {
      await deleteInterview(i.id)
      setInterviews((all) => all.filter((x) => x.id !== i.id))
      if (selection?.id === i.id) setSelection(null)
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <Modal width={820} height={600}>
      <div className="row">
        <h2 className="grow">Past interviews</h2>
        <button onClick={onDone}>Done</button>
      </div>
      {error && <div className="small warn">{error}</div>}
      {interviews.length === 0 ? (
        <div className="center muted">Interviews save automatically as you go. They'll appear here.</div>
      ) : (
        <div className="interviews">
          <ul className="interview-list">
            {interviews.map((i) => (
              <li key={i.id}>
                <button className={selection?.id === i.id ? 'selected' : ''} onClick={() => setSelection(i)}>
                  <div className="ellipsis">{title(i)}</div>
                  <div className="small muted">{formatDate(i.date)}</div>
                </button>
              </li>
            ))}
          </ul>
          <div className="scroll interview-detail">
            {selection ? (
              <>
                <div className="row end">
                  <button title="Copy interview" aria-label="Copy interview" onClick={() => writeText(markdown(selection))}>⧉</button>
                  <button className="destructive" title="Delete interview" aria-label="Delete interview" onClick={() => remove(selection)}>⌫</button>
                </div>
                <div className="selectable answer">{markdown(selection)}</div>
              </>
            ) : (
              <div className="muted center-top">Select an interview to review it.</div>
            )}
          </div>
        </div>
      )}
    </Modal>
  )
}
