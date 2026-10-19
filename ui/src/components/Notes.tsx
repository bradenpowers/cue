export default function Notes({ notes, setNotes }: { notes: string; setNotes: (n: string) => void }) {
  return (
    <section className="card">
      <label className="small strong muted" htmlFor="notes"><span className="icon">✎</span>Your notes</label>
      <textarea id="notes" className="bare notes" placeholder="Names, questions to ask, things to come back to…" value={notes} onChange={(e) => setNotes(e.target.value)} />
    </section>
  )
}
