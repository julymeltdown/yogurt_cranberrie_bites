// TEST ONLY: real browser paths, unchanged createAppRouter/createProductionPorts and canonical Auth.
// Synthetic capability/login/cookie policy only. Bootstrap is denied; no world or service acceptance.
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../../app/router'
import { createProductionPorts } from '../../../../../app/ports'
import { createSessionController } from '../../../model/session'
import { authResponse, capabilities, eventId, isolatedCookiePolicy } from '../../fixtures/data'
import '../../../../../shared/styles/base.css'
import '../../../../../app/shell.css'
const mode = new URLSearchParams(location.search).get('fixtureMode'), policy = isolatedCookiePolicy()
const calls: string[] = []
let count = 0, settle: (() => void) | undefined
const auth = createSessionController({ eventId: mode === 'unavailable' ? null : eventId, cookiePolicy: policy,
  fetch: async input => {
    const r = input as Request, path = new URL(r.url).pathname; calls.push(path)
    if (path.endsWith('/capabilities')) return mode === 'options-error' ? new Response(null, { status: 503 }) : Response.json(capabilities())
    if (path.endsWith('/login')) {
      count++
      if (mode === 'pending') await new Promise<void>(resolve => { settle = resolve })
      const body = await r.json() as { password: string }
      if (body.password === 'wrong') return Response.json({ message: 'PRIVATE_RAW_ERROR' }, { status: 401 })
      if (body.password === 'throttle') return new Response(null, { status: 429 })
      return Response.json(authResponse())
    }
    if (path.endsWith('/logout')) return new Response(null, { status: 204 })
    return new Response(null, { status: 503 })
  },
})
auth.start()
const ports = createProductionPorts(auth), router = createAppRouter(ports)
const root = createRoot(document.getElementById('root')!)
root.render(<RouterProvider router={router} />)
window.__AUTH_ROUTE__ = { state: () => ({ path: router.state.location.pathname, count, calls, readiness: auth.read().readiness }), settle: () => settle?.(), invalidate: () => policy.invalidate(), dispose: () => { root.unmount(); router.dispose(); ports.dispose?.(); auth.dispose() } }
declare global { interface Window { __AUTH_ROUTE__: { state(): { path: string; count: number; calls: string[]; readiness: string }; settle(): void; invalidate(): void; dispose(): void } } }
