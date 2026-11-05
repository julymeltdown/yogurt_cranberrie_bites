import { matchPath } from 'react-router-dom'

export const tabs = [
  { path: '/world', label: 'World' }, { path: '/feed', label: 'Feed' },
  { path: '/meet', label: 'Meet' }, { path: '/gifts', label: 'Gifts' },
  { path: '/wall', label: 'Wall' }, { path: '/me', label: 'Me' },
] as const
export type TabPath = typeof tabs[number]['path']
export interface PanelRoute {
  path: string
  title: string
  tab: TabPath
  parent: string
  links?: readonly { path: string; label: string }[]
}
const links = (...entries: [string, string][]) => entries.map(([path, label]) => ({ path, label }))

/** Plan §6 route composition only. Domain screens remain owned by their feature tasks. */
export const panelRoutes: readonly PanelRoute[] = [
  { path: '/world', title: 'World', tab: '/world', parent: '/world' },
  ...[
    ['map', 'Map'], ['places/:placeId', 'Place'], ['rooms', 'Rooms'],
    ['nearby', 'Nearby in world'], ['theme', 'Choose an era'],
    ['check-in', 'Real-world check-in'], ['help', 'How to play'],
  ].map(([path, title]) => ({ path: `/world/${path}`, title: title!, tab: '/world' as const, parent: path === 'places/:placeId' ? '/world/map' : '/world' })),
  { path: '/feed', title: 'Feed', tab: '/feed', parent: '/world', links: links(['/feed/new', 'New post']) },
  { path: '/feed/new', title: 'New post', tab: '/feed', parent: '/feed' },
  { path: '/feed/:postId', title: 'Thread', tab: '/feed', parent: '/feed' },
  { path: '/feed/:postId/edit', title: 'Edit post', tab: '/feed', parent: '/feed/:postId' },
  { path: '/notices/:noticeId', title: 'Notice', tab: '/feed', parent: '/feed' },
  { path: '/meet', title: 'Meet', tab: '/meet', parent: '/world', links: links(
    ['/meet/parties/new', 'Create party'], ['/meet/people', 'People'], ['/meet/intro/edit', 'Your intro']) },
  { path: '/meet/parties/new', title: 'Create party', tab: '/meet', parent: '/meet' },
  { path: '/meet/parties/:partyId', title: 'Party', tab: '/meet', parent: '/meet' },
  { path: '/meet/parties/:partyId/manage', title: 'Manage party', tab: '/meet', parent: '/meet/parties/:partyId' },
  { path: '/meet/parties/:partyId/rally', title: 'Rendezvous', tab: '/meet', parent: '/meet/parties/:partyId' },
  { path: '/meet/beacons/:beaconId', title: 'Find each other', tab: '/meet', parent: '/meet' },
  { path: '/meet/invitations/:invitationId', title: 'Meeting invitation', tab: '/meet', parent: '/meet' },
  { path: '/meet/invitations/:invitationId/conversation', title: 'Meeting conversation', tab: '/meet', parent: '/meet/invitations/:invitationId' },
  { path: '/meet/people', title: 'People', tab: '/meet', parent: '/meet' },
  { path: '/meet/intro/edit', title: 'Your intro', tab: '/meet', parent: '/meet' },
  { path: '/people/:userId', title: 'Profile', tab: '/meet', parent: '/meet/people' },
  { path: '/gifts', title: 'Gifts', tab: '/gifts', parent: '/world', links: links(
    ['/gifts/mine', 'My gifts'], ['/gifts/new', 'Create giveaway']) },
  { path: '/gifts/new', title: 'Create giveaway', tab: '/gifts', parent: '/gifts' },
  { path: '/gifts/mine', title: 'My gifts', tab: '/gifts', parent: '/gifts' },
  { path: '/gifts/:giveawayId', title: 'Giveaway', tab: '/gifts', parent: '/gifts' },
  { path: '/gifts/:giveawayId/manage', title: 'Manage giveaway', tab: '/gifts', parent: '/gifts/:giveawayId' },
  { path: '/gifts/:giveawayId/reservation', title: 'Reserve', tab: '/gifts', parent: '/gifts/:giveawayId' },
  { path: '/gifts/pickups/:reservationId', title: 'My pickup', tab: '/gifts', parent: '/gifts/mine' },
  { path: '/wall', title: 'Wall', tab: '/wall', parent: '/world' },
  { path: '/me', title: 'My profile', tab: '/me', parent: '/world', links: links(
    ['/me/avatar', 'Customize'], ['/me/profile', 'Edit profile'], ['/me/social', 'Social links'],
    ['/me/requests', 'Requests'], ['/me/age', 'Age & privacy'], ['/me/settings', 'Settings'],
    ['/me/blocks', 'Blocked users'], ['/me/account', 'Account']) },
  ...[
    ['avatar', 'Customize'], ['profile', 'Edit profile'], ['social', 'Social links'],
    ['requests', 'Requests'], ['age', 'Age & privacy'], ['settings', 'Settings'],
    ['blocks', 'Blocked users'], ['account', 'Account'],
  ].map(([path, title]) => ({ path: `/me/${path}`, title: title!, tab: '/me' as const, parent: '/me' })),
  { path: '/notifications', title: 'Inbox', tab: '/me', parent: '/world' },
  { path: '/report', title: 'Report', tab: '/me', parent: '/world' },
]
export const publicPaths = ['/', '/login', '/signup', '/verify-email', '/reset-password',
  '/auth/callback', '/legal/:document', '/account/deleted', '/archive'] as const
export const adminPaths = ['/admin', '/admin/event', '/admin/places', '/admin/reports',
  '/admin/users/:userId', '/admin/content', '/admin/mural', '/admin/audit', '/admin/archive'] as const

export const canonicalId = (value: string) => /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
export function resolvePanel(pathname: string) {
  // Prefer static siblings such as /gifts/mine over /gifts/:giveawayId.
  const candidates = [...panelRoutes].sort((a, b) => Number(a.path.includes(':')) - Number(b.path.includes(':')))
  for (const route of candidates) {
    const match = matchPath(route.path, pathname)
    if (match) return { route, params: match.params,
      valid: Object.values(match.params).every(value => value !== undefined && canonicalId(value)) }
  }
  return null
}
export function parentPath(pathname: string): string {
  const resolved = resolvePanel(pathname)
  if (!resolved?.valid) return '/world'
  return resolved.route.parent.replace(/:([A-Za-z]+)/g, (_, key: string) => resolved.params[key] ?? '')
}

/** A return URL is a known participant destination, never a public/auth/admin redirect chain.
 * Reject before URL normalization so traversal, escaped separators and secrets cannot hide in it.
 * Only the report route has a query contract here; domain filters need their owning handoff.
 */
export function safeReturnTarget(value: unknown, fallback = '/world'): string {
  if (typeof value !== 'string' || value.length > 512 || !value.startsWith('/')
    || value.startsWith('//') || /[\\%#\s]/.test(value)
    || [...value].some(character => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127)) return fallback
  const [path, query, extra] = value.split('?')
  if (!path || extra !== undefined || path.split('/').some(segment => segment === '.' || segment === '..')) return fallback
  const resolved = resolvePanel(path)
  if (!resolved?.valid) return fallback
  if (query !== undefined) {
    if (path !== '/report') return fallback
    const params = new URLSearchParams(query)
    // Only lexical URL safety here. C04/FE36 own target semantics and authorization.
    if ([...params].length !== 2 || !/^[A-Za-z][A-Za-z_-]{0,31}$/.test(params.get('targetType') ?? '')
      || !canonicalId(params.get('targetId') ?? '')) return fallback
  }
  return value
}
