// TEST ONLY: declared synthetic HTTP and event-role inputs; no production authority.
import { createRoot } from 'react-dom/client'
import { createModeration } from '../../model/controller'
import { ModerationReview } from '../../ui/ModerationReview'
import { harness, id } from '../fixtures/harness'
import type { Lane } from '../../model/contract'

const query = new URLSearchParams(location.search), root = createRoot(document.getElementById('root')!)
const lane: Lane = query.get('lane') === 'members' ? 'members' : query.get('lane') === 'content' ? 'content' : 'reports'
const fault = query.get('fault')
let pending = true
const fixture = harness(lane, { open: async () => ({ mediaId: id(500), blob: new Blob([Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aWQAAAABJRU5ErkJggg=='), c => c.charCodeAt(0))], { type: 'image/png' }) }) })
fixture.override(request => {
  if (new URL(request.url).pathname.endsWith('/commands')) return Response.json(fixture.receipt('FAILED'))
  if (request.method === 'POST' && pending) return Response.json(fixture.receipt(), { status: 202 })
  return null
})
if (fault === 'disposed') fixture.auth.privacy.dispose()
const controller = fault ? createModeration({ auth: fixture.auth, lane, access: fault === 'disposed' ? fixture.access : {
  read: fixture.grant, subscribe() { throw new Error('PRIVATE producer diagnostic') } } }) : fixture.controller
window.__OPS_CURRENT__ = { snapshot: controller.read, requests: () => fixture.requests.length,
  writes: () => fixture.requests.filter(request => request.method === 'POST').length,
  fail: () => { fixture.setGrant(null) }, success: () => { pending = false },
  dispose() { root.unmount(); controller.dispose(); fixture.dispose() } }
root.render(<><p>SYNTHETIC FE37 FIXTURE ONLY. No real authority or moderation.</p><ModerationReview controller={controller} /></>)
declare global { interface Window { __OPS_CURRENT__: {
  snapshot(): ReturnType<typeof controller.read>; requests(): number; writes(): number; fail(): void; success(): void; dispose(): void
} } }
