import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../../shared/api/http'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { decodeBootstrap } from '../../../features/auth/model/bootstrap'
import type { ShellBootstrap } from '../../ports'
import base from '../../../../../contracts/fixtures/core/bootstrap-open.json'
import { contentHash } from '../../../world/map/json'
import { createAuthorityTimeline, createCommunityAuthority, type CommunityAuthorityOptions } from '..'

export const userId = base.participant.user_id, eventId = base.event.event_id
export const checkedBootstrap = () => decodeBootstrap(base, { userId, eventId, generation: 1 })
export async function physicalBootstrap(change: (place: Record<string, unknown>) => void = () => {}) {
  const b = structuredClone(base), p = b.map_manifest.places[0]!
  Object.assign(p, { place_kind: 'PHYSICAL', verification_state: 'VERIFIED',
    provenance: { ...p.provenance, source_kind: 'PLACE_REFERENCE', source_uri: 'https://venue.example.invalid/source',
      review_state: 'REVIEWED', reviewed_by: 'Synthetic reviewer', reviewed_at: '2026-11-01T00:00:00Z' } })
  change(p)
  b.map_manifest.content_hash = await contentHash(b.map_manifest)
  for (const theme of b.theme_manifests) { theme.map_content_hash = b.map_manifest.content_hash; theme.content_hash = await contentHash(theme) }
  return decodeBootstrap(b, { userId, eventId, generation: 1 })
}
export function fixture(bootstrap: ShellBootstrap, patch: Partial<CommunityAuthorityOptions> = {}) {
  const session = createMemorySession(), client = new QueryClient(), privacy = createQueryPrivacy({ session, client })
  session.replace(0, { userId, eventId, accessToken: 'SYNTHETIC_ONLY' })
  const scope = privacy.capture(), lifetime = new AbortController()
  let ms = 100, origin = 12345, interrupt = () => {}
  const timeline = createAuthorityTimeline({ observe: () => ({ nowMs: ms, origin }), maxObservationGapMs: 2000,
    onInterruption(fn) { interrupt = fn; return () => { interrupt = () => {} } } })
  const requestStarted = timeline.capture()!; ms = 600; const responseReceived = timeline.capture()!
  const authority = createCommunityAuthority({ bootstrap, scope, privacy, signal: lifetime.signal, timeline, requestStarted, responseReceived,
    qualification: { maxAgeMs: 10_000, uncertaintyMs: 0, maxElapsedRate: 1 }, ...patch })
  return { authority, scope, session, privacy, client, timeline, lifetime, requestStarted, responseReceived,
    set(value: number) { ms = value }, setOrigin(value: number) { origin = value }, interrupt: () => interrupt(),
    dispose() { authority.dispose(); timeline.dispose(); privacy.dispose(); client.clear() } }
}
