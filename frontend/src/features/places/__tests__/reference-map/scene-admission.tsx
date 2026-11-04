// TEST ONLY actual canonical Auth/bootstrap/root rejection. No candidate art load.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createProductionPorts } from '../../../../app/ports'
import { createAppRoutes } from '../../../../app/router'
import { createSessionController } from '../../../auth/model/session'
import { isolatedCookiePolicy, capabilities, authResponse } from '../../../auth/__tests__/fixtures/data'
import { bundle, seal, eventId } from '../../../../app/world-entry/__tests__/data'
import { referenceMapRelease } from './scene-data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'
const requests: string[] = []
let denied = false
let denial = ''
const b = await bundle()
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
  const path = new URL((input as Request).url).pathname; requests.push(path)
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json(authResponse())
  if (path.endsWith('/bootstrap')) return Response.json(b)
  return new Response(null, { status: 404 })
} })
auth.start(); while (auth.read().capabilities === 'loading') await new Promise(resolve => setTimeout(resolve, 5))
if (!await auth.login('fixture@example.test', 'fixture-password')) throw new Error('Test login failed')
const original = createProductionPorts(auth), active = auth.session.read()
await original.bootstrap.load({ generation: active.generation, userId: active.userId!, eventId: active.eventId! }, new AbortController().signal)
const baselineAccepted = true
const candidate = await referenceMapRelease(auth.session.read())
b.map_manifest = candidate.manifest as typeof b.map_manifest
b.theme_manifests = b.theme_manifests.filter(t => t.theme_id === 'PARADE'); await seal(b)
const ports = { ...original, bootstrap: {
  async load(...args: Parameters<typeof original.bootstrap.load>) {
    try { return await original.bootstrap.load(...args) } catch (error) { denied = true; denial = String(error); throw error }
  },
} }
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/world/map'] })
const root = createRoot(document.getElementById('root')!);root.render(<RouterProvider router={router} />)
Object.assign(window, { __SCENE_MAP__: { state: () => ({ production: true, baselineAccepted, denied, denial, requests: [...requests] }),
  dispose() { root.unmount(); router.dispose(); original.dispose?.(); auth.dispose() } } })
