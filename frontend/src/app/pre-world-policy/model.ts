import type { PrivacyScope } from '../../shared/query/privacy'

/** Consumer projection, not a public wire DTO. Root owns the release manifest. */
export interface PolicyApplicationRelease {
  readonly version: 1
  readonly applicationVersion: string
  readonly buildId: string
  readonly sourceRevision: string
}
export interface PolicyAttempt {
  readonly scope: PrivacyScope & { readonly userId: string; readonly eventId: string }
  readonly release: PolicyApplicationRelease
  readonly attempt: number
}
/** Only the strict public-contract adapter may produce this projection. */
export interface PreWorldDecision {
  readonly kind: 'normal' | 'maintenance' | 'version' | 'unavailable'
  readonly eventRevision: number
  readonly policyRevision: number | null
  readonly minimumClientVersion: string | null
  readonly serverTimeMs: number
  readonly validUntilMs: number
}
export interface PreWorldPolicyPort {
  /** Captured once per attempt. Missing/unrecognized identity never grants entry. */
  release(): PolicyApplicationRelease | null
  read(attempt: PolicyAttempt, signal: AbortSignal): Promise<PreWorldDecision>
}
export type PolicyView =
  | { readonly kind: 'idle' | 'checking' | 'normal' | 'consumed' }
  | { readonly kind: 'maintenance' }
  | { readonly kind: 'version'; readonly minimumClientVersion: string }
  | { readonly kind: 'unavailable'; readonly reason: 'unavailable' | 'expired' | 'changed' }

export interface PolicyClock {
  /** Monotonic, never Date.now. Request latency consumes the server's TTL. */
  now(): number
  set(callback: () => void, ms: number): unknown
  clear(timer: unknown): void
}
