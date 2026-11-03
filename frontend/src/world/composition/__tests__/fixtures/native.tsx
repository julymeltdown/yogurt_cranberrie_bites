// TEST ONLY: actual Phaser/canonical Auth/default shell with synthetic HTTP/WS and terrain.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { validate } from '../../../network/transport/contract'
import type { WorldRooms } from '../../../network/WorldTransport'
import { loadPhaserEngine } from '../../../scenes/MapScene'
import { createWorldComposition } from '../../index'
import { eventId, resolverFor, testData } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const nativeFetch = globalThis.fetch.bind(globalThis), data = await testData()
const requests: { path: string; method: string; cache: RequestCache; intent: string | null; hasBearer: boolean }[] = []
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async (input, init) => {
  const r = input instanceof Request ? input : new Request(input, init)
  requests.push({ path: new URL(r.url).pathname, method: r.method, cache: r.cache, intent: r.headers.get('X-MCR-Intent'), hasBearer: r.headers.has('Authorization') })
  return nativeFetch(input, init)
} })
auth.start()
await new Promise<void>(resolve => { const stop = auth.subscribe(() => { if (auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
if (!await auth.login('fixture@example.test', 'fixture-password')) throw new Error('Canonical test login failed')
const stats = { games: 0, peakGames: 0, mounts: 0, disposals: 0, closed: 0 }
const composition = createWorldComposition({ auth,
  resolveRelease: async request => {
    const rooms = await auth.request({ path: `/api/concert/events/${eventId}/rooms`, method: 'GET', access: 'protected', errorFamily: 'core', signal: request.signal,
      decode: value => validate<WorldRooms>('WorldRooms', value) })
    const selected = await resolverFor(auth, data, nativeFetch)()
    return { ...selected, privacyScope: request.privacyScope, bootstrap: request.bootstrap.document, rooms: rooms.data }
  },
  importEngine: async () => {
    const factory = await loadPhaserEngine()
    return (host, map) => {
      const engine = factory(host, map); stats.games++; stats.mounts++; stats.peakGames = Math.max(stats.peakGames, stats.games)
      void engine.closed.then(() => { stats.closed++ })
      let disposed = false
      return { ...engine, destroy() { if (disposed) return; disposed = true; engine.destroy(); stats.games--; stats.disposals++ } }
    }
  },
})
history.replaceState(null, '', '/world')
const router = createBrowserRouter(createAppRoutes({ ...createProductionPorts(auth), world: composition }))
const root = createRoot(document.getElementById('root')!)
const fixture = {
  state: () => ({ ...stats, world: composition.read(), replication: composition.replication.read(), requests,
    canvases: document.querySelectorAll('canvas').length, userId: auth.session.read().userId }),
  reconnect: composition.replication.reconnect,
  switchRoom: composition.replication.switchRoom,
  revoke() { auth.privacy.changePolicy(auth.privacy.capture(), 'suspended'); return fixture.state() },
  logout: () => auth.logout(),
  async login() { return auth.login('fixture@example.test', 'fixture-password') },
  navigate: (path: string) => router.navigate(path),
  dispose() { root.unmount(); router.dispose(); composition.dispose(); auth.dispose(); return fixture.state() },
}
Object.assign(window, { __WORLD_COMPOSITION_TEST_ONLY__: fixture })
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
declare global { interface Window { __WORLD_COMPOSITION_TEST_ONLY__: typeof fixture } }
