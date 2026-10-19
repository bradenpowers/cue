import { listen } from '@tauri-apps/api/event'
import { useEffect, useState } from 'react'
import { snapshot } from './api'
import type { Snapshot } from './types'

export function useCueState(): Snapshot | null {
  const [state, setState] = useState<Snapshot | null>(null)
  useEffect(() => {
    let live = true
    const unlisten = listen<Snapshot>('state', (e) => setState(e.payload))
    // An event that lands first is newer than this initial fetch.
    snapshot().then((s) => live && setState((cur) => cur ?? s))
    return () => {
      live = false
      unlisten.then((f) => f())
    }
  }, [])
  return state
}
