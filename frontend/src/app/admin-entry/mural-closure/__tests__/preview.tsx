import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { fixture, ids, partialJob } from './fixture'
import { MuralClosurePanel } from '../MuralClosurePanel'
import type { MuralClosureView } from '../index'
import '../../../../features/admin/mural/__tests__/preview.css'
if (!import.meta.env.DEV) throw new Error('Test fixture is development-only')
const f = fixture(), entry = await f.open()
f.server.wall = { ...f.server.wall, state: 'PAUSED' }
function App() {
  const [view, setView] = useState<MuralClosureView>('mural')
  return <main><p className="fixture-caption">TEST_ONLY · canonical Auth and FE39 adapter · synthetic operator and server · no live authority</p>
    <nav aria-label="Fixture panel navigation" className="mcr-ops__actions"><button onClick={() => setView('mural')}>Open wall panel</button><button onClick={() => setView('closure')}>Open closure panel</button></nav>
    <MuralClosurePanel entry={entry} view={view} />
  </main>
}
const testOnly = {
  counts: () => ({ posts: f.posts().length, admissions: f.server.admissions, requests: f.server.requests.length, authorityListeners: f.listeners() }),
  mode(value: string) { f.server.mode = value },
  readOnly() { f.update({ ...f.current(), phase: 'READ_ONLY', eventRevision: 9 }); f.server.job = partialJob() },
  revoke() { f.update(null) },
  dispose() { entry.dispose(); f.dispose() },
  ids,
}
declare global { interface Window { __FE39_CURRENT__: typeof testOnly } }
window.__FE39_CURRENT__ = testOnly
createRoot(document.getElementById('root')!).render(<App />)
