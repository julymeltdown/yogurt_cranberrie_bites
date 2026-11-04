import type { ShellBootstrap } from '../../../src/app/ports'
import type { PrivacyScope } from '../../../src/shared/query/privacy'
import { canonicalJson } from '../../../src/features/auth/model/bootstrap'

export interface HttpObservation {
  readonly sequence: number
  readonly method: string
  readonly path: string
  readonly status: number
  readonly noStore: boolean
}

/** Pass through real fetch without retaining credentials, bodies, query values or headers. */
export function observePublicHTTP() {
  const observations: HttpObservation[] = []
  let overflow = false
  let held: { path: string; claimed: boolean; received: boolean; status: number | null; released: boolean; gate: Promise<void>; release(): void } | null = null
  const fetch: typeof globalThis.fetch = async (input, init) => {
    const url = new URL(input instanceof Request ? input.url : String(input), location.origin)
    const method = (init?.method ?? (input instanceof Request ? input.method : 'GET')).toUpperCase()
    const delayed = url.origin === location.origin && method === 'GET' && held?.path === url.pathname && !held.claimed ? held : null
    if (delayed) delayed.claimed = true
    const response = await globalThis.fetch(input, init)
    if (url.origin === location.origin && url.pathname.startsWith('/api/concert/')) {
      if (observations.length >= 1024) overflow = true
      else observations.push(Object.freeze({ sequence: observations.length + 1,
        method,
        path: url.pathname, status: response.status,
        noStore: (response.headers.get('cache-control') ?? '').split(',').some(v => v.trim().toLowerCase() === 'no-store') }))
    }
    // Delay delivery only after the real upstream response arrives. No body/status is replaced.
    if (delayed && !delayed.released) { delayed.received = true; delayed.status = response.status; await delayed.gate }
    return response
  }
  return { fetch,
    holdNextProfileRead(path: string) {
      if (held || !/^\/api\/concert\/events\/[0-9a-f-]{36}\/people\/[0-9a-f-]{36}$/.test(path)) throw Error('IN03 delayed read unavailable')
      let resume = () => {}
      const gate = new Promise<void>(resolve => { resume = resolve })
      const current = { path, claimed: false, received: false, status: null as number | null, released: false, gate,
        release() { if (current.released) return; current.released = true; if (held === current) held = null; resume() } }
      held = current
      return { read: () => ({ received: current.received, status: current.status }), release: current.release }
    }, dispose() { held?.release() }, read() {
    if (overflow) throw new Error('IN03 HTTP observation limit exceeded; evidence is incomplete')
    return observations.slice()
  } }
}

export interface SourcePins {
  readonly frontendRevision: string
  readonly backendRevision: string
  readonly contractRevision: string
}

/** Validate in memory; persist only public manifests/outcomes, never authenticated C03. */
export async function captureBootstrap(bootstrap: ShellBootstrap, scope: PrivacyScope,
  http: readonly HttpObservation[], source: SourcePins) {
  if (![source.frontendRevision, source.backendRevision, source.contractRevision].every(v => typeof v === 'string' && /^[0-9a-f]{40}$/.test(v))) throw new Error('NOT_RUN: missing source pins')
  const endpoint = `/api/concert/events/${scope.eventId}/bootstrap`
  const response = [...http].reverse().find(v => v.method === 'GET' && v.path === endpoint)
  if (!bootstrap.document || response?.status !== 200 || bootstrap.participant.userId !== scope.userId
    || bootstrap.participant.eventId !== scope.eventId || bootstrap.event.eventId !== scope.eventId)
    throw new Error('NOT_RUN: no successful validated C03 response for this identity')
  const document = bootstrap.document
  const projection = { map_manifest: document.map_manifest, theme_manifests: document.theme_manifests,
    avatar_manifest: document.avatar_manifest,
    capabilities: Object.fromEntries(Object.entries(bootstrap.capabilities).map(([key, value]) => [key, value.available])),
    worldAvailable: bootstrap.worldAvailable }
  const body = canonicalJson(projection)
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(body))
  const sha256 = Array.from(new Uint8Array(hash), b => b.toString(16).padStart(2, '0')).join('')
  return { format: 'IN03_PUBLIC_C03_PROJECTION_V1', classification: 'NOT_BYTE_EXACT_FULL_C03',
    source, endpoint: '/api/concert/events/{eventId}/bootstrap', status: response.status,
    identityBindingValidatedInMemory: true, worldAvailable: bootstrap.worldAvailable,
    bodyEncoding: 'canonical JSON of public manifest descriptors and capability availability only', sha256, body }
}
