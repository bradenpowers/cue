import { useState } from 'react'
import { isMac } from '../api'
import type { Settings } from '../types'
import ExperienceSettings from './ExperienceSettings'
import Modal from './Modal'
import ProviderSettings from './ProviderSettings'
import SpeechSettings from './SpeechSettings'
import TypeSafeSettings from './TypeSafeSettings'
import type { UpdateSettings } from './Workspace'

export default function Setup({ settings, update, download, onDone }: { settings: Settings; update: UpdateSettings; download: number | null; onDone: () => void }) {
  const [importing, setImporting] = useState(false)
  return (
    <Modal width={600} height={700}>
      <div className="scroll stack-loose">
        <div className="row">
          <h2 className="grow">Make Cue yours.</h2>
          <button onClick={onDone} disabled={importing}>Done</button>
        </div>
        <div className="muted">Give your assistant the context behind your answers.</div>
        <ExperienceSettings settings={settings} update={update} importing={importing} setImporting={setImporting} />
        <hr />
        <ProviderSettings settings={settings} update={update} />
        <hr />
        <TypeSafeSettings />
        {!isMac && (
          <>
            <hr />
            <SpeechSettings settings={settings} update={update} download={download} />
          </>
        )}
      </div>
    </Modal>
  )
}
