import { useState } from 'react'
import type { ImportedResume } from '../types'
import Modal from './Modal'

type Props = { imported: ImportedResume; hasExperience: boolean; onCancel: () => void; onSave: (text: string, append: boolean) => void }

export default function ResumePreview({ imported, hasExperience, onCancel, onSave }: Props) {
  const [text, setText] = useState(imported.text)
  const cleaned = text.trim()
  return (
    <Modal width={600}>
      <div className="stack-loose">
        <h2>Review your résumé</h2>
        <div className="mint ellipsis"><span className="icon">▤</span>{imported.filename}</div>
        <div className="muted">Check the reading order and edit anything that needs fixing. This text becomes context for future talking points.</div>
        <textarea className="field" style={{ minHeight: 260 }} aria-label="Extracted résumé text" value={text} onChange={(e) => setText(e.target.value)} />
        <div className="small muted">Extracted locally · {[...text].length.toLocaleString()} characters · Original file unchanged</div>
        {hasExperience && <div className="small muted">Replace overwrites your saved experience. Add keeps your existing experience and appends this text.</div>}
        <div className="row">
          <button onClick={onCancel}>Cancel</button>
          <span className="grow" />
          {hasExperience && <button disabled={!cleaned} onClick={() => onSave(cleaned, true)}>Add to experience</button>}
          <button className="prominent" disabled={!cleaned} onClick={() => onSave(cleaned, false)}>
            {hasExperience ? 'Replace experience' : 'Use résumé'}
          </button>
        </div>
      </div>
    </Modal>
  )
}
