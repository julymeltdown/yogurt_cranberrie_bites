import type { Activity, Application, Party, Receipt } from '../../model/contract'
export const ids = { actor: '00000000-0000-4000-8000-000000000001', other: '00000000-0000-4000-8000-000000000002', event: '00000000-0000-4000-8000-000000000003',
  party: '00000000-0000-4000-8000-000000000010', next: '00000000-0000-4000-8000-000000000011', app: '00000000-0000-4000-8000-000000000020', place: '00000000-0000-4000-8000-000000000030', receipt: '00000000-0000-4000-8000-000000000040', key: '00000000-0000-4000-8000-000000000050' }
export const profile = (user_id = ids.other) => ({ user_id, display_name: user_id === ids.actor ? 'You · fixture' : 'Jules · fixture', bio: null,
  avatar: { revision: 1, config: { contract_version: '1.0.0', kind: 'avatar_config', manifest_id: 'fixture_only', manifest_version: 1,
    item_ids: { body: 'fixture_body', eyes: 'fixture_eyes', hair: 'fixture_hair', outfit: 'fixture_outfit', accessory: null }, skin_tone_id: 'fixture_tone' } } })
export const party = (overrides: Partial<Party> = {}): Party => ({ party_id: ids.party, event_id: ids.event, owner: profile(), title: 'Before the encore', description: 'Find a few people to share the concert with. We will agree on plans together.',
  ticket_section: 'Standing A', capacity: 5, member_count: 2, meeting_starts_at: '2026-11-08T08:00:00Z', meeting_ends_at: '2026-11-08T09:00:00Z', requirements: 'Be considerate. Check venue guidance before making plans.',
  created_at: '2026-11-01T01:00:00Z', state: 'OPEN', revision: 3, public_place_id: ids.place, meeting_revision: 1, ...overrides })
export const application = (overrides: Partial<Application> = {}): Application => ({ application_id: ids.app, event_id: ids.event, party_id: ids.party, applicant: profile(ids.actor), note: 'See you at the show.', created_at: '2026-11-03T01:00:00Z', state: 'PENDING', revision: 1, ...overrides })
export const activity = (): Activity => ({ owned: { items: [], next_cursor: null }, memberships: { items: [], next_cursor: null }, applications: { items: [], next_cursor: null } })
export const pendingReceipt = (key = ids.key, overrides: Partial<Receipt> = {}): Receipt => ({ receipt_id: ids.receipt, event_id: ids.event, actor_id: ids.actor, operation_id: 'applyConcertParty', idempotency_key: key,
  state: 'PENDING', resource_id: null, resource_revision: null, failure_reason: null, created_at: '2026-11-03T01:00:00Z', expires_at: '2099-11-03T01:00:00Z', result_status: null, ...overrides })
export const errorResponse = (status: number, reason: string) => Response.json({ code: 'FIXTURE', message: 'PRIVATE RAW SERVER MESSAGE MUST NEVER RENDER', details: { reason } }, { status })
