import type { PrivacyChange, PrivacyScope } from '../../../shared/query/privacy'

/**
 * WP-08 T-705: avatar cache isolation on account switch.
 *
 * The profile avatar is private account state. It must never survive an account
 * switch, an identity change, or a `409 ACCOUNT_CONTEXT_STALE`. This module is
 * the one place that decides whether a cached avatar entry belongs to the
 * current account version, and it is deliberately *pessimistic*: an entry is
 * only readable while the account version it was written under is still the
 * current one, and every change event clears the whole cache.
 *
 * "Account version" is the canonical privacy scope's `(userId, generation,
 * policyGeneration)`. An event is not part of it: the avatar is profile-owned.
 */
export interface AccountVersion {
  readonly userId: string
  readonly generation: number
  readonly policyGeneration: number
}

/** The account version of a scope, or `null` when no account is authenticated. */
export function accountVersion(scope: PrivacyScope): AccountVersion | null {
  if (typeof scope.userId !== 'string' || scope.userId.length === 0) return null
  return Object.freeze({ userId: scope.userId, generation: scope.generation, policyGeneration: scope.policyGeneration })
}

/** Two versions are the same account version only if every component matches. */
export function sameAccountVersion(left: AccountVersion | null, right: AccountVersion | null): boolean {
  return !!left && !!right && left.userId === right.userId && left.generation === right.generation
    && left.policyGeneration === right.policyGeneration
}

interface Entry<T> {
  readonly version: AccountVersion
  readonly value: T
}

export interface AccountScopedAvatarCache<T> {
  /** The cached value for this version, or `undefined` when it is not this account's. */
  read(version: AccountVersion | null): T | undefined
  /** Store a value under this version. A `null` version stores nothing. */
  write(version: AccountVersion | null, value: T): void
  /** Drop everything. Called on every account change. */
  clear(): void
  /** Subscribe to cache clears (account switch, stale context, revocation). */
  onClear(listener: () => void): () => void
  /** The version currently stored, if any. Diagnostic only. */
  version(): AccountVersion | null
  readonly size: number
}

/**
 * A cache whose every entry is stamped with the account version it was written
 * under. A read under a different version returns `undefined` *and* evicts the
 * stale entry, so the previous account's avatar cannot be rendered even by a
 * caller that ignored the account switch.
 */
export function createAccountScopedAvatarCache<T>(): AccountScopedAvatarCache<T> {
  let entry: Entry<T> | null = null
  const listeners = new Set<() => void>()
  let size = 0
  function clear() {
    if (entry === null && size === 0) return
    entry = null; size = 0
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* One faulty consumer cannot keep another account's avatar. */ } }
  }
  return {
    read(version) {
      if (entry === null || version === null) return undefined
      if (!sameAccountVersion(entry.version, version)) {
        // A read under another account version is a switch that was not
        // announced. Evict rather than serve the previous account's avatar.
        clear()
        return undefined
      }
      return entry.value
    },
    write(version, value) {
      if (version === null) { clear(); return }
      if (entry !== null && !sameAccountVersion(entry.version, version)) clear()
      entry = { version, value }; size = 1
    },
    clear,
    onClear(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    version: () => entry?.version ?? null,
    get size() { return size },
  }
}

/**
 * Wire a cache to the canonical privacy owner.
 *
 * The privacy owner already aborts private work and scrubs private query data
 * on an identity change. This adds the avatar cache to that same boundary: every
 * `PrivacyChange` — account switch, `ACCOUNT_CONTEXT_STALE`, policy change,
 * disposal — clears it, and the cleanup registration covers the synchronous
 * path even if a listener throws.
 */
export function bindAvatarCacheToPrivacy<T>(options: {
  readonly cache: AccountScopedAvatarCache<T>
  readonly privacy: {
    subscribe(listener: (change: PrivacyChange) => void): () => void
    registerCleanup(cleanup: (change: Omit<PrivacyChange, 'cleanupFailures'>) => void): () => void
  }
}): () => void {
  const { cache, privacy } = options
  const unsubscribe = privacy.subscribe(() => { cache.clear() })
  const unregister = privacy.registerCleanup(() => { cache.clear() })
  return () => { unsubscribe(); unregister() }
}

/**
 * The account-context-stale reason the server returns when the session's
 * `account_context_version` no longer matches (domain-contract.md §1.2).
 *
 * The foundation does not emit this reason anywhere yet (WP-02 recorded the
 * same gap: the string appears only in the integration documents). The client
 * therefore *recognizes* it and clears the avatar cache, and never invents the
 * header or fabricates the status. When the backend starts emitting it, this
 * branch is already correct.
 */
export const ACCOUNT_CONTEXT_STALE = 'ACCOUNT_CONTEXT_STALE' as const

/** Whether a server refusal is an account-context-stale that must clear the cache. */
export function isAccountContextStale(status: number | undefined, reason: string | undefined): boolean {
  return status === 409 && reason === ACCOUNT_CONTEXT_STALE
}
