/**
 * WP-09 T-901: the three-section primary navigation.
 *
 * The product replaces the concert-first tab bar with three sections
 * (product-plan.md §7, charter decision D-10):
 *
 * | Section | Contents |
 * | --- | --- |
 * | Discover | People (shared interests) and Mutuals-intro posts; compose entry |
 * | Activity | Likes received, comments, replies, follow notifications |
 * | My Profile | Introduction, favorites, profile image, emo character, posts, room entry |
 *
 * The concert screens — world, feed, meet, gifts, wall — are **not** primary
 * navigation. They stay reachable from a secondary menu, which is a separate
 * disclosure in the shell rather than a fourth tab.
 *
 * This module is the single source of that structure. It holds:
 *
 * * [`sections`] — exactly three entries, in product order;
 * * [`secondaryDestinations`] — the concert screens, in their old order;
 * * [`legacyRedirects`] — every old primary path and where it now goes;
 * * [`isPrimaryPath`] / [`sectionForPath`] — the active-section decision.
 *
 * The redirect table is data so a test can prove that **every** old path still
 * resolves, rather than spot-checking a few.
 */

/** A primary section. Closed: there are exactly three and there is no fourth. */
export type SectionId = 'DISCOVER' | 'ACTIVITY' | 'MY_PROFILE'

export interface Section {
  readonly id: SectionId
  /** The tab label. Short enough for a 360 px tab bar. */
  readonly label: string
  /** The section's home path. */
  readonly path: string
  /** The icon the shell draws for it. */
  readonly icon: 'discover' | 'activity' | 'me'
  /**
   * The path prefixes that belong to this section. A path is in a section when
   * it equals the home path or starts with one of these prefixes and a `/`.
   */
  readonly prefixes: readonly string[]
}

/**
 * The three primary sections, in the product's order.
 *
 * The order matters: it is the tab order, and it is what the keyboard arrow
 * navigation and the `aria-current` decision follow.
 */
export const sections: readonly Section[] = Object.freeze([
  Object.freeze({
    id: 'DISCOVER' as const,
    label: 'Discover',
    path: '/discover',
    icon: 'discover' as const,
    prefixes: Object.freeze(['/discover', '/people']),
  }),
  Object.freeze({
    id: 'ACTIVITY' as const,
    label: 'Activity',
    path: '/activity',
    icon: 'activity' as const,
    // The inbox is the Activity section's list: `/notifications` is where the
    // likes, comments, replies and follows actually live. It is a *prefix* of the
    // section, so `/notifications` and `/notifications/:id` both activate it.
    prefixes: Object.freeze(['/activity', '/notifications']),
  }),
  Object.freeze({
    id: 'MY_PROFILE' as const,
    label: 'My Profile',
    path: '/me',
    icon: 'me' as const,
    prefixes: Object.freeze(['/me', '/profile-media']),
  }),
])

/**
 * The section id of a path, or `null` when the path is not in a section.
 *
 * A path is in a section when it equals the section's home path, equals one of
 * its prefixes, or starts with a prefix and a `/`. The middle case is what makes
 * `/notifications` (the Activity list) activate Activity without also making
 * `/notifications-settings` a match.
 */
export function sectionForPath(pathname: string): SectionId | null {
  for (const section of sections) {
    if (pathname === section.path) return section.id
    for (const prefix of section.prefixes) {
      if (pathname === prefix || pathname.startsWith(`${prefix}/`)) return section.id
    }
  }
  return null
}

/** Whether a path is inside one of the three primary sections. */
export const isPrimaryPath = (pathname: string): boolean => sectionForPath(pathname) !== null

/**
 * A concert destination in the secondary menu.
 *
 * `path` is the destination as it is reached today. The concert screens keep
 * their existing URLs: the product decision is that they leave the *primary*
 * navigation, not that they move.
 */
export interface SecondaryDestination {
  readonly path: string
  readonly label: string
  readonly icon: 'world' | 'feed' | 'meet' | 'gifts' | 'wall'
}

/**
 * The concert screens, in their old tab order.
 *
 * This is the whole of the secondary menu. It is deliberately not a second
 * navigation tree: each entry is one destination, and the screens below them are
 * reached from within those screens exactly as before.
 */
export const secondaryDestinations: readonly SecondaryDestination[] = Object.freeze([
  Object.freeze({ path: '/world', label: 'World', icon: 'world' as const }),
  Object.freeze({ path: '/feed', label: 'Feed', icon: 'feed' as const }),
  Object.freeze({ path: '/meet', label: 'Meet', icon: 'meet' as const }),
  Object.freeze({ path: '/gifts', label: 'Gifts', icon: 'gifts' as const }),
  Object.freeze({ path: '/wall', label: 'Wall', icon: 'wall' as const }),
])

/** The label of the secondary menu disclosure. */
export const SECONDARY_MENU_LABEL = 'Concert screens'

/**
 * A compatibility redirect for an old primary path.
 *
 * `from` is the path that used to be a primary tab (or a primary tab's landing
 * page). `to` is where it goes now. A redirect is a *replace*, never a push, so
 * the browser's Back button does not bounce between the old and new homes.
 */
export interface LegacyRedirect {
  readonly from: string
  readonly to: string
}

/**
 * Every old primary path and its new home.
 *
 * The old tab bar had six entries: `/world`, `/feed`, `/meet`, `/gifts`, `/wall`,
 * `/me`. Only `/me` survives as a primary section — and it survives with the same
 * path, so it needs no redirect. The other five are concert screens now, and
 * their old *tab* position is what changed, not their URL: a user who had `/feed`
 * bookmarked still lands on the feed.
 *
 * The redirects below are for the paths whose meaning changed:
 *
 * * `/meet/people` was the concert "People" screen and is now the Discover
 *   section's people list;
 * * `/meet/intro/edit` was the concert intro editor and is now the mutuals-intro
 *   composer in Discover;
 * * `/people/:userId` moved under Discover so a visitor profile is reached from
 *   the section it belongs to. The old path redirects, so a bookmarked visitor
 *   profile still resolves.
 *
 * Each entry is a *prefix* redirect: the remainder of the path is preserved and
 * appended, so `/meet/people` and any child route stay coherent.
 *
 * The order matters. `/meet/people` is the more specific prefix and comes first,
 * so `/meet/people/x` is not captured by the `/people` entry (which would be a
 * false match: `/meet/people` does not start with `/people/`).
 */
export const legacyRedirects: readonly LegacyRedirect[] = Object.freeze([
  Object.freeze({ from: '/meet/people', to: '/discover/people' }),
  Object.freeze({ from: '/meet/intro/edit', to: '/discover/intro' }),
  Object.freeze({ from: '/people', to: '/discover/people' }),
])

/**
 * Resolve a legacy path to its new destination.
 *
 * Returns `null` when the path is not a legacy path. The match is on whole
 * segments, so `/meet/people` matches `/meet/people` and `/meet/people/x` but
 * `/meet/peopled` does not.
 */
export function legacyRedirect(pathname: string): string | null {
  for (const { from, to } of legacyRedirects) {
    if (pathname === from) return to
    if (pathname.startsWith(`${from}/`)) return `${to}${pathname.slice(from.length)}`
  }
  return null
}

/**
 * Whether a panel route is a legacy path that now redirects.
 *
 * A legacy path keeps its panel entry so the shell still knows its title and its
 * parent (Back from a redirected screen is coherent), but the router must not
 * register it as a panel: the redirect owns the URL. Registering both makes the
 * ranking depend on which pattern React Router considers more specific, which is
 * not a contract worth relying on.
 */
export function isLegacyPath(path: string): boolean {
  // A panel path may carry a parameter (`/people/:userId`); the redirect table
  // names the concrete prefix (`/people`). Compare segment by segment, treating a
  // `:param` segment as one segment of the legacy prefix.
  const segments = path.split('/')
  return legacyRedirects.some(({ from }) => {
    const legacy = from.split('/')
    return segments.length >= legacy.length
      && legacy.every((segment, index) => segments[index]!.startsWith(':') || segments[index] === segment)
  })
}

/**
 * The concrete route patterns a legacy redirect must be registered under.
 *
 * A redirect is registered as a real route so it runs *inside* the shell: a
 * signed-in user who follows an old link keeps the shell and its world
 * connection instead of being dropped to the outer not-found page. Each pattern
 * is the legacy prefix plus one splat, which covers the prefix itself and every
 * child path.
 */
export function legacyRedirectPaths(): readonly string[] {
  return legacyRedirects.map(({ from }) => `${from}/*`)
}

/**
 * The routes of the three sections, for the router and for the tests.
 *
 * Every screen the product plan lists under a section appears here. A section
 * screen that is not listed is not reachable, which is the point: the list is
 * closed and a test compares it against the router's registration.
 */
export const sectionRoutes = Object.freeze({
  DISCOVER: Object.freeze([
    '/discover',
    '/discover/people',
    '/discover/intro',
    '/discover/intro/:introId',
    '/discover/people/:profileId',
  ]),
  ACTIVITY: Object.freeze(['/activity', '/notifications', '/notifications/:notificationId']),
  MY_PROFILE: Object.freeze([
    '/me',
    '/me/profile',
    '/me/settings',
    '/me/identity',
    '/me/avatar',
    '/me/posts/new',
    '/me/room',
  ]),
} as const)

/** Every section route, flattened. */
export const allSectionRoutes: readonly string[] = Object.freeze([
  ...sectionRoutes.DISCOVER,
  ...sectionRoutes.ACTIVITY,
  ...sectionRoutes.MY_PROFILE,
])

/**
 * The visitor profile route.
 *
 * A visitor profile is not the viewer's own profile, so it is not in
 * `MY_PROFILE`; it lives under Discover because that is where a viewer finds
 * people. It is listed separately so the router can register it without a
 * section claiming it as its own home.
 */
export const VISITOR_PROFILE_ROUTE = '/discover/people/:profileId'

/** The room route. A subordinate screen of a profile, never a section. */
export const visitorRoomRoute = (profileId: string) => `/discover/people/${profileId}/room`
