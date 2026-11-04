import { QueryClientProvider } from '@tanstack/react-query'
import { lazy, Suspense, type ReactNode } from 'react'
import { createBrowserRouter, Link, Outlet, type RouteObject } from 'react-router-dom'
import { Feedback } from '../shared/ui'
import { WorldPanelOwnership } from './world-panels/ownership'
import { PortsProvider } from './access'
import { SafetyOwnership } from './safety-entry/context'
import { AccountDeletionOwnership } from './account-deletion-entry/context'
import type { ShellPorts } from './ports'
import { adminPaths, panelRoutes, publicPaths } from './routes'
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
const MapPanel = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionMapPanel })))
const PlacePanel = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionPlacePanel })))
const ThemePanel = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionThemePanel })))
const WallPanel = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionWallPanel })))
const SocialPanel = lazy(() => import('./social-composition').then(module => ({ default: module.SocialRoutePanel })))
const WorldHelp = lazy(() => import('../features/world-help/WorldHelp').then(module => ({ default: module.WorldHelp })))
const socialPaths = ['/feed', '/feed/new', '/feed/:postId', '/feed/:postId/edit', '/notices/:noticeId', '/meet/people', '/people/:userId', '/meet/intro/edit', '/me/age', '/notifications', '/me/social', '/me/requests'] as const
const featurePanels: Readonly<Record<string, ReactNode>> = {
  ...communityEntryPanels,
  ...Object.fromEntries(socialPaths.map(path => [path, <Suspense fallback={<p role="status">Opening this section…</p>}><SocialPanel /></Suspense>])),
  '/world/map': <Suspense fallback={<p role="status">Opening map…</p>}><MapPanel /></Suspense>,
  '/world/places/:placeId': <Suspense fallback={<p role="status">Opening place…</p>}><PlacePanel /></Suspense>,
  '/world/theme': <Suspense fallback={<p role="status">Opening eras…</p>}><ThemePanel /></Suspense>,
  '/world/help': <Suspense fallback={<p role="status">Opening controls…</p>}><WorldHelp /></Suspense>,
  '/wall': <Suspense fallback={<p role="status">Opening wall…</p>}><WallPanel /></Suspense>,
  '/me': <Suspense fallback={<p role="status">Opening your account…</p>}><AccountPanel view="me" /></Suspense>,
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
  '/meet': <Suspense fallback={<p role="status">Opening parties…</p>}><PartiesPanel /></Suspense>,
  '/meet/parties/:partyId': <Suspense fallback={<p role="status">Opening parties…</p>}><PartiesPanel detail /></Suspense>,
  '/gifts': <Suspense fallback={<p role="status">Opening gifts…</p>}><GiftsPanel view="list" /></Suspense>,
  '/gifts/:giveawayId': <Suspense fallback={<p role="status">Opening gifts…</p>}><GiftsPanel view="detail" /></Suspense>,
  '/gifts/mine': <Suspense fallback={<p role="status">Opening gifts…</p>}><GiftsPanel view="mine" /></Suspense>,
}
function LazyBoundary({ children }: { children: ReactNode }) {
  return <Suspense fallback={<main className="shell-gate"><Feedback kind="status" title="Opening this page" />
    <Link className="shell-link" to="/">Back to home</Link></main>}>{children}</Suspense>
}
export function RouteFailure() {
  // Never render thrown responses, URLs, stack traces or transport payloads.
  return <div className="shell-gate"><RouteState kind="unavailable" reload={() => location.reload()} /></div>
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
      ...panelRoutes.map(route => ({ path: route.path, element: <PanelScreen route={route}>{composition.panels?.[route.path] ?? featurePanels[route.path]}</PanelScreen>, errorElement: <RouteFailure /> })),
      ...['/world/*', '/feed/*', '/meet/*', '/gifts/*', '/wall/*', '/me/*'].map(path => ({ path, element: <RouteState kind="not-found" reload={() => location.reload()} /> })),
    ] },
    { path: '*', element: <RouteState kind="not-found" reload={() => location.reload()} /> },
  ] }]
}
export const createAppRouter = (ports: ShellPorts) => createBrowserRouter(createAppRoutes(ports))
