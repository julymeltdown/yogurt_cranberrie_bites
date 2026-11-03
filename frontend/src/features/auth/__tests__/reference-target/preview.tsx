import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { PortsProvider } from '../../../../app/access'
import { SignIn } from '../../ui/SignIn'
import { createSessionController } from '../../model/session'
import { authResponse, capabilities, eventId, isolatedCookiePolicy } from '../fixtures/data'
import './preview.css'
const params = new URLSearchParams(location.search), mode = params.get('mode') ?? 'ready'
const cookiePolicy = isolatedCookiePolicy()
let loginCount = 0, settle: (() => void) | null = null
const calls: string[] = []
const auth = createSessionController({ eventId: mode === 'unavailable' ? null : eventId, cookiePolicy,
  fetch: async input => {
    const request = input as Request, path = new URL(request.url).pathname; calls.push(path)
    if (path.endsWith('/capabilities')) {
      if (mode === 'options-error') return new Response(null, { status: 503 })
      if (mode === 'options-loading') return new Promise<Response>(() => {})
      return Response.json(capabilities())
    }
    if (path.endsWith('/login')) {
      loginCount++
      if (mode === 'pending') await new Promise<void>(resolve => { settle = resolve })
      const body = await request.json() as { password: string }
      if (mode === 'pending' || body.password === 'wrong') return Response.json({ code: 'BAD', message: 'PRIVATE_RAW_ERROR' }, { status: 401 })
      if (body.password === 'throttle') return new Response(null, { status: 429 })
      return Response.json(authResponse())
    }
    return new Response(null, { status: 503 })
  },
})
auth.start()
const router = createMemoryRouter([
  { path: '/login', element: <SignIn /> },
  { path: '*', element: <p role="status">Fixture destination reached</p> },
], { initialEntries: [`/login?returnTo=${encodeURIComponent(params.get('target') ?? '/feed')}`] })
const ports = { auth, session: auth.session, readiness: auth.readiness, bootstrap: { load: auth.loadBootstrap }, admin: { authorize: async () => false } }
createRoot(document.getElementById('root')!).render(<StrictMode><PortsProvider ports={ports}><RouterProvider router={router} /></PortsProvider></StrictMode>)
window.__REF_AUTH__ = { auth, calls, count: () => loginCount, settle: () => settle?.(), invalidate: () => cookiePolicy.invalidate(), path: () => router.state.location.pathname, navigate: (path: string) => router.navigate(path), dispose: () => { router.dispose(); auth.dispose() } }
declare global { interface Window { __REF_AUTH__: { auth: typeof auth; calls: string[]; count(): number; settle(): void; invalidate(): void; path(): string; navigate(path: string): Promise<void>; dispose(): void } } }
