import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { createSessionController } from '../../../features/auth/model/session'
import { eventId, isolatedCookiePolicy } from '../../../features/auth/__tests__/fixtures/data'
import { createFixturePorts } from '../../__tests__/fixtures/ports'
import { createAppRoutes } from '../../router'
import '../../../shared/styles/base.css'
import '../../shell.css'
if (!import.meta.env.DEV) throw new Error('Synthetic development-only qualification')
const params = new URLSearchParams(location.search), auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy() }), world = createFixturePorts()
auth.start()
if (auth.read().capabilities === 'loading') await new Promise<void>(resolve => { const stop = auth.subscribe(() => { if (auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
if (!params.has('anonymous') && !await auth.login('synthetic@example.org', 'fixture-password')) throw new Error('Synthetic login unavailable')
const initial = ['/me/settings', '/me/account', '/account/deleted', '/world'].includes(params.get('route') ?? '') ? params.get('route')! : '/me/settings'
history.replaceState(null, '', initial)
const router = createBrowserRouter(createAppRoutes({ ...world.ports, auth, session: auth.session, readiness: auth.readiness, bootstrap: { load: auth.loadBootstrap } }))
window.__ACCOUNT_DELETION_ROOT__ = {
  stats: world.stats, snapshot: () => ({ userId: auth.session.read().userId, gate: auth.privacy.capture().gate, privateQueries: auth.queryClient.getQueryCache().getAll().length }),
  seedPrivate: () => auth.queryClient.setQueryData(['private', 'sentinel'], 'PRIVATE sentinel'),
  privateCleared: () => auth.queryClient.getQueryData(['private', 'sentinel']) === undefined,
  changeIdentity: (userId: string) => auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'NEXT_SYNTHETIC' }),
  disposeAuth: () => auth.dispose(), navigate: (path: string) => router.navigate(path),
}
declare global { interface Window { __ACCOUNT_DELETION_ROOT__: { stats: typeof world.stats; snapshot(): { userId: string | null; gate: string; privateQueries: number }; seedPrivate(): unknown; privateCleared(): boolean; changeIdentity(userId: string): unknown; disposeAuth(): void; navigate(path: string): Promise<void> } } }
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router}/></StrictMode>)
