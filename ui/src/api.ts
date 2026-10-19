import { invoke } from '@tauri-apps/api/core'
import { useEffect } from 'react'
import type { ImportedResume, Interview, LLMProvider, Settings, Snapshot } from './types'

export const isMac = navigator.userAgent.includes('Mac')
export const keys = isMac ? { suggest: '⌘ ↵', floating: '⌘ ⇧ J' } : { suggest: 'Ctrl ↵', floating: 'Ctrl ⇧ J' }

export const snapshot = () => invoke<Snapshot>('snapshot')
export const startListening = (call: boolean) => invoke<void>('start_listening', { call })
export const stopListening = () => invoke<void>('stop_listening')
export const suggest = (question?: string) => invoke<void>('suggest', { question })
export const cancel = () => invoke<void>('cancel')
export const clear = () => invoke<void>('clear')
export const exportSession = (path: string) => invoke<void>('export', { path })
export const setNotes = (text: string) => invoke<void>('set_notes', { text })
export const getSettings = () => invoke<Settings>('get_settings')
export const saveSettings = (settings: Settings) => invoke<void>('save_settings', { settings })
export const readKey = (account: string) => invoke<string>('read_key', { account })
export const saveKey = (account: string, value: string) => invoke<void>('save_key', { account, value })
export const testProvider = (provider: LLMProvider, model: string, localBaseUrl: string, key: string) =>
  invoke<string>('test_provider', { provider, model, localBaseUrl, key })
export const testTypesafe = (key: string) => invoke<void>('test_typesafe', { key })
export const importResume = (path: string) => invoke<ImportedResume>('import_resume', { path })
export const listInterviews = () => invoke<Interview[]>('list_interviews')
export const deleteInterview = (id: string) => invoke<void>('delete_interview', { id })
export const downloadWhisperModel = (name: string) => invoke<void>('download_whisper_model', { name })
export const toggleFloating = () => invoke<void>('toggle_floating')
export const showMain = () => invoke<void>('show_main')

/** Ctrl/⌘+Enter suggests with `question`; Ctrl/⌘+Shift+J toggles the floating window. */
export function useShortcuts(question: string) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return
      if (e.key === 'Enter') {
        e.preventDefault()
        suggest(question.trim() || undefined)
      } else if (e.shiftKey && e.key.toLowerCase() === 'j') {
        e.preventDefault()
        toggleFloating()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [question])
}
