import { canonicalId, safeReturnTarget } from '../routes'
export type SocialRoute = { readonly kind: 'feed' | 'compose' | 'people' | 'intro' | 'age' | 'inbox' | 'handles' | 'requests' }
  | { readonly kind: 'thread' | 'edit' | 'notice' | 'profile'; readonly id: string }
// WP-09: the people list and the intro composer moved under Discover. The old
// `/meet/people` and `/meet/intro/edit` paths are redirects now, so the social
// owner does not claim them; the visitor profile's legacy `/people/{id}`
// spelling is still recognised below.
const fixed: Readonly<Record<string, SocialRoute['kind']>> = { '/feed': 'feed', '/feed/new': 'compose',
  '/discover/people': 'people', '/discover/intro': 'intro', '/me/age': 'age', '/notifications': 'inbox',
  '/me/social': 'handles', '/me/requests': 'requests' }
export function socialRoute(path: string): SocialRoute | null {
  // WP-09: the Discover paths are primary-section routes, not concert panel
  // routes, so they are not in the shell's return-target table. They are still
  // social destinations, so they are matched directly before that table.
  if (path === '/discover/people') return { kind: 'people' }
  if (path === '/discover/intro') return { kind: 'intro' }
  const discoverProfile = /^\/discover\/people\/([^/]+)$/.exec(path)
  if (discoverProfile && canonicalId(discoverProfile[1]!)) return { kind: 'profile', id: discoverProfile[1]! }
  if (safeReturnTarget(path, '') !== path || !path || path.includes('?')) return null
  const kind = fixed[path]
  if (kind) return { kind } as SocialRoute
  // The visitor profile lives under Discover; the legacy `/people/{id}` spelling
  // is still recognised so an in-flight component does not lose its route.
  const match = /^\/(feed|notices|people|discover\/people)\/([^/]+)(\/edit)?$/.exec(path)
  if (!match || !canonicalId(match[2]!) || match[3] && match[1] !== 'feed') return null
  const isProfile = match[1] === 'people' || match[1] === 'discover/people'
  return { kind: match[1] === 'feed' ? match[3] ? 'edit' : 'thread' : isProfile ? 'profile' : 'notice', id: match[2]! }
}
