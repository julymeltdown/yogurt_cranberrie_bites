import { createRoot } from 'react-dom/client'
import { MuralEditor } from '../../MuralEditor'
import { ack, fixture, snapshot } from '../fixture'
import '../../../../../shared/styles/base.css'

const state = new URLSearchParams(location.search).get('state') ?? 'loaded'
const f = fixture({ place: async command => state === 'sending' ? new Promise(() => {}) : state === 'unknown' ? { kind: 'unknown' }
  : state === 'pending' ? { kind: 'pending' } : state === 'locked' ? { kind: 'rejected', reason: 'MURAL_REGION_LOCKED' } : ack(command) }, () => 0)
if (state === 'empty') f.controller.acceptSnapshot(f.controller.scope, snapshot({ pixels: Array(16384).fill(0) }))
if (['frozen', 'paused'].includes(state)) f.controller.acceptSnapshot(f.controller.scope, snapshot({ state: state === 'frozen' ? 'FROZEN' : 'PAUSED' }))
if (state === 'cooldown') f.controller.acceptSnapshot(f.controller.scope, snapshot({ next_allowed_at: '2026-11-04T00:01:00Z' }))
if (state === 'read-only') f.controller.setAvailability(f.controller.scope, 'read-only')
if (state === 'error' || state === 'loading') { f.controller.acceptSnapshot(f.controller.scope, {}); if (state === 'loading') f.controller.setAvailability(f.controller.scope, 'loading') }
if (['preview', 'cooldown', 'unknown', 'pending', 'locked', 'sending'].includes(state)) { f.controller.chooseColor(2); f.controller.select(64, 64) }
if (['unknown', 'pending', 'locked'].includes(state)) await f.controller.confirm()
if (state === 'sending') void f.controller.confirm()
window.__WALL_VISUAL__ = { controller: f.controller, counts: () => ({ places: f.places.length, checks: f.checks.length }), refresh: () => f.controller.acceptSnapshot(f.controller.scope, snapshot()), dispose: f.dispose }
declare global { interface Window { __WALL_VISUAL__: { controller: typeof f.controller; counts(): { places: number; checks: number }; refresh(): unknown; dispose(): void } } }
createRoot(document.getElementById('root')!).render(<main className="wall-fixture"><p className="fixture-label">UIV05 · editor / {state} · deterministic mural fixture, no live collaboration.</p><MuralEditor controller={f.controller} onRefresh={() => { window.__WALL_VISUAL__.refresh() }} /><style>{`.wall-fixture{padding:12px;max-width:720px;margin:auto}.fixture-label{font:12px/1.4 system-ui;color:#dfd8c9}body{margin:0;background:#171616}`}</style></main>)
