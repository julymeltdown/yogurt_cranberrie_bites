// TEST_ONLY. The production image renderer, catalogs and root fixture are unchanged.
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { GiftDiscovery } from '../../../../frontend/src/features/gift-discovery/ui/Gifts'
import type { RenderGiftImage } from '../../../../frontend/src/features/gift-discovery/ui/reference-presentation'
import { fixture, giveaway, uid } from '../../../../frontend/src/features/gift-discovery/__tests__/fixtures/data'
import { ConcertWindow } from '../../../../frontend/src/shared/ui/concert-window'
import '../../../../frontend/src/shared/styles/base.css'
import '../../../../frontend/src/features/gift-discovery/__tests__/reference-native/preview.css'
import './native.css'

// Existing root-touch fixture row identities/titles; unmodified management quantities.
// The one retained media-entry GIVEAWAY fixture media ID is scoped by row here.
// This composite synthetic mapping is not a production media/cache contract proof.
const mediaId = '00000000-0000-4000-8000-000000000004'
const definitions = [
  { giveawayId: uid(100), title: 'Sticker pack', file: 'sticker-pack.svg' },
  { giveawayId: uid(99), title: 'Hand banner', file: 'hand-banner.svg' },
  { giveawayId: uid(98), title: 'Custom button', file: 'diy-buttons.svg' },
] as const
const rows = definitions.map((row, index) => giveaway({ giveaway_id: row.giveawayId,
  title: row.title, image_media_id: mediaId, created_at: `2026-11-03T00:${50 - index * 10}:00Z` }))
const f = fixture({ handler: request => {
  const path = new URL(request.url).pathname
  if (path.endsWith('/me/giveaways')) return Response.json({ giving: { items: [], next_cursor: null }, receiving: { items: [], next_cursor: null } })
  if (path.endsWith('/giveaways')) return Response.json({ items: rows, next_cursor: null })
  return Response.json({ code: 'TEST_ONLY_NOT_ROUTED' }, { status: 404 })
} })
const renderGiftImage: RenderGiftImage = request => {
  const definition = definitions.find(row => row.giveawayId === request.giveawayId)
  const context = f.context()
  if (!definition || request.mediaId !== mediaId || !context
    || request.scope.userId !== context.scope.userId || request.scope.eventId !== context.scope.eventId) return null
  return <img src={`/reference-gifts-content-v1/${definition.file}`} alt={request.alt}
    width={256} height={256} data-art-fixture={definition.file} />
}
const root = createRoot(document.getElementById('root')!)
root.render(<BrowserRouter><main className="reference-gifts-fixture">
  <ConcertWindow title="GIVEAWAY"><GiftDiscovery controller={f.controller}
    places={[{ id: giveaway().public_place_id, label: 'Fixture public place' }]} renderGiftImage={renderGiftImage} /></ConcertWindow>
  <p className="art-fixture-notice">TEST ONLY · original reference art with retained synthetic contract identities and quantities. No live inventory or production media binding.</p>
</main></BrowserRouter>)
const api = {
  usage: 'TEST_ONLY_REFERENCE_FIXTURE', productionBinding: null,
  state: () => ({ view: f.controller.read(), definitions, quantitiesFromContract: true,
    requests: f.requests.map(request => ({ method: request.method, path: new URL(request.url).pathname })) }),
  invalidate: () => { f.publish(null) },
  dispose: () => { root.unmount(); f.dispose() },
}
window.referenceGiftsArt = api
declare global { interface Window { referenceGiftsArt: typeof api } }
