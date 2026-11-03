import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider, useParams } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import type { ActiveScope, ShellPorts } from '../../../../app/ports'
import { GiftDetail, GiftDiscovery } from '../../ui/Gifts'
import { MyGifts } from '../../../my-gifts/MyGifts'
import { allCapabilities, fixture, giveaway, now, otherActor, reservation, uid } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

let mode = 'ok', disposed = false
const pending: (() => void)[] = []
const places = [{ id: giveaway().public_place_id, label: 'Fixture exchange reference' }, { id: uid(31), label: 'Fixture second reference' }]
const rows = [giveaway({ title: 'Handmade fan badges' }), giveaway({ giveaway_id: uid(40), title: 'Ribbon keepsakes', state: 'PAUSED', created_at: '2026-11-03T00:50:00Z' }),
  giveaway({ giveaway_id: uid(39), title: 'Pocket lyric zines', available: 0, created_at: '2026-11-03T00:40:00Z' }),
  giveaway({ giveaway_id: uid(38), title: 'Earlier sticker exchange', pickup_starts_at: '2026-11-04T08:00:00Z', pickup_ends_at: '2026-11-04T09:00:00Z', created_at: '2026-11-03T00:30:00Z' }),
  giveaway({ giveaway_id: uid(37), title: 'Cancelled patch swap', state: 'CANCELLED', created_at: '2026-11-03T00:20:00Z' })]
const clockStart = Date.now()
const f = fixture({ clock: { now: () => now + Date.now() - clockStart,
  setTimeout: (callback, delay) => setTimeout(callback, delay), clearTimeout: handle => clearTimeout(handle as ReturnType<typeof setTimeout>) }, handler: async request => {
  const url = new URL(request.url), path = url.pathname, actor = f.auth.session.read().userId!, event = f.auth.session.read().eventId!
  if (mode === 'pending') await new Promise<void>(resolve => pending.push(resolve))
  const failure = (status: number, reason: string) => Response.json({ code: 'FIXTURE', message: 'PRIVATE SERVER TEXT MUST NOT RENDER', details: { reason, ...(status === 429 ? { retry_after_ms: 1000 } : {}) } }, { status })
  if (mode === 'error') return failure(503, 'DEPENDENCY_UNAVAILABLE')
  if (mode === 'denied') return failure(403, 'PERMISSION_DENIED')
  if (mode === 'missing') return failure(404, 'RESOURCE_UNAVAILABLE')
  if (mode === 'rate') return failure(429, 'RATE_LIMITED')
  if (path.endsWith('/me/giveaways')) {
    const givingCursor = url.searchParams.get('giving_cursor'), receivingCursor = url.searchParams.get('receiving_cursor')
    return Response.json({ giving: { items: givingCursor === 'giving_2' ? [] : [giveaway({ event_id: event, giveaway_id: givingCursor ? uid(50) : uid(51), title: givingCursor ? 'Completed prints' : 'Your private draft prints', state: givingCursor ? 'CLOSED' : 'DRAFT', owner: { ...giveaway().owner, user_id: actor } })],
      next_cursor: givingCursor === 'giving_2' ? 'giving_3' : givingCursor ? null : 'giving_2' },
    receiving: { items: [reservation({ event_id: event, recipient_id: actor, reservation_id: receivingCursor ? uid(61) : reservation().reservation_id,
      created_at: receivingCursor ? '2026-11-03T00:30:00Z' : reservation().created_at,
      state: receivingCursor ? 'CLAIMED' : 'HELD', claim_receipt_id: receivingCursor ? uid(62) : null })], next_cursor: receivingCursor ? null : 'receiving_2' } })
  }
  const all = rows.map(row => ({ ...row, event_id: event }))
  if (path.endsWith('/giveaways')) {
    let filtered = mode === 'empty' ? [] : all.filter(row => (!url.searchParams.has('state') || row.state === url.searchParams.get('state'))
      && (!url.searchParams.has('in_stock') || (row.available > 0) === (url.searchParams.get('in_stock') === 'true'))
      && (!url.searchParams.has('public_place_id') || row.public_place_id === url.searchParams.get('public_place_id'))
      && (!url.searchParams.has('pickup_after') || Date.parse(row.pickup_starts_at) >= Date.parse(url.searchParams.get('pickup_after')!))
      && (!url.searchParams.has('pickup_before') || Date.parse(row.pickup_starts_at) < Date.parse(url.searchParams.get('pickup_before')!)))
    const next = url.searchParams.get('cursor')
    if (next) filtered = filtered.slice(2); else filtered = filtered.slice(0, 2)
    const items = mode === 'leak' ? filtered.map(row => ({ ...row, recipients: [actor] })) : filtered
    return Response.json({ items, next_cursor: !next && !url.searchParams.has('state') && mode !== 'empty' ? 'catalog_2' : null })
  }
  const selected = all.find(row => row.giveaway_id === path.split('/').at(-1))
  if (!selected) return failure(404, 'RESOURCE_UNAVAILABLE')
  return Response.json(mode === 'leak' ? { ...selected, pickup_code: '123456' } : selected)
} })
let mounts = 0, disposals = 0, resources = 0, peakResources = 0
function bootstrap(scope: ActiveScope) {
  return { contractVersion: '0.1.0' as const, event: { eventId: scope.eventId, title: 'FE24 synthetic concert', phase: 'OPEN' as const },
    participant: { userId: scope.userId, eventId: scope.eventId }, capabilities: allCapabilities(), worldAvailable: true }
}
const ports: ShellPorts = { auth: f.auth, session: f.auth.session, readiness: { read: () => 'ready', subscribe: () => () => {} },
  bootstrap: { load: async scope => bootstrap(scope) }, admin: { authorize: async () => false }, world: {
    mount(host) {
      mounts++; resources++; peakResources = Math.max(peakResources, resources)
      const label = document.createElement('p'); label.textContent = 'Persistent shell owner · instrumented fixture only'; host.append(label)
      let stopped = false
      return { setInputBlocked() {}, setVisible() {}, dispose() { if (stopped) return; stopped = true; disposals++; resources--; label.remove() } }
    },
  } }
function Detail() { const { giveawayId = '' } = useParams(); return <GiftDetail controller={f.controller} giveawayId={giveawayId} places={places} /> }
history.replaceState(null, '', '/gifts')
const router = createBrowserRouter(createAppRoutes(ports, { panels: { '/gifts': <GiftDiscovery controller={f.controller} places={places} />,
  '/gifts/:giveawayId': <Detail />, '/gifts/mine': <MyGifts controller={f.controller} /> } }))
const root = createRoot(document.getElementById('root')!); root.render(<RouterProvider router={router} />)
const style = document.createElement('style'); style.textContent = 'body{padding:0}#qualification{font:12px/1.4 system-ui;padding:8px;background:#faf6ed}.world-shell{height:calc(100dvh - 50px)}'; document.head.append(style)
window.__FE24__ = {
  state: () => ({ view: f.controller.read(), requests: f.requests.map(request => request.url), mounts, disposals, resources, peakResources, contextListeners: f.listeners() }),
  async mode(value: string, target = 'list') { mode = value; if (target === 'detail') await f.controller.retryDetail(); else if (target === 'own') await f.controller.loadActivity(); else await f.controller.retryList() },
  policy() { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block') },
  rebind() { mode = 'ok'; f.rebind() },
  replace() { f.auth.session.replace(f.auth.session.read().generation, { userId: otherActor, eventId: uid(901), accessToken: 'replacement-fixture-token' }); f.rebind() },
  release() { mode = 'ok'; pending.splice(0).forEach(resolve => resolve()) },
  dispose() { if (disposed) return; disposed = true; f.dispose(); root.unmount(); router.dispose(); style.remove() },
}
declare global { interface Window { __FE24__: {
  state(): { view: ReturnType<typeof f.controller.read>; requests: string[]; mounts: number; disposals: number; resources: number; peakResources: number; contextListeners: number }
  mode(value: string, target?: string): Promise<void>; policy(): void; rebind(): void; replace(): void; release(): void; dispose(): void
} } }
