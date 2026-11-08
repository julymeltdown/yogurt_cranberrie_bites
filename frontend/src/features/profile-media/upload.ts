/**
 * WP-06 T-506/T-510: the profile media upload client.
 *
 * One upload is three requests, exactly as the frozen contract has it:
 *
 * 1. `POST /api/me/media/uploads` — declare purpose `PROFILE_POST`, content
 *    type, byte length and SHA-256. The server answers a `PENDING` intent.
 * 2. `PUT /api/me/media/{id}/bytes` — send exactly those bytes.
 * 3. `POST /api/me/media/{id}/finalize` — the server sanitizes and answers a
 *    `READY` asset.
 *
 * # Cancellation leaves no orphan
 *
 * A cancelled upload must not leave a `PENDING` or `UPLOADED` asset behind. The
 * client cannot delete a `PENDING` row — the frozen contract has no such
 * operation — so the guarantee is made where it can be:
 *
 * * **Before `init`**: nothing has been created, so there is nothing to orphan.
 * * **Before `put`**: the intent exists but holds no bytes. The client never
 *   sends the bytes after a cancellation, so the row stays `PENDING` with no
 *   content and is erased by the server's orphan window (24 h). The client
 *   records it as abandoned so a later composer session does not resurrect it.
 * * **Before `finalize`**: the bytes are staged but the asset is not `READY`,
 *   so it is not published and is likewise erased by the orphan window.
 *
 * The client therefore never *publishes* a cancelled upload, and the only state
 * it can leave behind is one the server is designed to clean. `cancel()` is
 * idempotent and every request carries the caller's abort signal, so an
 * in-flight request is aborted rather than allowed to finish.
 *
 * # A failed upload keeps the draft
 *
 * A failure is reported as a value, never thrown at the caller, and the caller's
 * draft (description, visibility, alt texts) is never mutated by this module.
 * The composer re-renders the same draft with a retry affordance.
 */
import type { HttpClient, SessionPort } from '../../shared/api/http'
import { TransportError } from '../../shared/api/errors'
import type { PrivacyScope } from '../../shared/query/privacy'
import {
  decodeMedia,
  decodeUploadBytesResult,
  decodeUploadIntent,
  PATHS,
  type ProfileMedia,
  type ProfileMediaPurpose,
} from './contract'

/** The closed upload failure set the composer distinguishes. */
export type UploadFailure =
  | { readonly kind: 'cancelled' }
  | { readonly kind: 'unsupported-type' }
  | { readonly kind: 'too-large' }
  | { readonly kind: 'invalid' }
  | { readonly kind: 'unavailable'; readonly retryAfterMs: number | null }
  | { readonly kind: 'unknown'; readonly uncertain: boolean }

/** The observable phase of one upload. */
export type UploadPhase = 'idle' | 'initializing' | 'uploading' | 'finalizing' | 'ready' | 'failed' | 'cancelled'

/** One upload's state. */
export type UploadState = Readonly<{
  localId: string
  phase: UploadPhase
  /** The declared intent, once `init` succeeded. */
  mediaId: string | null
  /** The staged revision, once `put` succeeded. */
  revision: number | null
  /** The `READY` asset, once `finalize` succeeded. */
  media: ProfileMedia | null
  failure: UploadFailure | null
  /** Bytes sent so far, for a progress affordance. */
  sent: number
  total: number
}>

/** The minimal auth surface this client needs. */
export type UploadAuth = Readonly<{
  session: SessionPort
  privacy: { run<T>(scope: PrivacyScope, work: (context: { readonly signal: AbortSignal }) => T | Promise<T>, signal?: AbortSignal): Promise<T> }
  request: HttpClient['request']
}>

export type UploadDeps = Readonly<{
  auth: UploadAuth
  scope: PrivacyScope
  /**
   * The SHA-256 of the bytes, as lowercase hex. Injected so a test can supply a
   * deterministic digest and so the browser implementation is the only place
   * that touches `crypto.subtle`.
   */
  digest: (bytes: Uint8Array) => Promise<string>
  /** Monotonic local id source. Injected for deterministic tests. */
  nextLocalId: () => string
  /** The idempotency key source. Injected so a test can pin it. */
  idempotencyKey?: () => string
}>

const HEX64 = /^[0-9a-f]{64}$/

/** The content types the foundation accepts. */
const ACCEPTED = ['image/jpeg', 'image/png', 'image/webp'] as const
export type AcceptedType = typeof ACCEPTED[number]

export const isAcceptedType = (value: string): value is AcceptedType =>
  (ACCEPTED as readonly string[]).includes(value)

/** Map a transport failure to the closed upload failure set. */
export function classifyFailure(error: unknown, signal: AbortSignal): UploadFailure {
  if (signal.aborted) return { kind: 'cancelled' }
  if (error instanceof TransportError) {
    switch (error.kind) {
      case 'aborted':
      case 'session-changed':
        return { kind: 'cancelled' }
      case 'invalid-response':
      case 'invalid-request':
        return { kind: 'invalid' }
      case 'http':
        switch (error.status) {
          case 415:
            return { kind: 'unsupported-type' }
          case 413:
            return { kind: 'too-large' }
          case 400:
            return { kind: 'invalid' }
          case 404:
            return { kind: 'invalid' }
          default:
            return { kind: 'unavailable', retryAfterMs: error.retryAfterMs }
        }
      default:
        return { kind: 'unavailable', retryAfterMs: null }
    }
  }
  return { kind: 'unknown', uncertain: false }
}

/** A human-readable line for a failure. Never includes a server diagnostic. */
export function failureMessage(failure: UploadFailure): string {
  switch (failure.kind) {
    case 'cancelled':
      return 'Upload cancelled.'
    case 'unsupported-type':
      return 'Choose a JPEG, PNG or WebP image. GIF and video are not supported.'
    case 'too-large':
      return 'Choose an image of 10 MiB or smaller.'
    case 'invalid':
      return 'That image could not be uploaded. Choose another file.'
    case 'unavailable':
      return 'The upload service is unavailable. Try again.'
    case 'unknown':
      return 'The upload did not complete. Try again.'
  }
}

/** Whether a failure is worth retrying. */
export const retryable = (failure: UploadFailure): boolean =>
  failure.kind === 'unavailable' || failure.kind === 'unknown'

/**
 * One profile media upload.
 *
 * `start()` runs the three steps. It resolves with the final state; it never
 * rejects, because a failure is part of the state the composer renders.
 */
export function createProfileMediaUpload(deps: UploadDeps) {
  const { auth, scope, digest, nextLocalId } = deps
  const key = deps.idempotencyKey ?? cryptoIdempotencyKey
  const lifetime = new AbortController()
  let state: UploadState = Object.freeze({
    localId: nextLocalId(),
    phase: 'idle',
    mediaId: null,
    revision: null,
    media: null,
    failure: null,
    sent: 0,
    total: 0,
  })
  const listeners = new Set<() => void>()
  let cancelled = false
  /** Every media id this upload created, so a cancellation can report them. */
  const created: string[] = []

  const publish = (next: Partial<UploadState>) => {
    state = Object.freeze({ ...state, ...next })
    for (const listener of Array.from(listeners)) listener()
  }

  /** Abort the in-flight request and mark the upload cancelled. */
  function cancel() {
    if (cancelled) return
    cancelled = true
    lifetime.abort()
    publish({ phase: 'cancelled', failure: { kind: 'cancelled' } })
  }

  function assertLive() {
    if (cancelled || lifetime.signal.aborted) throw new TransportError('aborted')
  }

  async function run(bytes: Uint8Array, contentType: string): Promise<UploadState> {
    if (cancelled) return state
    if (!isAcceptedType(contentType)) {
      publish({ phase: 'failed', failure: { kind: 'unsupported-type' } })
      return state
    }
    const total = bytes.byteLength
    publish({ phase: 'initializing', sent: 0, total, failure: null })
    try {
      const sha256 = await digest(bytes)
      if (!HEX64.test(sha256)) throw new TransportError('invalid-request')
      assertLive()
      const intent = await auth.privacy.run(scope, ({ signal }) => auth.request({
        path: PATHS.initUpload,
        method: 'POST',
        access: 'protected',
        errorFamily: 'domain',
        domainOperation: 'initProfileMediaUpload',
        idempotencyKey: key(),
        signal: combine(signal, lifetime.signal),
        body: {
          json: {
            purpose: 'PROFILE_POST' satisfies ProfileMediaPurpose,
            content_type: contentType as AcceptedType,
            byte_length: total,
            sha256,
          },
        },
        decode: decodeUploadIntent,
      }), lifetime.signal)
      assertLive()
      created.push(intent.data.media_id)
      publish({ phase: 'uploading', mediaId: intent.data.media_id })
      const uploaded = await auth.privacy.run(scope, ({ signal }) => auth.request({
        path: PATHS.putBytes(intent.data.media_id),
        method: 'PUT',
        access: 'protected',
        errorFamily: 'domain',
        domainOperation: 'putProfileMediaBytes',
        idempotencyKey: key(),
        signal: combine(signal, lifetime.signal),
        body: { binary: imageBlob(bytes, contentType) },
        decode: decodeUploadBytesResult,
      }), lifetime.signal)
      assertLive()
      publish({ phase: 'finalizing', revision: uploaded.data.revision, sent: total })
      const finalized = await auth.privacy.run(scope, ({ signal }) => auth.request({
        path: PATHS.finalize(intent.data.media_id),
        method: 'POST',
        access: 'protected',
        errorFamily: 'domain',
        domainOperation: 'finalizeProfileMedia',
        idempotencyKey: key(),
        signal: combine(signal, lifetime.signal),
        body: { json: { expected_revision: uploaded.data.revision } },
        decode: decodeMedia,
      }), lifetime.signal)
      assertLive()
      publish({ phase: 'ready', media: finalized.data, failure: null })
      return state
    } catch (error) {
      if (cancelled || lifetime.signal.aborted) {
        publish({ phase: 'cancelled', failure: { kind: 'cancelled' } })
        return state
      }
      publish({ phase: 'failed', failure: classifyFailure(error, lifetime.signal) })
      return state
    }
  }

  return Object.freeze({
    /** The current state. Stable identity until it changes. */
    read: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => { listeners.delete(listener) }
    },
    /** Run the three steps. Resolves with the final state; never rejects. */
    start: run,
    /** Cancel. Idempotent. Leaves nothing published and nothing to retry. */
    cancel,
    /**
     * The media ids this upload created. A cancelled upload reports them so the
     * caller can show that the staged asset will be cleaned by the server's
     * orphan window rather than pretending it never existed.
     */
    createdMediaIds: () => [...created],
    /** Whether the upload is finished, either way. */
    settled: () => state.phase === 'ready' || state.phase === 'failed' || state.phase === 'cancelled',
  })
}

export type ProfileMediaUpload = ReturnType<typeof createProfileMediaUpload>

/** Combine two abort signals without leaking a listener. */
function combine(a: AbortSignal, b: AbortSignal): AbortSignal {
  if (typeof AbortSignal.any === 'function') return AbortSignal.any([a, b])
  const controller = new AbortController()
  const abort = () => controller.abort()
  if (a.aborted || b.aborted) controller.abort()
  else {
    a.addEventListener('abort', abort, { once: true })
    b.addEventListener('abort', abort, { once: true })
  }
  return controller.signal
}

/**
 * The bytes as a `Blob` of the declared type.
 *
 * The shared HTTP client only accepts a `Blob` for a raw upload, and it checks
 * the blob's size and type against the same limits this feature does. Wrapping
 * here keeps the caller's `Uint8Array` immutable: `Blob` copies its input.
 */
function imageBlob(bytes: Uint8Array, contentType: string): Blob {
  const copy = new Uint8Array(bytes.byteLength)
  copy.set(bytes)
  return new Blob([copy], { type: contentType })
}

/** A fresh canonical idempotency key for one request. */
export function cryptoIdempotencyKey(): string {
  const bytes = new Uint8Array(16)
  globalThis.crypto.getRandomValues(bytes)
  bytes[6] = (bytes[6]! & 0x0f) | 0x40
  bytes[8] = (bytes[8]! & 0x3f) | 0x80
  const hex = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}
