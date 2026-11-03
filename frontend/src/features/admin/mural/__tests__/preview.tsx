import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { fixture, job } from './fixture'
import { WallControl } from '../WallControl'
import { EventClosure } from '../../closure/EventClosure'
import './preview.css'
const f = fixture()
await f.mural.start(); f.closure.start()
function App() {
  const [tab, setTab] = useState('wall')
  return <main><p className="fixture-caption">FE39 deterministic C04 fixture · MOCK_READY · no live moderation</p>
    <nav aria-label="Fixture operations"><button onClick={() => setTab('wall')}>Wall control</button><button onClick={() => setTab('closure')}>Event closure</button></nav>
    {tab === 'wall' ? <WallControl controller={f.mural} /> : <EventClosure controller={f.closure} />}
  </main>
}
const testOnly = {
  counts: () => ({ posts: f.posts().length, requests: f.server.requests.length }),
  mode(value: string) { f.server.mode = value },
  progress(state: 'RUNNING' | 'PARTIAL_FAILURE' | 'COMPLETED') { f.server.job = job(state, f.server.job.revision + 1) },
  revoke() { f.update(null) },
  logout() { f.session.replace(f.session.read().generation, null) },
  dispose: f.cleanup,
}
declare global { interface Window { __FE39__: typeof testOnly } }
window.__FE39__ = testOnly
createRoot(document.getElementById('root')!).render(<App />)
