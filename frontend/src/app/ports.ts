import type { SessionController } from '../features/auth/model/session'
import type { ShellCapabilities } from './capabilities'
import { createMemorySession, type SessionPort, type SessionScope } from '../shared/api/http'

export type SessionReadiness = 'unknown' | 'checking' | 'ready' | 'anonymous' | 'unavailable' | 'error'
export interface ReadinessPort {
  read(): SessionReadiness
  subscribe(listener: () => void): () => void
  retry?(): void
}
export interface ActiveScope extends SessionScope { readonly userId: string; readonly eventId: string }
/** A shell view projection AFTER C03 validation; not a competing wire DTO/decoder.
 * FE09's adapter checks the full Bootstrap and maps only these non-secret fields.
 */
export interface ShellBootstrap {
  readonly contractVersion: '0.1.0'
  readonly event: { readonly eventId: string; readonly title: string
    readonly phase: 'DRAFT' | 'OPEN' | 'LIVE' | 'READ_ONLY' | 'ARCHIVED' }
  readonly participant: { readonly userId: string; readonly eventId: string }
  readonly worldAvailable: boolean
  readonly capabilities: ShellCapabilities
}
export type ShellFailureReason = 'unauthenticated' | 'denied' | 'not-found' | 'gone' | 'rate-limited' | 'unavailable'
/** Safe adapter presentation category; never pass a server error body to the shell. */
export class ShellLoadError extends Error {
  constructor(readonly reason: ShellFailureReason) { super(reason); this.name = 'ShellLoadError' }
}
export interface WorldLease {
  /** Synchronously release game/connection authority and all listeners. Idempotent. */
  dispose(): void
  setInputBlocked(blocked: boolean): void
  setVisible(visible: boolean): void
}
export interface WorldLifecyclePort {
  /** Own BOTH future game/socket resources, at most one of each. Async work must obey signal.
   * No credentials in the DOM/context. The adapter obtains tickets from the canonical session.
   * A throwing mount must release partial resources itself; dispose must never throw.
   */
  mount(host: HTMLElement, context: { scope: ActiveScope; bootstrap: ShellBootstrap; signal: AbortSignal }): WorldLease
}
export interface ShellPorts {
  readonly auth?: SessionController
  readonly session: SessionPort
  readonly readiness: ReadinessPort
  readonly bootstrap: { load(scope: ActiveScope, signal: AbortSignal): Promise<ShellBootstrap> }
  readonly world?: WorldLifecyclePort
  /** Requires a current server authorization decision. C03 participant data grants no admin role. */
  readonly admin: { authorize(scope: ActiveScope, signal: AbortSignal): Promise<boolean> }
}
export function createProductionPorts(auth?: SessionController): ShellPorts {
  if (auth) return { auth, session: auth.session, readiness: auth.readiness,
    bootstrap: { load: auth.loadBootstrap }, admin: { authorize: () => Promise.resolve(false) } }
  return {
    session: createMemorySession(),
    readiness: { read: () => 'unavailable', subscribe: () => () => undefined },
    bootstrap: { load: () => Promise.reject(new Error('Bootstrap adapter unavailable')) },
    admin: { authorize: () => Promise.resolve(false) },
  }
}
