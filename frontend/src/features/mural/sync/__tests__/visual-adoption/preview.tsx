import { createRoot } from 'react-dom/client'
import { MuralSyncPanel } from '../../MuralSyncPanel'
import { delta, fixture, pixel, snapshot } from '../fixture'
import '../../../../../shared/styles/base.css'

const state = new URLSearchParams(location.search).get('state') ?? 'loaded'
const f = fixture({ ...(state === 'error' || state === 'loading' ? { reads: { snapshot: async () => { if (state === 'loading') return new Promise(() => {}); throw new Error('Fixture failed read') }, delta: async () => delta(10, []) } } : {}),
  commands: { place: async () => state === 'sending' ? new Promise(() => {}) : state === 'locked' ? { kind: 'rejected', reason: 'MURAL_REGION_LOCKED' } : { kind: state === 'pending' ? 'pending' : 'unknown' }, check: async () => ({ kind: 'unknown' }) } })
if (state === 'empty') f.server(snapshot({ pixels: Array(16384).fill(0) }))
if (state === 'frozen' || state === 'paused') f.server(snapshot({ state: state === 'frozen' ? 'FROZEN' : 'PAUSED' }))
if (state === 'cooldown') f.server(snapshot({ next_allowed_at: '2026-11-04T00:01:00Z' }))
if (state === 'unavailable') f.authority.set(null)
await f.start()
if (state === 'read-only') f.authority.set({ ...f.authority.read()!, writable: false })
if (state === 'gap') f.sync.receiveDelta(f.sync.lease()!, delta(11, [pixel(12)]))
if (['preview', 'cooldown', 'unknown', 'pending', 'locked', 'sending'].includes(state)) { f.sync.editor.chooseColor(2); f.sync.editor.select(64, 64) }
if (['unknown', 'pending', 'locked'].includes(state)) await f.sync.editor.confirm()
if (state === 'sending') void f.sync.editor.confirm()
window.__WALL_SYNC_VISUAL__ = { sync: f.sync, counts: () => ({ places: f.requests.filter(r => r.kind === 'place').length, checks: f.requests.filter(r => r.kind === 'check').length }), dispose: f.dispose }
declare global { interface Window { __WALL_SYNC_VISUAL__: { sync: typeof f.sync; counts(): { places: number; checks: number }; dispose(): void } } }
createRoot(document.getElementById('root')!).render(<main className="wall-fixture"><p className="fixture-label">UIV05 · sync / {state} · deterministic mural fixture, no live collaboration.</p><MuralSyncPanel sync={f.sync} /><style>{`.wall-fixture{padding:12px;max-width:720px;margin:auto}.fixture-label{font:12px/1.4 system-ui;color:#dfd8c9}body{margin:0;background:#171616}`}</style></main>)
