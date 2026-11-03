import { StrictMode, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { EventOperations } from '../../index'
import { PlacesOperations } from '../../../places'
import { AuditOperations } from '../../../audit'
import { fixture, replaceIdentity, type Mode } from './data'

const role = new URLSearchParams(location.search).get('role')
const f = fixture({ ...(role === 'member' ? { role: 'MEMBER' } : {}), ...(role === 'absent' ? { absent: true } : {}) })
const root = createRoot(document.getElementById('root')!)
function Fixture() {
  const [surface, setSurface] = useState(new URLSearchParams(location.search).get('surface') ?? 'event')
  return <><header style={{ background: '#eee5d3', padding: 8, font: '14px Arial' }}><strong>SYNTHETIC FE38 FIXTURE ONLY</strong>
    <p>Fictional event, operator, clock and venue. No real publication.</p>
    <nav aria-label="Fixture surfaces" style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
      {['summary', 'event', 'places', 'audit'].map(value => <button key={value} type="button" style={{ minHeight: 44, minWidth: 44 }} onClick={() => setSurface(value)}>{value}</button>)}
    </nav></header>
    {surface === 'places' ? <PlacesOperations controller={f.controller} /> : surface === 'audit' ? <AuditOperations controller={f.controller} />
      : <EventOperations controller={f.controller} summaryOnly={surface === 'summary'} />}</>
}
root.render(<StrictMode><Fixture /></StrictMode>)
declare global { interface Window { __OPERATIONS__: {
  state(): { view: ReturnType<typeof f.controller.read>; writes: number; reads: number; subscriptions: ReturnType<typeof f.subscriptions> }
  mode(mode: Mode): void
  revoke(): void
  identity(): void
  expire(): void
  settle(): void
  dispose(): void
} } }
window.__OPERATIONS__ = {
  state: () => ({ view: f.controller.read(), writes: f.writes().length, reads: f.requests.filter(r => r.method === 'GET').length, subscriptions: f.subscriptions() }),
  mode: mode => f.setMode(mode), revoke: () => { f.setLease(null) }, identity: () => { replaceIdentity(f) }, expire: () => { f.advance(60000) }, settle: f.settle,
  dispose() { root.unmount(); f.dispose() },
}
