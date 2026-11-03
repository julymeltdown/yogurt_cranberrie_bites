import { QueryClientProvider } from '@tanstack/react-query'
import { lazy, Suspense, type ReactNode } from 'react'
import { createBrowserRouter, Link, Outlet, type RouteObject } from 'react-router-dom'
import { Feedback } from '../shared/ui'
import { PortsProvider } from './access'
import type { ShellPorts } from './ports'
import { adminPaths, panelRoutes, publicPaths } from './routes'
import { PanelScreen, WorldBoundary } from './WorldShell'

const PublicBoundary = lazy(() => import('./PublicBoundary'))
const AdminBoundary = lazy(() => import('./AdminBoundary'))
function LazyBoundary({ children }: { children: ReactNode }) {
  return <Suspense fallback={<main className="shell-gate"><Feedback kind="status" title="Opening this page" />
    <Link className="shell-link" to="/">Back to home</Link></main>}>{children}</Suspense>
}
export function RouteFailure() {
  // Never render thrown responses, URLs, stack traces or transport payloads.
  return <div className="shell-gate"><Feedback kind="error" title="This page is unavailable">
    <p>We could not open this page. You can return safely and try another section.</p>
    <div className="shell-actions"><Link className="shell-link" to="/world">Back to world</Link>
      <Link className="shell-link" to="/">Back to home</Link></div>
  </Feedback></div>
}
export interface RouteComposition {
  /** Reviewed feature elements attach here without taking ownership of the shell lifetime.
   * Elements still own their server authorization/data states. Test fixtures use the same seam.
   */
  panels?: Readonly<Record<string, ReactNode>>
}
export function createAppRoutes(ports: ShellPorts, composition: RouteComposition = {}): RouteObject[] {
  const outlet = <PortsProvider ports={ports}><Outlet /></PortsProvider>
  return [{ element: ports.auth ? <QueryClientProvider client={ports.auth.queryClient}>{outlet}</QueryClientProvider> : outlet, errorElement: <RouteFailure />, children: [
    ...publicPaths.map(path => ({ path, element: <LazyBoundary><PublicBoundary /></LazyBoundary>, errorElement: <RouteFailure /> })),
    ...adminPaths.map(path => ({ path, element: <LazyBoundary><AdminBoundary /></LazyBoundary>, errorElement: <RouteFailure /> })),
    { element: <WorldBoundary />, children: [
      ...panelRoutes.map(route => ({ path: route.path, element: <PanelScreen route={route}>{composition.panels?.[route.path]}</PanelScreen>, errorElement: <RouteFailure /> })),
      ...['/world/*', '/feed/*', '/meet/*', '/gifts/*', '/wall/*', '/me/*'].map(path => ({ path, element: <RouteFailure /> })),
    ] },
    { path: '*', element: <LazyBoundary><PublicBoundary /></LazyBoundary> },
  ] }]
}
export const createAppRouter = (ports: ShellPorts) => createBrowserRouter(createAppRoutes(ports))
