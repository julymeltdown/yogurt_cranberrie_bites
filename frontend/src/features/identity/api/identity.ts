import { z } from 'zod'
import type { SessionScope } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import { send, type RequestPort } from '../../../shared/api/request'
import { freeze, timestamp } from '../../people/contract'

/**
 * The four `core` identity operations, as the frozen contract declares them
 * (`contracts/core/v1/operations.json`, `core.schema.json`).
 *
 * These are `core` operations, not `domain` ones: they carry no event, no
 * domain operation id and no idempotency key. The server binds them to the
 * authenticated session, and the client never sends a user id, an age or a
 * gender — the whole point of the flow is that those come from the provider.
 */
export const IDENTITY_PROVIDER = 'naver' as const
export type IdentityProvider = typeof IDENTITY_PROVIDER

/** The closed status set of `core.IdentityVerification.status`. */
export const VERIFICATION_STATUSES = [
  'NOT_VERIFIED',
  'VERIFIED',
  'EXPIRED',
  'REVOKED',
  'PROVIDER_UNAVAILABLE',
] as const
export type VerificationStatus = (typeof VERIFICATION_STATUSES)[number]

/** The closed gender bucket set. `unknown` is a real provider answer, not a wildcard. */
export const GENDER_BUCKETS = ['female', 'male', 'unknown'] as const
export type GenderBucket = (typeof GENDER_BUCKETS)[number]

/**
 * The closed set of return paths. This mirrors the server's own allowlist; the
 * client never sends a path outside it, and the callback `Location` can only
 * ever be one of these constants.
 */
export const RETURN_PATHS = ['/me/identity', '/me/settings', '/me/profile'] as const
export type ReturnPath = (typeof RETURN_PATHS)[number]

/**
 * The closed set of callback result markers. The server appends exactly one of
 * these as `?identity=<marker>`; anything else is treated as an unavailable
 * outcome rather than shown to the user.
 */
export const RESULT_MARKERS = [
  'success',
  'duplicate',
  'profile_incomplete',
  'age_not_supported',
  'failed',
  'cancelled',
  'expired',
  'replayed',
] as const
export type ResultMarker = (typeof RESULT_MARKERS)[number]

const claimsSchema = z.strictObject({
  age: z.number().int().min(14).max(100),
  gender_bucket: z.enum(GENDER_BUCKETS),
})

/**
 * `core.IdentityVerification`. `claims` is present only when the status is
 * `VERIFIED`; the decoder enforces that, so an expired or revoked claim can
 * never be read as an attribute by a component.
 */
export const verificationSchema = z
  .strictObject({
    provider: z.literal(IDENTITY_PROVIDER),
    status: z.enum(VERIFICATION_STATUSES),
    verified_at: timestamp.nullable(),
    expires_at: timestamp.nullable(),
    claims: claimsSchema.nullable(),
    display_on_profile: z.boolean(),
  })
  .refine(
    (value) =>
      value.status === 'VERIFIED'
        ? value.claims !== null && value.verified_at !== null && value.expires_at !== null
        : value.claims === null,
    { message: 'claims are present only for a verified claim' },
  )
  .refine(
    (value) =>
      value.verified_at === null ||
      value.expires_at === null ||
      Date.parse(value.expires_at) > Date.parse(value.verified_at),
    { message: 'a claim must expire after it was verified' },
  )
export type IdentityVerification = z.infer<typeof verificationSchema>

/** `core.IdentityVerificationStart`. */
export const startSchema = z.strictObject({
  authorization_url: z
    .string()
    .max(2048)
    .url()
    // The provider URL is followed by the browser, so it must be https and must
    // not carry credentials or a fragment.
    .refine((value) => {
      const url = new URL(value)
      return (
        url.protocol === 'https:' &&
        !url.username &&
        !url.password &&
        !url.hash &&
        url.searchParams.get('response_type') === 'code' &&
        !!url.searchParams.get('state')
      )
    }, 'the authorization URL must be a plain https authorization request'),
  expires_at: timestamp,
})
export type IdentityVerificationStart = z.infer<typeof startSchema>

/** Decode a `core.IdentityVerification` for the given session scope. */
export function decodeVerification(value: unknown): IdentityVerification {
  return freeze(verificationSchema.parse(value))
}

/** Decode a `core.IdentityVerificationStart`, bounded to the state lifetime. */
export function decodeStart(value: unknown, now: number): IdentityVerificationStart {
  const start = startSchema.parse(value)
  const expires = Date.parse(start.expires_at)
  // The state lives at most ten minutes. A server answer outside that window is
  // not one this client issued.
  if (expires <= now || expires > now + 10 * 60_000) throw new Error('Invalid verification start')
  return freeze(start)
}

export interface IdentityApiOptions extends RequestPort {
  readonly now?: () => number
}

/**
 * The identity API surface, one method per frozen operation.
 *
 * Every method is authenticated (`access: 'protected'`) except the callback,
 * which the browser reaches by navigation and never through this client.
 */
export function createIdentityApi(options: IdentityApiOptions) {
  const now = options.now ?? Date.now
  const base = `/api/me/identity/${IDENTITY_PROVIDER}`
  return {
    /** `getOwnIdentityVerification` — GET `/api/me/identity/{provider}`. */
    async read(): Promise<IdentityVerification> {
      return send(options, {
        path: base,
        method: 'GET',
        access: 'protected',
        errorFamily: 'core',
        decode: (value: unknown) => decodeVerification(value),
      })
    },
    /** `startIdentityVerification` — POST `/api/me/identity/{provider}/verifications`. */
    async start(returnPath: ReturnPath): Promise<IdentityVerificationStart> {
      if (!RETURN_PATHS.includes(returnPath)) throw new TransportError('invalid-request')
      return send(options, {
        path: `${base}/verifications`,
        method: 'POST',
        access: 'protected',
        errorFamily: 'core',
        body: { json: { return_path: returnPath } },
        decode: (value: unknown) => decodeStart(value, now()),
      })
    },
    /** `disconnectIdentityVerification` — DELETE `/api/me/identity/{provider}`. */
    async disconnect(): Promise<IdentityVerification> {
      return send(options, {
        path: base,
        method: 'DELETE',
        access: 'protected',
        errorFamily: 'core',
        decode: (value: unknown) => decodeVerification(value),
      })
    },
  }
}
export type IdentityApi = ReturnType<typeof createIdentityApi>

/**
 * Read the callback result marker from the current location.
 *
 * The callback is a server redirect (`303`), so the landing page is an ordinary
 * route whose query carries the marker. This is the only place the marker is
 * read, and an unknown marker is reported as `null` rather than displayed: a
 * value that is not one of the eight constants is not something this product
 * produced.
 */
export function readResultMarker(search: string): ResultMarker | null {
  const marker = new URLSearchParams(search).get('identity')
  return RESULT_MARKERS.find((value) => value === marker) ?? null
}

/** The user-facing sentence for one result marker. */
export function resultMessage(marker: ResultMarker | null): string {
  switch (marker) {
    case 'success':
      return 'Your age and gender are verified. Restricted introductions may now be visible to you.'
    case 'duplicate':
      return 'This NAVER account already verifies another profile. Use a different NAVER account.'
    case 'profile_incomplete':
      return 'NAVER did not return the age and gender this needs. Check your NAVER profile and try again.'
    case 'age_not_supported':
      return 'The age NAVER reported is outside the range this product supports.'
    case 'cancelled':
      return 'Verification was cancelled. Nothing changed.'
    case 'expired':
      return 'The verification session expired before it finished. Start again when you are ready.'
    case 'replayed':
      return 'That verification link was already used. Start a new one.'
    case 'failed':
      return 'Verification could not be completed. Nothing changed; you can try again.'
    default:
      return 'Verification did not finish. Nothing changed.'
  }
}

/** Whether a marker means the user now holds a live claim. */
export function isSuccess(marker: ResultMarker | null): boolean {
  return marker === 'success'
}

/** The scope a decoded verification belongs to, for cache-key purposes. */
export function verificationScope(scope: SessionScope): SessionScope {
  return { generation: scope.generation, userId: scope.userId, eventId: null }
}
