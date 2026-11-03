import type { SessionScope } from '../api/http'

/** Tokens, provider accounts, handles, drafts and DTO objects are never key parts. */
export interface PrivateQueryScope extends SessionScope {
  readonly userId: string
  readonly policyGeneration: number
}
export type PrivateResource = 'bootstrap' | 'participant' | 'profile' | 'avatar'
  | 'places' | 'place' | 'policy' | 'social-reveal'
export type PrivateQueryKey = readonly ['mcr', 'private', string, string | null, number, number, PrivateResource, ...string[]]
export type PublicQueryKey = readonly ['mcr', 'public', 'auth-capabilities']
const resources: readonly PrivateResource[] = ['bootstrap', 'participant', 'profile', 'avatar', 'places', 'place', 'policy', 'social-reveal']
const uuid = (value: unknown): value is string => typeof value === 'string'
  && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
const targeted: readonly PrivateResource[] = ['profile', 'place', 'social-reveal']
const generation = (value: unknown): value is number => Number.isSafeInteger(value) && (value as number) >= 0

export function privateQueryKey(scope: PrivateQueryScope, resource: PrivateResource, entityIds: readonly string[] = []): PrivateQueryKey {
  if (!uuid(scope.userId) || (scope.eventId !== null && !uuid(scope.eventId))
    || !generation(scope.generation) || !generation(scope.policyGeneration)
    || !resources.includes(resource) || entityIds.length !== (targeted.includes(resource) ? 1 : 0) || !Array.from(entityIds).every(uuid)) {
    throw new TypeError('Invalid private query key.')
  }
  return Object.freeze(['mcr', 'private', scope.userId, scope.eventId, scope.generation, scope.policyGeneration, resource, ...entityIds])
}

/** C03 public capability DTO only. Authenticated public-profile views remain private. */
export function publicQueryKey(): PublicQueryKey {
  return Object.freeze(['mcr', 'public', 'auth-capabilities'])
}
export function isPublicQueryKey(key: readonly unknown[]): key is PublicQueryKey {
  return key.length === 3 && key[0] === 'mcr' && key[1] === 'public' && key[2] === 'auth-capabilities'
}
