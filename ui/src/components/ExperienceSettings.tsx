import { open } from '@tauri-apps/plugin-dialog'
import { useState } from 'react'
import { importResume } from '../api'
import type { ImportedResume, Settings } from '../types'
import ResumePreview from './ResumePreview'
import type { UpdateSettings } from './Workspace'

type Props = { settings: Settings; update: UpdateSettings; importing: boolean; setImporting: (b: boolean) => void }

export default function ExperienceSettings({ settings, update, importing, setImporting }: Props) {
  const [imported, setImported] = useState<ImportedResume | null>(null)
  const [importError, setImportError] = useState('')

  const choose = async () => {
    const path = await open({ filters: [{ name: 'Résumé', extensions: ['pdf', 'docx', 'rtf', 'txt', 'text', 'md'] }] })
    if (!path) return
    setImporting(true)
    setImportError('')
    try {
      setImported(await importResume(path))
    } catch (e) {
      setImportError(String(e))
    } finally {
      setImporting(false)
    }
  }

  const resume = settings.resume.trim()
  return (
    <section className="stack">
      <div className="row">
        <span className="section-title grow">YOUR EXPERIENCE</span>
        {importing && <><span className="spinner" /><span className="small">Reading résumé…</span></>}
        <button onClick={choose} disabled={importing}>⇩ Import résumé</button>
      </div>
      {importError && <div className="small warn">{importError}</div>}
      <textarea className="field" style={{ height: 130 }} aria-label="Your experience" value={settings.resume} onChange={(e) => update({ resume: e.target.value })} />
      <div className="small muted">Import PDF, DOCX, RTF, or text, or paste your experience here. Saved automatically and used for talking points.</div>
      <span className="section-title">ROLE &amp; COMPANY</span>
      <textarea className="field" style={{ height: 90 }} aria-label="Role and company" value={settings.role} onChange={(e) => update({ role: e.target.value })} />
      {imported && (
        <ResumePreview
          imported={imported}
          hasExperience={resume !== ''}
          onCancel={() => setImported(null)}
          onSave={(text, append) => {
            update({ resume: append ? `${resume}\n\n${text}` : text })
            setImported(null)
          }}
        />
      )}
    </section>
  )
}
