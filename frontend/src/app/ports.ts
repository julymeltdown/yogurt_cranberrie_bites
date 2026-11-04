import type { SessionController } from '../features/auth/model/session'
import type { AccountAssetsPort } from './account-entry/assets'
import type { ShellCapabilities } from './capabilities'
import { createProductionWorld, type ProductionWorldOptions } from './world-entry/production'
import type { ReplicationReader } from '../features/world-rooms'
import type { WorldEntryRequest } from './world-entry/authority'
import type { PrivacyScope } from '../shared/query/privacy'
import { createMemorySession, type SessionPort, type SessionScope } from '../shared/api/http'
import { createRootCharacterRegistry, type RootCharacterRegistry } from './character-entry/runtime'
import type { ConnectionStatusController } from '../features/connection-status/model'
import { createDefaultAdmin, type AdminComposition } from './operations-composition/owner'
import { createProductionCommunityClock } from './community-clock-runtime'

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
  /** Validated immutable C03 response, usable only with this bootstrap's privacy lease. */
  readonly document?: Readonly<Record<string, unknown>>
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
  mount(host: HTMLElement, context: { scope: ActiveScope; bootstrap: ShellBootstrap; signal: AbortSignal; privacyScope?: PrivacyScope | null }): WorldLease
}
export interface WorldAccessBinding { readonly release: WorldEntryRequest; readonly replication: ReplicationReader;
  readonly lifecycle?: import('./errors/fe41-lifecycle/status').WorldLifecycleReader;
  readonly socialPolicy?: import('../world/composition/policy').WorldSocialPolicyPort;
  readonly presentation?: import('../world/composition').WorldPresentationPort; readonly reconnect?: () => Promise<boolean> }
export interface WorldAccessBindingPort { read(): WorldAccessBinding | null; subscribe(listener: () => void): () => void }
export interface ShellPorts {
  /** Absent until a supported clock environment is qualified; never inferred from Bootstrap. */
  readonly communityClock?: import('./community-adoption/bootstrap').CommunityClockProfile
  readonly connectionStatus?: { readonly controller: ConnectionStatusController; readonly reload: () => void }
  /** Explicit public event selection. It does not grant identity or world access. */
  readonly eventId?: string | null
  /** One fixed-scope selected-character owner, borrowed by shell views and World. */
  readonly characters?: RootCharacterRegistry
  readonly social?: import('./world-panels/social').SocialProductionPorts
  /** Explicit reviewed local avatar inputs; absent means previews are unavailable. */
  readonly accountAssets?: AccountAssetsPort
  readonly auth?: SessionController
  readonly session: SessionPort
  readonly readiness: ReadinessPort
  readonly bootstrap: { load(scope: ActiveScope, signal: AbortSignal): Promise<ShellBootstrap> }
  readonly world?: WorldLifecyclePort
  readonly worldAccess?: WorldAccessBindingPort
  /** Application teardown; does not replace the canonical Auth owner. */
  readonly dispose?: () => void
  /** Requires a current server authorization decision. C03 participant data grants no admin role. */
  readonly admin: { authorize(scope: ActiveScope, signal: AbortSignal): Promise<boolean>
    /** Retained canonical full-scope owner; a boolean alone cannot mount tools. */
    open?(scope: ActiveScope): AdminComposition | null }
}
export function createProductionPorts(auth?: SessionController, worldOptions: Omit<ProductionWorldOptions, 'auth' | 'characters'> = {}, social?: ShellPorts['social'], publicEventId: string | null = null): ShellPorts {
  if (auth) {
    const characters = createRootCharacterRegistry(auth, worldOptions.origin)
    let production: ReturnType<typeof createProductionWorld>
    try { production = createProductionWorld({ ...worldOptions, auth, characters }) }
    catch (error) { characters.dispose(); throw error }
    const admin = createDefaultAdmin(auth)
    const communityClock = createProductionCommunityClock(publicEventId)
    return { eventId: publicEventId, ...(social ? { social } : {}), ...(communityClock.profile ? { communityClock: communityClock.profile } : {}), characters, auth, session: auth.session, readiness: auth.readiness, world: production.world, worldAccess: production.access,
      dispose() { try { communityClock.dispose() } finally { try { admin.dispose() } finally { try { production.dispose() } finally { characters.dispose() } } } },
      bootstrap: { load: auth.loadBootstrap }, admin }
  }
  return {
    eventId: publicEventId,
    session: createMemorySession(),
    readiness: { read: () => 'unavailable', subscribe: () => () => undefined },
    bootstrap: { load: () => Promise.reject(new Error('Bootstrap adapter unavailable')) },
    admin: { authorize: () => Promise.resolve(false) },
  }
}
