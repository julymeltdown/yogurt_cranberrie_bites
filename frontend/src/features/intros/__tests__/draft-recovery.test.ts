import { describe, expect, it } from 'vitest'
import {
  clearDraft,
  clearOtherAccounts,
  draftKey,
  memoryStorage,
  readDraft,
  saveDraft,
  type StoredDraft,
} from '../draft-recovery'
import { initial } from '../composer-reducer'

const accountA = '00000000-0000-4000-8000-00000000000a'
const accountB = '00000000-0000-4000-8000-00000000000b'

function stored(body: string): StoredDraft {
  const state = initial()
  return {
    draft: { ...state.draft, body },
    introId: null,
    revision: 0,
    savedAt: 1_700_000_000_000,
  }
}

describe('draft recovery', () => {
  it('keys a draft by account', () => {
    expect(draftKey(accountA)).toBe(`mcr.intro-draft.v1:${accountA}`)
    expect(draftKey(accountA)).not.toBe(draftKey(accountB))
    // A malformed account id is refused rather than producing a shared key.
    expect(() => draftKey('not-an-account')).toThrow()
  })

  it('round-trips a draft for its own account', () => {
    const storage = memoryStorage()
    saveDraft(storage, accountA, stored('my draft'))
    expect(readDraft(storage, accountA)?.draft.body).toBe('my draft')
  })

  // The acceptance case: a draft from account A is invisible to account B.
  it('keeps one account draft invisible to another', () => {
    const storage = memoryStorage()
    saveDraft(storage, accountA, stored('A private draft'))
    expect(readDraft(storage, accountB)).toBeNull()
    // B writing its own draft does not see or overwrite A's.
    saveDraft(storage, accountB, stored('B draft'))
    expect(readDraft(storage, accountA)?.draft.body).toBe('A private draft')
    expect(readDraft(storage, accountB)?.draft.body).toBe('B draft')
  })

  // The acceptance case: a recovered draft does not auto-publish.
  it('recovers fields without publishing anything', () => {
    const storage = memoryStorage()
    saveDraft(storage, accountA, stored('recovered'))
    const recovered = readDraft(storage, accountA)!
    // Recovery returns the stored fields and nothing else: no publish marker, no
    // request, no state transition.
    expect(recovered.draft.body).toBe('recovered')
    expect(Object.keys(recovered)).toEqual(['draft', 'introId', 'revision', 'savedAt'])
    expect(recovered.introId).toBeNull()
    expect(recovered.revision).toBe(0)
  })

  it('clears one account draft', () => {
    const storage = memoryStorage()
    saveDraft(storage, accountA, stored('a'))
    saveDraft(storage, accountB, stored('b'))
    clearDraft(storage, accountA)
    expect(readDraft(storage, accountA)).toBeNull()
    expect(readDraft(storage, accountB)?.draft.body).toBe('b')
  })

  // Cleared on account switch: the previous account's draft cannot be read by
  // the next person on a shared device.
  it('clears every other account on a switch', () => {
    const storage = memoryStorage()
    saveDraft(storage, accountA, stored('a'))
    saveDraft(storage, accountB, stored('b'))
    const removed = clearOtherAccounts(storage, accountB)
    expect(removed).toBe(1)
    expect(readDraft(storage, accountA)).toBeNull()
    expect(readDraft(storage, accountB)?.draft.body).toBe('b')
    // Unrelated keys are untouched.
    storage.setItem('something-else', 'x')
    clearOtherAccounts(storage, accountB)
    expect(storage.getItem('something-else')).toBe('x')
  })

  it('discards a malformed entry rather than half-restoring it', () => {
    const storage = memoryStorage()
    storage.setItem(draftKey(accountA), 'not json')
    expect(readDraft(storage, accountA)).toBeNull()
    storage.setItem(draftKey(accountA), JSON.stringify({ draft: null }))
    expect(readDraft(storage, accountA)).toBeNull()
  })
})
