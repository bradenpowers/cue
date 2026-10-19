import type { ReactNode } from 'react'

export default function Modal({ width, height, children }: { width: number; height?: number; children: ReactNode }) {
  return (
    <div className="overlay">
      <div className="modal" role="dialog" aria-modal="true" style={{ width, height }}>
        {children}
      </div>
    </div>
  )
}
