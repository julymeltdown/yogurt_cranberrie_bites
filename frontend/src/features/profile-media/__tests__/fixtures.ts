import { QueryClient } from '@tanstack/react-query'
import { createHttpClient, createMemorySession } from '../../../shared/api/http'
import { createQueryPrivacy, type PrivacyScope } from '../../../shared/query/privacy'
import type { UploadAuth } from '../upload'

/** The account every fixture is bound to. */
export const ACCOUNT = '00000000-0000-4000-8000-000000000901'
/** The profile the gallery reads. */
export const PROFILE = '00000000-0000-4000-8000-000000000902'
/** An event id that must never appear in a profile post or media request. */
export const EVENT = '00000000-0000-4000-8000-000000000903'

export type RecordedCall = Readonly<{
  path: string
  method: string
  body: unknown
  contentType: string | null
  idempotencyKey: string | null
}>

export type Handler = (request: Request, call: RecordedCall) => Response | Promise<Response>

/** A deterministic PNG-like byte payload. */
export const BYTES = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4])
/** The digest the fixture reports for `BYTES`. */
export const DIGEST = 'a'.repeat(64)

/**
 * The profile media fixture.
 *
 * It composes the *real* memory session, privacy coordinator and HTTP client and
 * stubs only `fetch`, so every assertion about a request path, header or body is
 * an assertion about what the production adapter actually sends.
 */
export async function setup() {
  // The shared HTTP client requires a browser origin; jsdom supplies one, but a
  // node-environment test does not.
  if (globalThis.location === undefined) {
    Object.defineProperty(globalThis, 'location', { value: { origin: 'https://mcr.test' }, configurable: true })
  }
  const session = createMemorySession()
  const client = new QueryClient()
  const privacy = createQueryPrivacy({ session, client })
  session.replace(0, { userId: ACCOUNT, eventId: EVENT, accessToken: 'deterministic-test-token' })
  const scope: PrivacyScope = privacy.capture()
  const calls: RecordedCall[] = []
  let handler: Handler | null = null
  let counter = 0
  const http = createHttpClient({ session, fetch: async input => {
    if (!(input instanceof Request)) throw new Error('Expected canonical Request')
    const path = new URL(input.url).pathname
    const contentType = input.headers.get('content-type')
    // A raw upload is an image Blob; everything else is JSON. The Blob is read
    // through its own array buffer so the recorded value is exactly the bytes
    // the client put on the wire.
    const body: unknown = input.method === 'GET'
      ? null
      : path.endsWith('/bytes')
        ? new Uint8Array(await input.blob().then(blob => blob.arrayBuffer()) as ArrayBuffer)
        : await input.clone().json()
    const call: RecordedCall = {
      path,
      method: input.method,
      body,
      contentType,
      idempotencyKey: input.headers.get('idempotency-key'),
    }
    calls.push(call)
    if (!handler) throw new Error(`Unexpected profile media request: ${input.method} ${path}`)
    return handler(input, call)
  } })
  const auth: UploadAuth = { session, privacy, request: http.request }
  const signal = new AbortController()
  return {
    auth,
    scope,
    calls,
    session,
    privacy,
    handler: (next: Handler | null) => { handler = next },
    /** A fresh local id, so a test's ids are stable. */
    nextLocalId: () => `local-${++counter}`,
    digest: async (bytes: Uint8Array) => {
      // The fixture's digest is the one it advertises for `BYTES`; a caller that
      // sends different bytes gets a different digest, so a mismatch is
      // observable rather than hidden by a constant.
      const text = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')
      return text === Array.from(BYTES, byte => byte.toString(16).padStart(2, '0')).join('') ? DIGEST : 'b'.repeat(64)
    },
    dispose() { signal.abort(); privacy.dispose(); client.clear() },
  }
}

export type Fixture = Awaited<ReturnType<typeof setup>>

/** A `feed.Media` projection of a READY PROFILE_POST asset. */
export function media(overrides: Partial<Record<string, unknown>> = {}) {
  const id = (overrides.media_id as string | undefined) ?? '00000000-0000-4000-8000-0000000009aa'
  return {
    media_id: id,
    state: 'READY',
    revision: 3,
    width: 1200,
    height: 800,
    content_type: 'image/png',
    purpose: 'PROFILE_POST',
    read_path: `/api/media/${id}/content`,
    ...overrides,
  }
}

/** A `social.ProfilePost`. */
export function post(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    post_id: '00000000-0000-4000-8000-0000000009bb',
    author: { profile_id: PROFILE },
    description: 'a photograph',
    media: [media()],
    visibility: 'PUBLIC',
    created_at: '2026-11-05T00:00:00Z',
    expires_at: null,
    revision: 1,
    ...overrides,
  }
}

/** A canonical JSON response. */
export const json = (value: unknown, status = 200) =>
  Response.json(value, { status, headers: { 'content-type': 'application/json' } })

/** A closed profile error envelope. */
export const failure = (status: number, reason: string) => json({
  code: `PROFILE_${status}`,
  message: 'Profile request unavailable',
  details: { reason },
}, status)
