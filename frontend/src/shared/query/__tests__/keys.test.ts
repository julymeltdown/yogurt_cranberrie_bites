import { describe, expect, it } from 'vitest'
import { isPublicQueryKey, privateQueryKey, publicQueryKey, type PrivateQueryScope, type PrivateResource } from '../keys'

const userId = '00000000-0000-4000-8000-000000000101'
const eventId = '00000000-0000-4000-8000-000000000201'
const sparseIds: string[] = []
sparseIds.length = 1
const scope: PrivateQueryScope = { userId, eventId, generation: 1, policyGeneration: 0 }

describe('canonical private keys and explicit public classification', () => {
  it('binds canonical identity, event, session generation and policy generation', () => {
    expect(privateQueryKey(scope, 'participant')).toEqual(['mcr', 'private', userId, eventId, 1, 0, 'participant'])
    const original = privateQueryKey(scope, 'participant')
    for (const delta of [{ userId: '00000000-0000-4000-8000-000000000102' },
      { eventId: '00000000-0000-4000-8000-000000000202' }, { generation: 2 }, { policyGeneration: 1 }]) {
      expect(privateQueryKey({ ...scope, ...delta }, 'participant')).not.toEqual(original)
    }
    expect(Object.isFrozen(original)).toBe(true)
    expect(privateQueryKey({ ...scope, eventId: null }, 'policy')[3]).toBeNull()
  })

  it('allows only UUID target IDs and ignores non-key source properties', () => {
    const id = '00000000-0000-4000-8000-000000000301'
    const key = privateQueryKey({ ...scope, accessToken: 'secret-token', handle: '@private' } as PrivateQueryScope, 'profile', [id])
    expect(key.at(-1)).toBe(id)
    expect(JSON.stringify(key)).not.toMatch(/secret-token|@private|accessToken|handle/)
  })

  it.each([
    { userId: null }, { userId: 'provider:123' }, { eventId: 'MCR' }, { generation: -1 },
    { generation: 1.2 }, { policyGeneration: NaN }, { policyGeneration: -1 },
  ])('rejects noncanonical scope %#', patch => {
    expect(() => privateQueryKey({ ...scope, ...patch } as PrivateQueryScope, 'participant')).toThrow('Invalid private query key.')
  })

  it.each([{ ids: ['@handle'] }, { ids: ['Bearer private'] }, { ids: ['raw-payload'] }, { ids: [userId, eventId, userId] }, { ids: sparseIds }])('rejects unapproved key data %#', ({ ids }) => {
    expect(() => privateQueryKey(scope, 'social-reveal', ids)).toThrow('Invalid private query key.')
  })

  it('requires exactly one target for profile/place/reveal and none for collection/self reads', () => {
    for (const resource of ['profile', 'place', 'social-reveal'] as const) {
      expect(() => privateQueryKey(scope, resource)).toThrow('Invalid private query key.')
      expect(privateQueryKey(scope, resource, [userId]).at(-1)).toBe(userId)
    }
    expect(() => privateQueryKey(scope, 'participant', [userId])).toThrow('Invalid private query key.')
  })

  it('rejects arbitrary resource names without echoing them', () => {
    expect(() => privateQueryKey(scope, 'secret-payload' as PrivateResource)).toThrow('Invalid private query key.')
  })

  it('classifies only the exact approved capabilities key as public', () => {
    expect(isPublicQueryKey(publicQueryKey())).toBe(true)
    for (const key of [[], ['public'], ['mcr', 'public', 'profile'], [...publicQueryKey(), userId], privateQueryKey(scope, 'profile', [userId])]) {
      expect(isPublicQueryKey(key)).toBe(false)
    }
  })
})
