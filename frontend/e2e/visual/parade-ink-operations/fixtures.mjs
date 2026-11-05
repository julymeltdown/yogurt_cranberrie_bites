// TEST_ONLY pure public-contract data, selected verbatim getters from canonical event fixture. No Auth/controller imports.
export const uid = (n) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, '0')}`
export const actorId = uid(1), eventId = uid(2), placeId = uid(3), noticeId = uid(4), key = uid(5)
export const now = Date.parse('2026-11-04T04:00:00Z')
export const time = (delta = 0) => new Date(now + delta).toISOString()
export function place() { return { revision: 4, place: { place_id: placeId, label: 'SYNTHETIC landmark', place_kind: 'PHYSICAL', verification_state: 'VERIFIED', closure_state: 'OPEN',
  accessibility_notes: 'SYNTHETIC access notes; no real venue claim.', provenance: { source_id: 'synthetic_review', source_kind: 'PLACE_REFERENCE', source_revision: 'SYNTHETIC release 1',
    source_uri: 'https://venue.example.invalid/review', author: 'SYNTHETIC author', license: 'SYNTHETIC fixture only', review_state: 'REVIEWED', reviewed_by: 'SYNTHETIC reviewer',
    reviewed_at: time(-86400000), photo_permission: 'DENIED' } } } }
export function notice() { return { notice_id: noticeId, event_id: eventId, revision: 3, title: 'SYNTHETIC public notice', text: 'SYNTHETIC reviewed public text.',
  source_uri: 'https://notice.example.invalid/source', confirmed_at: time(-60000), created_at: time(-120000), state: 'PUBLISHED' } }
