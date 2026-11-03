import { queryOptions, type DehydrateOptions, type Query, type QueryClient } from '@tanstack/react-query'
import type { SessionPort, SessionScope } from '../api/http'
import { isPublicQueryKey, privateQueryKey, type PrivateQueryKey, type PrivateResource } from './keys'

export type PrivacyGate = 'open' | 'suspended' | 'event-closed' | 'disposed'
export interface PrivacyScope extends SessionScope {
  readonly policyGeneration: number
  readonly gate: PrivacyGate
}
export type PolicyChange = 'block' | 'consent-revoked' | 'consent-expired' | 'handle-replaced'
  | 'suspended' | 'event-closed' | 'revalidated'
export type PrivacyChangeReason = PolicyChange | 'identity-changed' | 'disposed'
export interface PrivacyChange {
  readonly reason: PrivacyChangeReason
  readonly previous: PrivacyScope
  readonly current: PrivacyScope
  readonly cleanupFailures: number
}
export interface PrivateReadContext {
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
}
export interface PrivateRead<T> {
  readonly resource: PrivateResource
  readonly entityIds?: readonly string[]
  readonly read: (context: PrivateReadContext) => T | Promise<T>
}
export class PrivacyError extends Error {
  constructor(readonly kind: 'scope-expired' | 'unavailable' | 'cancelled' | 'disposed') {
    super(`Private work rejected: ${kind}.`)
    this.name = 'PrivacyError'
  }
}

/** Pass explicitly to any future dehydrate/persistence boundary; never use the default. */
export const publicOnlyDehydrateOptions: DehydrateOptions = Object.freeze({
  shouldDehydrateQuery: (query: Query) => isPublicQueryKey(query.queryKey) && query.state.status === 'success',
  shouldDehydrateMutation: () => false,
})
const identity = ({ userId, eventId, generation }: SessionScope): SessionScope => Object.freeze({ userId, eventId, generation })

/** One coordinator per shell QueryClient/SessionPort; neither cache nor credentials are duplicated. */
export function createQueryPrivacy({ client, session }: { client: QueryClient; session: SessionPort }) {
  let observed = identity(session.read())
  let sessionSignal = session.read().signal
  let policyGeneration = 0
  let gate: PrivacyGate = 'open'
  let lifetime = new AbortController()
  const listeners = new Set<(change: PrivacyChange) => void>()
  const cleanups = new Set<(change: Omit<PrivacyChange, 'cleanupFailures'>) => void>()
  const capture = (): PrivacyScope => Object.freeze({ ...observed, policyGeneration, gate })
  const matches = (scope: PrivacyScope) => gate !== 'disposed' && session.isCurrent(scope)
    && scope.policyGeneration === policyGeneration && scope.gate === gate
  const isCurrent = (scope: PrivacyScope) => matches(scope) && gate === 'open' && scope.userId !== null
  const assertCurrent = (scope: PrivacyScope) => {
    if (gate === 'disposed') throw new PrivacyError('disposed')
    if (!matches(scope)) throw new PrivacyError('scope-expired')
    if (gate !== 'open' || scope.userId === null) throw new PrivacyError('unavailable')
  }

  function advance(reason: PrivacyChangeReason, nextGate: PrivacyGate, nextIdentity = observed) {
    const previous = capture()
    observed = nextIdentity
    policyGeneration++
    gate = nextGate
    const oldLifetime = lifetime
    lifetime = new AbortController()
    const change = Object.freeze({ reason, previous, current: capture() })
    // Invalidate leases first; cancellation/refetch/cleanup callbacks cannot reuse the old scope.
    oldLifetime.abort()
    const filters = { predicate: (query: { queryKey: readonly unknown[] }) => !isPublicQueryKey(query.queryKey) }
    let cleanupFailures = 0
    try { void client.cancelQueries(filters, { revert: false }) } catch { cleanupFailures++ }
    for (const cleanup of Array.from(cleanups)) {
      try { cleanup(change) } catch { cleanupFailures++ }
    }
    // removeQueries alone leaves active observers holding their last data. Scrub before eviction.
    for (const query of client.getQueryCache().findAll(filters)) {
      const observers = Array.from(query.observers)
      try {
        query.setState({ data: undefined, error: null, dataUpdatedAt: 0, errorUpdatedAt: 0,
          dataUpdateCount: 0, errorUpdateCount: 0, fetchFailureCount: 0, fetchFailureReason: null,
          fetchMeta: null, fetchStatus: 'idle', status: 'pending', isInvalidated: true })
      } catch { cleanupFailures++ }
      // A throwing observer must not leave a later observer displaying its former result.
      for (const observer of observers) {
        try { observer.updateResult() } catch { cleanupFailures++ }
      }
      try { client.getQueryCache().remove(query) } catch { cleanupFailures++ }
    }
    // Mutation cancellation is feature-owned: registered cleanup resets observers/aborts work.
    // Removing a Mutation does not cancel its server effect or its callbacks.
    for (const mutation of client.getMutationCache().getAll()) {
      try { client.getMutationCache().remove(mutation) } catch { cleanupFailures++ }
    }
    const event = Object.freeze({ ...change, cleanupFailures })
    for (const listener of Array.from(listeners)) {
      try { listener(event) } catch { /* One faulty consumer cannot prevent other owners from clearing private state. */ }
    }
  }

  const changedIdentity = () => {
    sessionSignal.removeEventListener('abort', changedIdentity)
    sessionSignal = session.read().signal
    sessionSignal.addEventListener('abort', changedIdentity, { once: true })
    advance('identity-changed', 'open', identity(session.read()))
  }
  sessionSignal.addEventListener('abort', changedIdentity, { once: true })

  async function run<T>(scope: PrivacyScope, work: (context: PrivateReadContext) => T | Promise<T>, signal?: AbortSignal): Promise<T> {
    assertCurrent(scope)
    const controller = new AbortController()
    const signals = [lifetime.signal, sessionSignal, ...(signal ? [signal] : [])]
    const abort = () => controller.abort()
    for (const source of signals) source.addEventListener('abort', abort, { once: true })
    if (signals.some(source => source.aborted)) abort()
    let rejectAborted: () => void = () => undefined
    const stopped = new Promise<never>((_, reject) => {
      rejectAborted = () => {
        try { assertCurrent(scope); reject(new PrivacyError('cancelled')) } catch (error) { reject(error) }
      }
      controller.signal.addEventListener('abort', rejectAborted, { once: true })
      if (controller.signal.aborted) rejectAborted()
    })
    try {
      const pending = Promise.resolve().then(() => {
        assertCurrent(scope)
        if (controller.signal.aborted) throw new PrivacyError('cancelled')
        return work({ scope, signal: controller.signal })
      })
      const result = await Promise.race([pending, stopped])
      assertCurrent(scope)
      if (controller.signal.aborted) throw new PrivacyError('cancelled')
      return result
    } catch (error) {
      assertCurrent(scope)
      if (controller.signal.aborted) throw new PrivacyError('cancelled')
      throw error
    } finally {
      controller.signal.removeEventListener('abort', rejectAborted)
      for (const source of signals) source.removeEventListener('abort', abort)
    }
  }

  function options<T>(request: PrivateRead<T>) {
    const scope = capture()
    if (scope.userId === null) throw new PrivacyError('unavailable')
    assertCurrent(scope)
    const queryKey = privateQueryKey({ ...scope, userId: scope.userId }, request.resource, request.entityIds)
    const reveal = request.resource === 'social-reveal'
    return queryOptions<T, Error, T, PrivateQueryKey>({
      queryKey, queryFn: context => run(scope, request.read, context.signal),
      // Query commits in a later microtask than queryFn settlement. Fence that final boundary too.
      structuralSharing: (_previous, next) => { assertCurrent(scope); return next },
      // Preserve FE01's retry policy: Query must not add an independent retry layer.
      retry: false, retryOnMount: false, staleTime: 0, gcTime: 0, notifyOnChangeProps: 'all',
      enabled: !reveal, refetchOnMount: !reveal, refetchOnWindowFocus: false,
      refetchOnReconnect: false, refetchInterval: false,
      initialData: () => undefined, placeholderData: () => undefined,
      // Override a client-wide persister without sending private results to it.
      persister: (queryFn, context) => queryFn(context),
    })
  }

  return {
    capture, isCurrent, options, run,
    changePolicy(expected: PrivacyScope, reason: PolicyChange): boolean {
      if (gate === 'disposed' || !session.isCurrent(expected)) return false
      // Revocations from the same stream/session must not be lost after an earlier policy bump.
      // Only reopening access requires the full policy-generation compare-and-set.
      if (reason === 'revalidated' && !matches(expected)) return false
      const nextGate = reason === 'revalidated' ? 'open'
        : reason === 'suspended' || reason === 'event-closed' ? reason : gate
      advance(reason, nextGate)
      return true
    },
    /** Synchronous: clear drafts, abort/reset mutations, and close private subscriptions. */
    registerCleanup(cleanup: (change: Omit<PrivacyChange, 'cleanupFailures'>) => void) {
      if (gate === 'disposed') throw new PrivacyError('disposed')
      cleanups.add(cleanup)
      return () => { cleanups.delete(cleanup) }
    },
    /** Rebind query options to current scope; never keep prior data as placeholder. */
    subscribe(listener: (change: PrivacyChange) => void) {
      if (gate === 'disposed') throw new PrivacyError('disposed')
      listeners.add(listener)
      return () => { listeners.delete(listener) }
    },
    dispose() {
      if (gate === 'disposed') return
      sessionSignal.removeEventListener('abort', changedIdentity)
      advance('disposed', 'disposed')
      cleanups.clear()
      listeners.clear()
    },
  }
}
