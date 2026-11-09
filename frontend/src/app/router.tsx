import { QueryClientProvider } from '@tanstack/react-query'
import { lazy, Suspense, type ReactNode } from 'react'
import { createBrowserRouter, Link, Navigate, Outlet, useLocation, type RouteObject } from 'react-router-dom'
import { Feedback } from '../shared/ui'
import { WorldPanelOwnership } from './world-panels/ownership'
import { PortsProvider } from './access'
import { SafetyOwnership } from './safety-entry/context'
import { AccountDeletionOwnership } from './account-deletion-entry/context'
import type { ShellPorts } from './ports'
import { adminPaths, panelRoutes, publicPaths } from './routes'
import { isLegacyPath, legacyRedirect, legacyRedirectPaths } from './sections'
import { PanelScreen, WorldBoundary } from './WorldShell'
import { RouteState } from './errors/RouteState'
import { communityEntryPanels } from './community-entry'

const PublicBoundary = lazy(() => import('./PublicBoundary'))
const AdminBoundary = lazy(() => import('./AdminBoundary'))
const GiftsPanel = lazy(() => import('./CommunityFeatures').then(module => ({ default: module.GiftsPanel })))
const PartiesPanel = lazy(() => import('./CommunityFeatures').then(module => ({ default: module.PartiesPanel })))
const RoomsPanel = lazy(() => import('./world-entry/root-features').then(module => ({ default: module.ProductionRoomsPanel })))
const NearbyPanel = lazy(() => import('./world-entry/root-features').then(module => ({ default: module.ProductionNearbyPanel })))
const AccountPanel = lazy(() => import('./account-entry/panels'))
const ReportPanel = lazy(() => import('./safety-entry/panels').then(module => ({ default: module.ReportPanel })))
const BlockedUsersPanel = lazy(() => import('./safety-entry/panels').then(module => ({ default: module.BlockedUsersPanel })))
const AccountControlsPanel = lazy(() => import('./account-deletion-entry/panel'))
const MeetingCheckInPanel = lazy(() => import('./meeting-entry').then(module => ({ default: module.MeetingCheckInPanel })))
const MeetingBeaconPanel = lazy(() => import('./meeting-entry').then(module => ({ default: module.MeetingBeaconPanel })))
const MapPanel = lazy(() => import('./venue-entry/ParadiseCityMapPanel').then(module => ({ default: module.ParadiseCityMapPanel })))
const PlacePanel = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionPlacePanel })))
const ThemePanel = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionThemePanel })))
const WallPanel = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionWallPanel })))
const SocialPanel = lazy(() => import('./social-composition').then(module => ({ default: module.SocialRoutePanel })))
const VenueInvitationPanel = lazy(() => import('./venue-community-entry').then(module => ({ default: module.VenueInvitationPanel })))
const VenueConversationPanel = lazy(() => import('./venue-community-entry').then(module => ({ default: module.VenueConversationPanel })))
const WorldHelp = lazy(() => import('../features/world-help/WorldHelp').then(module => ({ default: module.WorldHelp })))
const RoomPanel = lazy(() => import('./profile-room-entry/RoomPanel').then(module => ({ default: module.RoomRoutePanel })))
// WP-09: the three sections' screens. Each is its own lazy chunk, so opening the
// profile does not download the discovery list and vice versa.
const DiscoverPanel = lazy(() => import('./section-entry/DiscoverPanel').then(module => ({ default: module.DiscoverPanel })))
const ActivityPanel = lazy(() => import('./section-entry/ActivityPanel').then(module => ({ default: module.ActivityPanel })))
const MyProfilePanel = lazy(() => import('./section-entry/MyProfilePanel').then(module => ({ default: module.MyProfilePanel })))
const PhotoComposerPanel = lazy(() => import('./section-entry/PhotoComposerPanel').then(module => ({ default: module.PhotoComposerPanel })))
const IdentitySettingsPanel = lazy(() => import('./section-entry/IdentitySettingsPanel').then(module => ({ default: module.IdentitySettingsPanel })))
// WP-05 owns the intro screens. The composer is mounted: every operation it uses
// (`saveIntroPostDraft`, `publishIntroPost`, `deleteIntroPost`, `getOwnIntroPost`)
// is served by the concert-hosted profile transport, so it works end to end.
//
// The discovery list and the post detail are NOT mounted. They need
// `listIntroPosts` and `getIntroPost`, which require a visibility-filtered query
// that does not exist yet (WP-04 owns the predicate, WP-05 the query). Those
// routes stay on the explicit placeholder rather than mounting a screen that
// would fail on every load — an empty or broken feed is worse than an honest
// "not in this build yet".
const IntroSlotPanel = lazy(() => import('./section-entry/IntroSlot').then(module => ({ default: module.IntroSlot })))
const IntroComposerPanel = lazy(() => import('./section-entry/IntroComposerPanel').then(module => ({ default: module.IntroComposerPanel })))
// WP-09: the people list and the visitor profile moved under Discover. The
// legacy spellings stay in this list so a panel registered under one still gets
// the social surface rather than the generic placeholder.
const socialPaths = ['/feed', '/feed/new', '/feed/:postId', '/feed/:postId/edit', '/notices/:noticeId',
  '/discover/people', '/discover/people/:profileId', '/discover/intro', '/meet/people', '/people/:userId',
  '/meet/intro/edit', '/me/age', '/notifications', '/me/social', '/me/requests'] as const
const featurePanels: Readonly<Record<string, ReactNode>> = {
  ...communityEntryPanels,
  '/meet/invitations/:invitationId': <Suspense fallback={<p role="status">Opening invitation…</p>}><VenueInvitationPanel /></Suspense>,
  '/meet/invitations/:invitationId/conversation': <Suspense fallback={<p role="status">Opening conversation…</p>}><VenueConversationPanel /></Suspense>,
  ...Object.fromEntries(socialPaths.map(path => [path, <Suspense fallback={<p role="status">Opening this section…</p>}><SocialPanel /></Suspense>])),
  '/world/map': <Suspense fallback={<p role="status">Opening map…</p>}><MapPanel /></Suspense>,
  '/world/places/:placeId': <Suspense fallback={<p role="status">Opening place…</p>}><PlacePanel /></Suspense>,
  '/world/theme': <Suspense fallback={<p role="status">Opening eras…</p>}><ThemePanel /></Suspense>,
  '/world/help': <Suspense fallback={<p role="status">Opening controls…</p>}><WorldHelp /></Suspense>,
  '/wall': <Suspense fallback={<p role="status">Opening wall…</p>}><WallPanel /></Suspense>,
  // WP-09 T-902: `/me` is the My Profile section home — the owner profile page.
  // It is no longer the concert MeHub.
  '/me': <Suspense fallback={<p role="status">Opening your profile…</p>}><MyProfilePanel /></Suspense>,
  '/me/profile': <Suspense fallback={<p role="status">Opening your profile…</p>}><AccountPanel view="profile" /></Suspense>,
  '/me/avatar': <Suspense fallback={<p role="status">Opening your avatar…</p>}><AccountPanel view="avatar" /></Suspense>,
  '/report': <Suspense fallback={<p role="status">Opening report…</p>}><ReportPanel /></Suspense>,
  '/me/blocks': <Suspense fallback={<p role="status">Opening blocked users…</p>}><BlockedUsersPanel /></Suspense>,
  '/me/account': <Suspense fallback={<p role="status">Opening account controls…</p>}><AccountControlsPanel /></Suspense>,
  '/me/settings': <Suspense fallback={<p role="status">Opening settings…</p>}><AccountPanel view="settings" /></Suspense>,
  '/world/rooms': <Suspense fallback={<p role="status">Opening rooms…</p>}><RoomsPanel /></Suspense>,
  '/world/nearby': <Suspense fallback={<p role="status">Opening nearby…</p>}><NearbyPanel /></Suspense>,
  '/world/check-in': <Suspense fallback={<p role="status">Opening check-in…</p>}><MeetingCheckInPanel /></Suspense>,
  '/meet/beacons/:beaconId': <Suspense fallback={<p role="status">Opening meeting…</p>}><MeetingBeaconPanel /></Suspense>,
  '/people/:userId/room': <Suspense fallback={<p role="status">Opening room…</p>}><RoomPanel /></Suspense>,
  // --- WP-09: the three primary sections -----------------------------------
  '/discover': <Suspense fallback={<p role="status">Opening Discover…</p>}><DiscoverPanel /></Suspense>,
  '/discover/people': <Suspense fallback={<p role="status">Opening people…</p>}><SocialPanel /></Suspense>,
  '/discover/people/:profileId': <Suspense fallback={<p role="status">Opening profile…</p>}><SocialPanel /></Suspense>,
  '/discover/people/:profileId/room': <Suspense fallback={<p role="status">Opening room…</p>}><RoomPanel /></Suspense>,
  // The composer works end to end; the list and detail wait on the two unserved
  // reads. See the `IntroSlotPanel` comment above.
  '/discover/intro': <Suspense fallback={<p role="status">Opening your introduction…</p>}><IntroComposerPanel /></Suspense>,
  '/discover/intro/:introId': <Suspense fallback={<p role="status">Opening mutuals-intro post…</p>}><IntroSlotPanel view="detail" /></Suspense>,
  '/activity': <Suspense fallback={<p role="status">Opening activity…</p>}><ActivityPanel /></Suspense>,
  '/me/identity': <Suspense fallback={<p role="status">Opening identity settings…</p>}><IdentitySettingsPanel /></Suspense>,
  '/me/posts/new': <Suspense fallback={<p role="status">Opening the photo composer…</p>}><PhotoComposerPanel /></Suspense>,
  '/me/room': <Suspense fallback={<p role="status">Opening room…</p>}><RoomPanel /></Suspense>,
  '/notifications/:notificationId': <Suspense fallback={<p role="status">Opening notification…</p>}><SocialPanel /></Suspense>,
  '/meet': <Suspense fallback={<p role="status">Opening parties…</p>}><PartiesPanel /></Suspense>,
  '/meet/parties/:partyId': <Suspense fallback={<p role="status">Opening parties…</p>}><PartiesPanel detail /></Suspense>,
  '/gifts': <Suspense fallback={<p role="status">Opening gifts…</p>}><GiftsPanel view="list" /></Suspense>,
  '/gifts/:giveawayId': <Suspense fallback={<p role="status">Opening gifts…</p>}><GiftsPanel view="detail" /></Suspense>,
  '/gifts/mine': <Suspense fallback={<p role="status">Opening gifts…</p>}><GiftsPanel view="mine" /></Suspense>,
}
function LazyBoundary({ children }: { children: ReactNode }) {
  return <Suspense fallback={<main className="shell-gate pi-route-state pi-route-state--opening"><Feedback kind="status" title="Opening this page" />
    <Link className="shell-link" to="/">Back to home</Link></main>}>{children}</Suspense>
}
/**
 * WP-09 T-901/T-904: a legacy path redirects to its new home.
 *
 * The redirect is a `replace`, so Back does not bounce between the old and new
 * URLs, and the query string is carried across so a shared link keeps its
 * filters. The target is produced by `legacyRedirect`, which only ever returns a
 * path from the closed table — never a caller-supplied URL.
 */
export function LegacyRedirect() {
  const location = useLocation()
  const target = legacyRedirect(location.pathname)
  if (target === null) return <RouteState kind="not-found" reload={() => window.location.reload()} />
  return <Navigate to={`${target}${location.search}`} replace />
}

export function RouteFailure() {
  // Never render thrown responses, URLs, stack traces or transport payloads.
  return <div className="shell-gate pi-route-wrap"><RouteState kind="unavailable" reload={() => location.reload()} /></div>
}
export interface RouteComposition {
  /** Reviewed feature elements attach here without taking ownership of the shell lifetime.
   * Elements still own their server authorization/data states. Test fixtures use the same seam.
   */
  panels?: Readonly<Record<string, ReactNode>>
}
export function createAppRoutes(ports: ShellPorts, composition: RouteComposition = {}): RouteObject[] {
  const outlet = <PortsProvider ports={ports}><SafetyOwnership><AccountDeletionOwnership><Outlet /></AccountDeletionOwnership></SafetyOwnership></PortsProvider>
  return [{ element: ports.auth ? <QueryClientProvider client={ports.auth.queryClient}>{outlet}</QueryClientProvider> : outlet, errorElement: <RouteFailure />, children: [
    ...publicPaths.map(path => ({ path, element: <LazyBoundary><PublicBoundary /></LazyBoundary>, errorElement: <RouteFailure /> })),
    ...adminPaths.map(path => ({ path, element: <LazyBoundary><AdminBoundary /></LazyBoundary>, errorElement: <RouteFailure /> })),
    { element: <WorldPanelOwnership places={!composition.panels?.['/world/map'] && !composition.panels?.['/world/places/:placeId']} mural={!composition.panels?.['/wall']} social={!socialPaths.some(path => composition.panels?.[path])}><WorldBoundary /></WorldPanelOwnership>, children: [
      // WP-09: a legacy path redirects before the panel routes it would otherwise
      // match, so `/meet/people` lands on `/discover/people` rather than rendering
      // the concert People panel. The redirect runs inside the shell, so a
      // signed-in user keeps the shell and its world connection.
      ...legacyRedirectPaths().map(path => ({ path, element: <LegacyRedirect />, errorElement: <RouteFailure /> })),
      ...panelRoutes.filter(route => !isLegacyPath(route.path)).map(route => ({ path: route.path, element: <PanelScreen route={route}>{composition.panels?.[route.path] ?? featurePanels[route.path]}</PanelScreen>, errorElement: <RouteFailure /> })),
      ...['/world/*', '/feed/*', '/meet/*', '/gifts/*', '/wall/*', '/me/*', '/discover/*', '/activity/*'].map(path => ({ path, element: <RouteState kind="not-found" reload={() => location.reload()} /> })),
    ] },
    { path: '*', element: <RouteState kind="not-found" reload={() => location.reload()} /> },
  ] }]
}
export const createAppRouter = (ports: ShellPorts) => createBrowserRouter(createAppRoutes(ports))
