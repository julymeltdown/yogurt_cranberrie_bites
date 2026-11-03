import { canonicalId, safeReturnTarget } from '../routes'
export type SocialRoute = { readonly kind: 'feed' | 'compose' | 'people' | 'intro' | 'age' | 'inbox' | 'requests' }
  | { readonly kind: 'thread' | 'edit' | 'notice' | 'profile'; readonly id: string }
const fixed: Readonly<Record<string, SocialRoute['kind']>> = { '/feed': 'feed', '/feed/new': 'compose', '/meet/people': 'people', '/meet/intro/edit': 'intro', '/me/age': 'age', '/notifications': 'inbox', '/me/requests': 'requests' }
export function socialRoute(path: string): SocialRoute | null {
  if (safeReturnTarget(path, '') !== path || !path || path.includes('?')) return null
  const kind = fixed[path]
  if (kind) return { kind } as SocialRoute
  const match = /^\/(feed|notices|people)\/([^/]+)(\/edit)?$/.exec(path)
  if (!match || !canonicalId(match[2]!) || match[3] && match[1] !== 'feed') return null
  return { kind: match[1] === 'feed' ? match[3] ? 'edit' : 'thread' : match[1] === 'people' ? 'profile' : 'notice', id: match[2]! }
}
