import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { bootstrapFor, createFixturePorts } from '../../__tests__/fixtures/ports'
import { fixture, uid } from '../../../features/gift-discovery/__tests__/fixtures/data'
import '../../../shared/styles/base.css'
import '../../shell.css'

// Real default route composition, canonical Auth, and instrumented fake world.
// No composition override or meeting release is supplied by this fixture.
const auth = fixture(), world = createFixturePorts(); auth.controller.dispose()
const initialPath = new URLSearchParams(location.search).get('panel') === 'beacon'
  ? '/meet/beacons/00000000-0000-4000-8000-000000000015' : '/world/check-in'
const router = createMemoryRouter(createAppRoutes({ ...world.ports, auth: auth.auth, session: auth.auth.session,
  bootstrap: { load: async scope => bootstrapFor(scope) } }), { initialEntries: [initialPath] })
const root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
declare global { interface Window { __MEETING_ROOT__: {
  state(): { path: string; world: typeof world.stats; meetingRequests: number; writes: number }
  navigate(path: string): Promise<void>
  policy(): void
  identity(): void
  dispose(): void
} } }
window.__MEETING_ROOT__ = {
  state: () => ({ path: router.state.location.pathname, world: world.stats,
    meetingRequests: auth.requests.filter(r => new URL(r.url).pathname.includes('/beacons')).length,
    writes: auth.requests.filter(r => r.method !== 'GET').length }),
  navigate: path => router.navigate(path),
  policy: () => { auth.auth.privacy.changePolicy(auth.auth.privacy.capture(), 'block') },
  identity: () => { auth.auth.session.replace(auth.auth.session.read().generation,
    { userId: uid(90), eventId: uid(91), accessToken: 'synthetic-replacement-memory-token' }) },
  dispose() { root.unmount(); router.dispose(); auth.dispose() },
}
