// @vitest-environment jsdom
/**
 * Refactor track 2: the identity-scoped query key contract.
 *
 * The plan requires identity-scoped query keys (`['intro', accountVersion,
 * introId]`). This suite is the audit that keeps every feature honest: it
 * asserts the *shape* of the one key builder the shell uses, and it asserts that
 * no feature has quietly introduced its own key scheme.
 */
import { describe, expect, it } from 'vitest'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { privateQueryKey, publicQueryKey, isPublicQueryKey, type PrivateResource } from '../keys'

const ROOT = resolve(process.cwd(), 'src')
const ACCOUNT = '00000000-0000-4000-8000-000000000003'
const EVENT = '00000000-0000-4000-8000-000000000001'

function sourceFiles(directory: string, found: string[] = []): string[] {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry)
    if (statSync(path).isDirectory()) sourceFiles(path, found)
    else if (/\.(ts|tsx)$/.test(path) && !path.includes('__tests__') && !path.endsWith('.test.ts') && !path.endsWith('.test.tsx')) found.push(path)
  }
  return found
}

const scope = (overrides: Partial<{ generation: number; userId: string; eventId: string | null; policyGeneration: number }> = {}) => ({
  generation: 1,
  userId: ACCOUNT,
  eventId: EVENT,
  policyGeneration: 0,
  ...overrides,
})

describe('identity-scoped query keys', () => {
  it('every private key carries the account, the event, the generation and the policy generation', () => {
    const key = privateQueryKey(scope(), 'profile', [ACCOUNT])
    expect(key).toEqual(['mcr', 'private', ACCOUNT, EVENT, 1, 0, 'profile', ACCOUNT])
    // The identity is at a fixed position, so two accounts can never collide.
    expect(privateQueryKey(scope({ userId: '00000000-0000-4000-8000-000000000004' }), 'profile', [ACCOUNT])[2])
      .not.toBe(key[2])
    // A new generation is a new key even for the same account and event.
    expect(privateQueryKey(scope({ generation: 2 }), 'profile', [ACCOUNT])[4]).toBe(2)
    // A policy change is a new key too.
    expect(privateQueryKey(scope({ policyGeneration: 3 }), 'profile', [ACCOUNT])[5]).toBe(3)
  })

  it('a targeted resource carries exactly one entity id', () => {
    expect(privateQueryKey(scope(), 'profile', [ACCOUNT])).toHaveLength(8)
    expect(() => privateQueryKey(scope(), 'profile')).toThrow(TypeError)
    expect(() => privateQueryKey(scope(), 'profile', [ACCOUNT, ACCOUNT])).toThrow(TypeError)
    // An untargeted resource carries none.
    expect(privateQueryKey(scope(), 'participant')).toHaveLength(7)
    expect(() => privateQueryKey(scope(), 'participant', [ACCOUNT])).toThrow(TypeError)
  })

  it('a key never accepts a non-canonical identity or an unknown resource', () => {
    // `ACCOUNT` is all digits by design, so an upper-case spelling of it is still
    // canonical. A genuinely non-canonical identity is one with a letter in it.
    for (const userId of ['', 'not-a-uuid', 'AAAAAAAA-0000-4000-8000-000000000003']) {
      expect(() => privateQueryKey(scope({ userId }), 'profile', [ACCOUNT])).toThrow(TypeError)
    }
    expect(() => privateQueryKey(scope({ eventId: 'nope' }), 'profile', [ACCOUNT])).toThrow(TypeError)
    expect(() => privateQueryKey(scope({ generation: -1 }), 'profile', [ACCOUNT])).toThrow(TypeError)
    expect(() => privateQueryKey(scope(), 'not-a-resource' as PrivateResource)).toThrow(TypeError)
    expect(() => privateQueryKey(scope(), 'profile', ['not-a-uuid'])).toThrow(TypeError)
  })

  it('the public key is the one C03 capability DTO and nothing else', () => {
    expect(publicQueryKey()).toEqual(['mcr', 'public', 'auth-capabilities'])
    expect(isPublicQueryKey(publicQueryKey())).toBe(true)
    // A private key is never public, so a private result can never be dehydrated.
    expect(isPublicQueryKey(privateQueryKey(scope(), 'participant'))).toBe(false)
    expect(isPublicQueryKey(['mcr', 'public', 'something-else'])).toBe(false)
  })

  it('the keys are frozen, so no consumer can mutate a shared key', () => {
    expect(Object.isFrozen(privateQueryKey(scope(), 'participant'))).toBe(true)
    expect(Object.isFrozen(publicQueryKey())).toBe(true)
  })
})

describe('no feature invents its own key scheme', () => {
  it('every query key in the source comes from the one builder', () => {
    const files = sourceFiles(ROOT)
    expect(files.length).toBeGreaterThan(100)
    const offenders: string[] = []
    for (const path of files) {
      const text = readFileSync(path, 'utf8')
      // A literal key array is how a feature would bypass the builder.
      if (/queryKey\s*:\s*\[/.test(text) && !path.endsWith('shared/query/keys.ts')) offenders.push(path)
      // A hand-written `['mcr', ...]` literal is the same bypass.
      if (/\['mcr',/.test(text) && !path.endsWith('shared/query/keys.ts')) offenders.push(path)
    }
    expect(offenders).toEqual([])
  })
})
