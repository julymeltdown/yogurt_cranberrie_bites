import { createRoot } from 'react-dom/client'
import { MuralEditor } from '../MuralEditor'
import { ack, fixture, snapshot } from './fixture'
import '../../../../shared/styles/base.css'

type Mode = 'ack' | 'lost' | 'pending' | 'rejected' | 'cooldown' | 'bad-ack'
let mode: Mode = 'ack'
const f = fixture({ place: async command => {
  if (mode === 'lost') throw new Error('Fixture connection lost')
  if (mode === 'pending') return { kind: 'pending' }
  if (mode === 'rejected') return { kind: 'rejected', reason: 'INVALID_INPUT' }
  if (mode === 'cooldown') return { kind: 'rejected', reason: 'MURAL_COOLDOWN' }
  return ack(command, mode === 'bad-ack' ? { event_id: '00000000-0000-4000-8000-000000000099' } : {})
} })
const controls = {
  mode(value: Mode) { mode = value },
  read: f.controller.read,
  counts: () => ({ places: f.places.length, checks: f.checks.length }),
  refresh(state: 'EDITABLE' | 'PAUSED' | 'FROZEN' = 'EDITABLE') {
    f.controller.acceptSnapshot(f.controller.scope, snapshot({ state, revision: f.controller.read().snapshot?.revision ?? 10 }))
  },
  readOnly() { f.controller.setAvailability(f.controller.scope, 'read-only') },
  logout() { f.session.replace(f.session.read().generation, null) },
  dispose: f.dispose,
}
declare global { interface Window { __MURAL__: typeof controls } }
window.__MURAL__ = controls
createRoot(document.getElementById('root')!).render(<main><p style={{ maxWidth: '40rem', margin: '0 auto 1rem' }}>FE32 deterministic fixture · MOCK_READY only · no live collaboration</p><MuralEditor controller={f.controller} onRefresh={() => controls.refresh()} /></main>)
