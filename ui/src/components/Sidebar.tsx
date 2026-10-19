import { keys, toggleFloating } from '../api'

export default function Sidebar({ onSetup, onPast, onWelcome }: { onSetup: () => void; onPast: () => void; onWelcome: () => void }) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">✱</span>
        <span>Cue</span>
      </div>
      <nav className="stack">
        <div className="eyebrow muted">YOUR WORKSPACE</div>
        <div className="nav-item active"><span className="icon">∿</span>Live interview</div>
        <button className="nav-item" onClick={onSetup}><span className="icon">☰</span>Context &amp; setup</button>
        <button className="nav-item" onClick={onPast}><span className="icon">↺</span>Past interviews</button>
        <button className="nav-item" onClick={onWelcome}><span className="icon">?</span>Getting started</button>
      </nav>
      <div className="spacer" />
      <div className="promo stack">
        <span className="mint">❐</span>
        <div className="promo-title">A little room to think.</div>
        <div className="small muted">Keep your talking points nearby with the floating assistant.</div>
        <button className="link" onClick={() => toggleFloating()}>Open assistant ↗</button>
        <div className="tiny faint">{keys.floating}  while Cue is active</div>
      </div>
      <div className="tagline faint">BUILT FOR YOUR NEXT CHAPTER</div>
    </aside>
  )
}
