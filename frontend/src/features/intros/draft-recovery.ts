import type { ComposerDraft } from './composer-reducer'

/**
 * WP-05 T-414: draft recovery.
 *
 * An in-progress draft is persisted locally so a reload or a failed publish does
 * not lose the author's work. Three rules, each with a test:
 *
 * 1. **Scoped by account.** The storage key includes the account id, so a draft
 *    written by account A is invisible to account B. There is no "last draft"
 *    slot that two accounts could share.
 * 2. **Recovered, never auto-published.** Recovery restores the fields; it never
 *    calls publish. A recovered draft is still a draft.
 * 3. **Cleared on account switch.** Switching accounts removes the previous
 *    account's entry, so a shared device does not leave one person's draft where
 *    the next person can read it.
 *
 * The store is a narrow port so the tests use an in-memory implementation and
 * production uses `localStorage`. Nothing here touches the network.
 */
export interface DraftStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
  key(index: number): string | null
  readonly length: number
}

const PREFIX = 'mcr.intro-draft.v1'

/** The storage key for one account. */
export function draftKey(accountId: string): string {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(accountId)) {
    throw new Error('Invalid account id for draft recovery')
  }
  return `${PREFIX}:${accountId}`
}

export interface StoredDraft {
  readonly draft: ComposerDraft
  readonly introId: string | null
  readonly revision: number
  readonly savedAt: number
}

/** Persist a draft for one account. */
export function saveDraft(
  storage: DraftStorage,
  accountId: string,
  value: StoredDraft,
): void {
  storage.setItem(draftKey(accountId), JSON.stringify(value))
}

/** Read one account's draft, or `null` when there is none. */
export function readDraft(storage: DraftStorage, accountId: string): StoredDraft | null {
  const raw = storage.getItem(draftKey(accountId))
  if (raw === null) return null
  try {
    const parsed = JSON.parse(raw) as StoredDraft
    // A malformed entry is discarded rather than half-restored. `typeof null` is
    // `"object"`, so every level is checked for null explicitly.
    if (typeof parsed !== 'object' || parsed === null) return null
    if (typeof parsed.draft !== 'object' || parsed.draft === null) return null
    if (typeof parsed.draft.body !== 'string') return null
    if (typeof parsed.savedAt !== 'number') return null
    return parsed
  } catch {
    return null
  }
}

/** Remove one account's draft. */
export function clearDraft(storage: DraftStorage, accountId: string): void {
  storage.removeItem(draftKey(accountId))
}

/**
 * Remove every draft that does not belong to `accountId`.
 *
 * Called when the account context changes, so a previous account's draft cannot
 * be read by the next person on a shared device.
 */
export function clearOtherAccounts(storage: DraftStorage, accountId: string): number {
  const keep = draftKey(accountId)
  const doomed: string[] = []
  for (let index = 0; index < storage.length; index += 1) {
    const key = storage.key(index)
    if (key !== null && key.startsWith(`${PREFIX}:`) && key !== keep) doomed.push(key)
  }
  for (const key of doomed) storage.removeItem(key)
  return doomed.length
}

/** A minimal in-memory store, for tests and for a browser without storage. */
export function memoryStorage(): DraftStorage {
  const map = new Map<string, string>()
  return {
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => {
      map.set(key, value)
    },
    removeItem: (key) => {
      map.delete(key)
    },
    key: (index) => [...map.keys()][index] ?? null,
    get length() {
      return map.size
    },
  }
}
