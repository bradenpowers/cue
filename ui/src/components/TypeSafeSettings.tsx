import { useEffect, useRef, useState } from 'react'
import { readKey, saveKey, testTypesafe } from '../api'

const account = 'typesafe'

export default function TypeSafeSettings() {
  const [key, setKey] = useState('')
  const [message, setMessage] = useState('')
  const [isError, setIsError] = useState(false)
  const [testing, setTesting] = useState(false)
  // Bumped to ignore a test result that lands after Cancel or after this view closes.
  const testRun = useRef(0)

  const show = (text: string, error: boolean) => {
    setMessage(text)
    setIsError(error)
  }

  useEffect(() => {
    readKey(account).then(setKey, (e) => show(String(e), true))
  }, [])
  useEffect(() => () => void testRun.current++, [])

  const save = async () => {
    const value = key.trim()
    try {
      await saveKey(account, value)
      setKey(value)
      show(value ? 'TypeSafe key saved in Cue.' : 'TypeSafe key removed.', false)
    } catch (e) {
      show(String(e), true)
    }
  }

  const test = async () => {
    const run = ++testRun.current
    setTesting(true)
    setMessage('')
    try {
      await testTypesafe(key)
      if (run === testRun.current) show('TypeSafe key works.', false)
    } catch (e) {
      if (run === testRun.current) show(String(e), true)
    } finally {
      if (run === testRun.current) setTesting(false)
    }
  }

  const cancelTest = () => {
    testRun.current++
    setTesting(false)
    show('Key test canceled.', false)
  }

  return (
    <section className="stack">
      <span className="section-title">TYPESAFE (OPTIONAL)</span>
      <div className="row">
        <input className="field grow" type="password" disabled={testing} placeholder="TypeSafe API key" value={key} onChange={(e) => setKey(e.target.value)} />
        <button disabled={testing} onClick={save}>Save key</button>
      </div>
      <div className="row">
        <button disabled={testing} onClick={test}>{testing ? 'Testing…' : 'Test key'}</button>
        {testing && <><span className="spinner" /><button onClick={cancelTest}>Cancel</button></>}
        <span className="small muted">Sends a short sample question; no résumé or transcript.</span>
      </div>
      {message && <div className={`small ${isError ? 'warn' : 'mint'}`}>{message}</div>}
      <div className="small muted">
        With a key, Cue asks TypeSafe what kind of question was asked and which résumé lines fit it, so your AI provider receives only the relevant experience. This sends your role, résumé lines, and recent transcript to TypeSafe. It also catches interviewer questions that don’t sound like questions, such as “I’d love to hear about your last project.”
      </div>
    </section>
  )
}
