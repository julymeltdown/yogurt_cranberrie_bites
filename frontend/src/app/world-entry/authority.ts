import type { WorldReleaseRequest } from '../../world/composition'
import type { SessionScope } from '../../shared/api/http'
import { projectCapabilities, validCapabilities } from '../capabilities'
import type { SessionController } from '../../features/auth/model/session'
import { canonicalJson, decodeBootstrap } from '../../features/auth/model/bootstrap'
import { canonicalUuid, requireValue } from '../../features/auth/model/contract'

/** The committed composition public boundary; no parallel checkout dependency. */
export type WorldEntryRequest = WorldReleaseRequest
export type EntryAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
export const sameScope = (a: SessionScope, b: SessionScope) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation
function frozenDocument(value: unknown, depth = 0, budget = { remaining: 200000 }): boolean {
  if (--budget.remaining < 0 || depth > 40) return false
  if (value === null || typeof value !== 'object') return true
  if (!Object.isFrozen(value) || Object.getOwnPropertySymbols(value).length) return false
  const prototype: unknown = Object.getPrototypeOf(value)
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null) return false
  return Object.values(Object.getOwnPropertyDescriptors(value)).every(descriptor => Object.hasOwn(descriptor, 'value') && frozenDocument(descriptor.value, depth + 1, budget))
}
export function captureEntry(input: WorldEntryRequest): WorldEntryRequest {
  const bootstrap = input.bootstrap, document = bootstrap.document
  requireValue(canonicalUuid(input.scope.userId) && canonicalUuid(input.scope.eventId) && Number.isSafeInteger(input.scope.generation) && input.scope.generation >= 0
    && sameScope(input.scope, input.privacyScope) && input.privacyScope.gate === 'open' && Number.isSafeInteger(input.privacyScope.policyGeneration) && input.privacyScope.policyGeneration >= 0
    && document && frozenDocument(document))
  // Only primitive scope values and the already immutable document cross async validation.
  return Object.freeze({ scope: Object.freeze({ userId: input.scope.userId, eventId: input.scope.eventId, generation: input.scope.generation }),
    privacyScope: Object.freeze({ userId: input.privacyScope.userId, eventId: input.privacyScope.eventId, generation: input.privacyScope.generation, policyGeneration: input.privacyScope.policyGeneration, gate: input.privacyScope.gate }),
    bootstrap: Object.freeze({ ...bootstrap, document, event: Object.freeze({ ...input.bootstrap.event }), participant: Object.freeze({ ...input.bootstrap.participant }),
      capabilities: projectCapabilities(input.bootstrap.capabilities) }), signal: input.signal })
}
export function assertEntry(auth: EntryAuth, input: WorldEntryRequest) {
  requireValue(!input.signal.aborted && sameScope(input.scope, input.privacyScope) && auth.session.isCurrent(input.scope) && auth.privacy.isCurrent(input.privacyScope))
}
export async function validateEntry(auth: EntryAuth, input: WorldEntryRequest) {
  assertEntry(auth, input)
  const projected = await decodeBootstrap(input.bootstrap.document, input.scope)
  assertEntry(auth, input)
  requireValue(canonicalJson(projected) === canonicalJson(input.bootstrap) && validCapabilities(projected.capabilities)
    && projected.worldAvailable && projected.capabilities.world.available && !['DRAFT', 'ARCHIVED'].includes(projected.event.phase))
  return projected.document!
}
