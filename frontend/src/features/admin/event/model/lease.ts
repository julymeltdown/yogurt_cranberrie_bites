import type { PrivacyScope } from '../../../../shared/query/privacy'
import { equal, freeze, revision, utc, uuid } from './contract'
import type { OperationsOptions } from './ports'

export const ROLE_MAX_AGE_MS = 60_000
export const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
export function createOperatorGuard(options: OperationsOptions, scrub: () => void) {
  const { auth, authority, clock } = options, scope = freeze(auth.privacy.capture())
  let lease: ReturnType<NonNullable<typeof authority>['read']> = null
  try { const captured = authority?.read(); lease = captured ? freeze(structuredClone(captured)) : null } catch { /* unqualified producer */ }
  const lifetime = new AbortController(), stops: (() => void)[] = []
  let ended = false, timer: ReturnType<typeof setTimeout> | undefined, lastTime = Number.NEGATIVE_INFINITY
  function now(): number | null {
    try {
      const current = clock?.read()
      if (!current || !sameScope(current.scope, scope) || !Number.isFinite(current.nowMs) || current.nowMs < lastTime) return null
      lastTime = current.nowMs; return current.nowMs
    } catch { return null }
  }
  function valid() {
    try {
      const time = now(), current = authority?.read()
      return !ended && auth.privacy.isCurrent(scope) && uuid.safeParse(scope.userId).success && uuid.safeParse(scope.eventId).success
      && lease !== null && current !== null && equal(lease, current) && sameScope(lease.scope, scope)
      && lease.actorId === scope.userId && lease.eventId === scope.eventId && lease.role === 'OPERATOR'
      && revision.safeParse(lease.revision).success && utc.safeParse(lease.checkedAt).success && utc.safeParse(lease.expiresAt).success
      && time !== null && Date.parse(lease.checkedAt) <= time && time < Date.parse(lease.checkedAt) + ROLE_MAX_AGE_MS
        && Date.parse(lease.checkedAt) < Date.parse(lease.expiresAt) && time < Date.parse(lease.expiresAt)
    } catch { return false }
  }
  function withdraw() {
    if (ended) return
    ended = true; clearTimeout(timer); lifetime.abort(); stops.splice(0).forEach(stop => { try { stop() } catch { /* release remaining registrations */ } }); scrub()
  }
  function changed() {
    if (!valid()) { withdraw(); return }
    clearTimeout(timer)
    const time = now()!
    timer = setTimeout(changed, Math.max(1, Math.min(Date.parse(lease!.expiresAt), Date.parse(lease!.checkedAt) + ROLE_MAX_AGE_MS) - time))
  }
  function own(register: () => () => void) {
    if (ended) return
    try { const stop = register(); if (ended) stop(); else stops.push(stop) } catch { withdraw() }
  }
  if (!valid()) withdraw()
  else {
    try {
      own(() => auth.privacy.registerCleanup(withdraw))
      own(() => authority!.subscribe(changed))
      own(() => clock!.subscribe(changed))
      if (!ended) changed()
    } catch { withdraw() }
  }
  return { scope, signal: lifetime.signal, now, valid, withdraw,
    own,
    assert() { if (!valid()) { withdraw(); throw new Error('Operator lease unavailable') } },
  }
}
