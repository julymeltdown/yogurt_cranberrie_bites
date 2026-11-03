import { createRoot } from 'react-dom/client'
import { AvatarEditor } from '../../AvatarEditor'
import { createAvatarEditorController } from '../../controller'
import { createReviewedAvatarPreview } from '../../preview'
import { harness, original } from '../fixtures/harness'
import { createHttpClient } from '../../../../shared/api/http'
import manifest from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import '../../../../shared/styles/base.css'

if (!import.meta.env.DEV) throw new Error('Synthetic avatar fixture is development-only')
const state = new URLSearchParams(location.search).get('state') ?? 'loaded', f = harness()
let assetReads = 0
const unavailable = state === 'unavailable' || state === 'loading'
const http = createHttpClient({ session: f.session, fetch: async () => { throw new Error('Unavailable preview must not start participant requests') } })
const controller = unavailable ? createAvatarEditorController({ auth: { privacy: f.privacy, request: http.request }, createPreview: async (_scope, signal) => {
  if (state === 'loading') return new Promise(() => {})
  return createReviewedAvatarPreview({ document: manifest, pin: manifest, resources: { async read() { assetReads++; throw new Error('Unapproved asset fetch') } }, signal })
} }) : f.controller
if (state === 'load-error') f.setMode('read-failed')
controller.start()
const settle = () => new Promise<void>(resolve => {
  const ready = () => { const s = controller.read(); return s.phase === 'unavailable' || s.phase === 'ready' && (s.preview.phase === 'ready' || s.notice === 'load-failed') }
  if (ready()) { resolve(); return }; const stop = controller.subscribe(() => { if (ready()) { stop(); resolve() } })
})
if (state !== 'loading') await settle()
if (['draft', 'denied', 'unknown', 'conflict', 'candidate', 'saving', 'saved', 'preview-loading'].includes(state)) controller.selectPreset('fixture_alternate')
if (state === 'denied') { f.setMode('denied'); await controller.save() }
if (state === 'unknown') { f.setMode('uncertain'); await controller.save() }
if (state === 'conflict' || state === 'candidate') { f.setMode('conflict'); await controller.save() }
if (state === 'candidate') { f.setMode('ok'); f.setServer({ revision: 4, config: original }); await controller.reload() }
if (state === 'saving') { f.setMode('slow'); void controller.save() }
if (state === 'saved') await controller.save()
if (state === 'preview-loading') { f.previews[0]!.slow = true; controller.selectPreset('fixture_original') }
if (state === 'expired') f.privacy.changePolicy(f.privacy.capture(), 'consent-revoked')
window.__AVATAR_VISUAL__ = { controller, counts: () => ({ writes: f.requests.filter(r => r.method === 'PUT').length, reads: f.requests.filter(r => r.method === 'GET').length, assetReads }),
  mode: f.setMode, server: f.setServer, dispose() { controller.dispose(); f.dispose() } }
declare global { interface Window { __AVATAR_VISUAL__: { controller: typeof controller; counts(): { writes: number; reads: number; assetReads: number }; mode: typeof f.setMode; server: typeof f.setServer; dispose(): void } } }
createRoot(document.getElementById('root')!).render(<main className="avatar-fixture"><p className="fixture-label">UIV04 · {state} · test-only metadata/blank canvas. No character art or live save proof.</p><AvatarEditor controller={controller} /><style>{`body{background:#171616}.avatar-fixture{padding:12px;max-width:720px;margin:auto}.fixture-label{font:12px/1.4 system-ui;color:#dfd8c9;margin-bottom:8px}`}</style></main>)
