import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { GiftDiscovery } from '../../ui/Gifts'
import { ConcertWindow } from '../../../../shared/ui/concert-window'
import { allCapabilities, fixture, giveaway, now, uid } from '../fixtures/data'
import '../../../../shared/styles/base.css'
import './preview.css'
const params = new URLSearchParams(location.search), scenario = params.get('scenario') ?? 'open'
const titles = ['Fixture fan badges', 'Fixture ribbon keepsake', 'Fixture printed zine']
const rows = titles.map((title, index) => giveaway({ giveaway_id: uid(100 - index), title,
  created_at: `2026-11-03T00:${50 - index * 10}:00Z`, image_media_id: null,
  ...(scenario === 'unavailable' ? index === 0 ? { state: 'PAUSED' as const } : index === 1 ? { available: 0 } : { pickup_starts_at: '2026-11-03T08:00:00Z', pickup_ends_at: '2026-11-03T09:00:00Z' } : {}),
  ...(scenario === 'long' ? { title: `${title} — a longer descriptive item title`, owner: { ...giveaway().owner, display_name: 'Fixture giver with a long public name' } } : {}),
}))
let mode = scenario
const f = fixture({ handler: request => {
  const path = new URL(request.url).pathname
  if (mode === 'error') return Response.json({ code: 'FIXTURE', message: 'DO NOT DISPLAY PRIVATE TRANSPORT TEXT', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
  if (path.endsWith('/me/giveaways')) return Response.json({ giving: { items: [giveaway({ title: 'Fixture private draft', state: 'DRAFT', owner: { ...giveaway().owner, user_id: f.auth.session.read().userId! } })], next_cursor: null }, receiving: { items: [], next_cursor: null } })
  return Response.json({ items: mode === 'empty' ? [] : rows, next_cursor: null })
} })
if (scenario === 'read-only') f.publish({ scope: f.auth.privacy.capture(), capabilities: { ...allCapabilities(), mutations: { available: false, reason: 'EVENT_READ_ONLY' } }, eventPhase: 'READ_ONLY' })
const root = createRoot(document.getElementById('root')!)
root.render(<BrowserRouter><main className="reference-gifts-fixture"><ConcertWindow title="GIVEAWAY" onClose={() => { history.back() }}>
  <GiftDiscovery controller={f.controller} places={[{ id: giveaway().public_place_id, label: 'Fixture public place' }]} />
</ConcertWindow><aside className="reference-gifts-fixture-label">Production Gifts component · synthetic contract data · no photos supplied · shared root footer integration pending</aside></main></BrowserRouter>)
window.referenceGifts = {
  state: () => ({ requests: f.requests.map(r => ({ method: r.method, url: r.url })), view: f.controller.read(), now }),
  invalidate: () => f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block'),
  retry: async () => { mode = 'open'; await f.controller.retryList() },
  dispose: () => { root.unmount(); f.dispose() },
}
declare global { interface Window { referenceGifts: { state(): unknown; invalidate(): void; retry(): Promise<void>; dispose(): void } } }
