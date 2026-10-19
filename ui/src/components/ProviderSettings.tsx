import { useEffect, useRef, useState } from 'react'
import { readKey, saveKey, testProvider } from '../api'
import { defaultModels, modelFor, providerLabels, providers, type LLMProvider, type Settings } from '../types'
import type { UpdateSettings } from './Workspace'

export default function ProviderSettings({ settings, update }: { settings: Settings; update: UpdateSettings }) {
  const provider = settings.provider
  const label = providerLabels[provider]
  const [key, setKey] = useState('')
  const [message, setMessage] = useState('')
  const [isError, setIsError] = useState(false)
  const [testing, setTesting] = useState(false)
  const drafts = useRef<Partial<Record<LLMProvider, string>>>({})
  // Bumped to ignore a test result that lands after Cancel or after this view closes.
  const testRun = useRef(0)

  const show = (text: string, error: boolean) => {
    setMessage(text)
    setIsError(error)
  }

  useEffect(() => {
    const draft = drafts.current[provider]
    if (draft !== undefined) return setKey(draft)
    let live = true
    readKey(provider).then(
      (k) => live && setKey(k),
      (e) => live && (setKey(''), show(String(e), true)),
    )
    return () => {
      live = false
    }
  }, [provider])

  useEffect(() => () => void testRun.current++, [])

  const choose = (p: LLMProvider) => {
    drafts.current[provider] = key
    setMessage('')
    update({ provider: p })
  }

  const save = async () => {
    const value = key.trim()
    try {
      await saveKey(provider, value)
      setKey(value)
      drafts.current[provider] = value
      show(value ? `${label} key saved in Cue.` : `Key removed for ${label}.`, false)
    } catch (e) {
      show(String(e), true)
    }
  }

  const test = async () => {
    const run = ++testRun.current
    setTesting(true)
    setMessage('')
    try {
      const ok = await testProvider(provider, modelFor(settings, provider), settings.localBaseUrl, key)
      if (run === testRun.current) show(ok, false)
    } catch (e) {
      if (run === testRun.current) show(String(e), true)
    } finally {
      if (run === testRun.current) setTesting(false)
    }
  }

  const cancelTest = () => {
    testRun.current++
    setTesting(false)
    show('Connection test canceled.', false)
  }

  return (
    <section className="stack">
      <span className="section-title">AI PROVIDER</span>
      <select aria-label="Provider" value={provider} disabled={testing} onChange={(e) => choose(e.target.value as LLMProvider)}>
        {providers.map((p) => <option key={p} value={p}>{providerLabels[p]}</option>)}
      </select>
      <label className="row">
        Model
        <input className="field grow" disabled={testing} placeholder={provider === 'local' ? 'Model ID loaded in your local server' : defaultModels[provider]}
          value={modelFor(settings, provider)} onChange={(e) => update({ models: { ...settings.models, [provider]: e.target.value } })} />
      </label>
      {provider === 'local' && (
        <>
          <label className="row">
            Base URL
            <input className="field grow" disabled={testing} placeholder="http://localhost:11434/v1" value={settings.localBaseUrl} onChange={(e) => update({ localBaseUrl: e.target.value })} />
          </label>
          <div className="small muted pre">
            {'Ollama: http://localhost:11434/v1\nLM Studio: http://localhost:1234/v1\nStart the server and enter a model you have downloaded. Include /v1 in the URL.'}
          </div>
        </>
      )}
      <div className="row">
        <input className="field grow" type="password" disabled={testing} placeholder={provider === 'local' ? 'Server API key (optional)' : `${label} API key`}
          value={key} onChange={(e) => setKey(e.target.value)} />
        <button disabled={testing} onClick={save}>Save key</button>
      </div>
      <div className="row">
        <button disabled={testing} onClick={test}>{testing ? 'Testing…' : 'Test connection'}</button>
        {testing && <><span className="spinner" /><button onClick={cancelTest}>Cancel</button></>}
        <span className="small muted">Sends a short test prompt; no résumé or transcript.</span>
      </div>
      {message && <div className={`small selectable ${isError ? 'warn' : 'mint'}`}>{message}</div>}
      <div className="small muted">
        {provider === 'local'
          ? 'Talking points go to the local server you configure. Keys are optional unless your server requires one. Speech recognition may still use Apple’s service.'
          : `Talking points send your experience, role, question, and recent transcript to ${label}. Use a provider API key; keys are saved on this Mac in Cue’s settings. Audio is not sent to this provider.`}
      </div>
      <div className="small muted">Provider and model settings save automatically. Click Save key after entering a key; Test connection can use the unsaved key above.</div>
    </section>
  )
}
