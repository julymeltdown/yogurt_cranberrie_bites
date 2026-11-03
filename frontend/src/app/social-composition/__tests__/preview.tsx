import { StrictMode, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider, useLocation, Link } from 'react-router-dom'
import { PortsProvider } from '../../access'
import { WorldHost } from '../../WorldHost'
import { SocialFeatures, useSocialOwner } from '../provider'
import { SocialNavigationBoundary } from '../navigation'
import { SocialRoutePanel, useSocialUnreadTotal } from '../panels'
import { fixture, user } from './fixtures'
import type { SocialOwner } from '../owner'
import './preview.css'
const f = fixture(r => globalThis.fetch(r)), active = { current: null as SocialOwner | null }
const navigate = (d: { path: string; scope: typeof f.scope }) => { if (f.auth.privacy.isCurrent(d.scope)) void router.navigate(d.path) }
const scope = { userId: user, eventId: f.scope.eventId!, generation: f.scope.generation }
function Toolbar() {
  const owner = useSocialOwner(), total = useSocialUnreadTotal()
  useLayoutEffect(() => { active.current = owner }, [owner])
  return <nav aria-label="Fixture navigation"><p>TEST ONLY — canonical controllers, instrumented world lease, synthetic HTTP</p>
    {Object.entries({ World: '/world', Feed: '/feed', Compose: '/feed/new', People: '/meet/people', Intro: '/meet/intro/edit', Age: '/me/age', Inbox: '/notifications', Requests: '/me/requests' }).map(([label, path]) => <button key={path} type="button" onClick={() => owner?.navigate(path)}>{label}</button>)}
    <Link to="/notifications">Inbox router link</Link>
    <output aria-label="Canonical unread total">{total?.total ?? 'Unavailable'}</output>
  </nav>
}
function Root() {
  const location = useLocation()
  return <PortsProvider ports={f.ports}><SocialFeatures {...f.options} navigate={navigate}><SocialNavigationBoundary><Toolbar />
    <WorldHost scope={scope} bootstrap={f.bootstrap} privacyScope={f.scope} blocked={location.pathname !== '/world'} covered={false} />
    {location.pathname !== '/world' && <SocialRoutePanel />}
  </SocialNavigationBoundary></SocialFeatures></PortsProvider>
}
const router = createMemoryRouter([{ path: '*', element: <Root /> }], { initialEntries: [new URLSearchParams(location.search).get('initial') ?? '/feed'] })
Object.assign(window, { __SOCIAL__: {
  read: () => active.current?.read(), requests: () => f.requests.map(r => ({ path: new URL(r.url).pathname, method: r.method })),
  stats: () => ({ ...f.stats }), current: () => router.state.location.pathname,
  command: () => { const t = active.current?.thread.read(); return t ? { key: t.mutation.key, phase: t.mutation.phase, draft: t.draft.text } : null },
  policy: () => active.current?.notifyPolicy({ scope: f.scope, reason: 'consent-revoked' }),
  logout: () => f.auth.session.replace(f.auth.session.read().generation, null),
  availability(value: boolean) { f.setLease(value ? { scope: f.scope, bootstrap: f.bootstrap, services: { feed: true, people: true, intro: true, age: true, inbox: true } } : null) },
  back: () => router.navigate(-1),
} })
createRoot(document.getElementById('root')!).render(<StrictMode><RootRouter /></StrictMode>)
function RootRouter() { return <RouterProvider router={router} /> }
