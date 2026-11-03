// Fixture-only C04 HTTP data for the actual default root harness; no route/panel overrides.
import management from '../../../../../../contracts/fixtures/domain/giveaway-management.json'
import { eventId } from '../../../../app/world-entry/__tests__/data'
import { response as fallback } from '../../../../app/social-composition/__tests__/fixtures'
const id = (n: number) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, '0')}`
const now = Date.now()
const rows = ['Sticker pack', 'Hand banner', 'Custom button'].map((title, i) => ({ ...structuredClone(management.giveaway), event_id: eventId,
  giveaway_id: id(100 - i), title, image_media_id: null, owner: { ...management.giveaway.owner, user_id: id(900) },
  created_at: new Date(now - i * 60000).toISOString(), pickup_starts_at: new Date(now - 60000).toISOString(), pickup_ends_at: new Date(now + 86400000).toISOString(),
}))
export function response(r: Request) {
  const path = new URL(r.url).pathname
  if (path.endsWith('/me/giveaways')) return Response.json({ giving: { items: [], next_cursor: null }, receiving: { items: [], next_cursor: null } })
  if (path.endsWith('/giveaways')) return Response.json({ items: rows, next_cursor: null })
  if (path.includes('/giveaways/')) return Response.json(rows.find(row => path.endsWith(row.giveaway_id)))
  return fallback(r)
}
