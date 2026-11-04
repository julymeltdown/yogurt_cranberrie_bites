// @vitest-environment jsdom
// Test infrastructure adapted from composition/__tests__/composition.test.ts at ff96d53e.
// This is the real release/map/transport/controller seam with an isolated engine port.
import { afterEach, expect, it, vi } from 'vitest'
import { createSessionController } from '../../../../features/auth/model/session'
import { authResponse, capabilities, eventId, isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { createWorldComposition } from '../../../composition'
import { currentScope, project, resolverFor, snapshot, testData, ticket } from '../../../composition/__tests__/fixtures/data'
import { MockSocket } from '../../mock'
import type { EngineFactory } from '../../../runtime'
import type { WorldCamera } from '../../../camera/port'
import type { WorldTicketRequest } from '../../WorldTransport'
import negotiated from '../../../../../../contracts/fixtures/realtime/motion-v1/negotiated.json'

vi.mock('../../../map/resources', async original => {
  const source = await original<typeof import('../../../map/resources')>()
  return { ...source, loadMap: (input: Parameters<typeof source.loadMap>[0], signal: AbortSignal) => source.loadMap({ ...input,
    decode: async () => { const image = new Image(); Object.defineProperties(image, { naturalWidth: { value: 96 }, naturalHeight: { value: 32 } }); return { image, dispose() {} } },
  }, signal) }
})
const dispose: (() => void)[] = []
afterEach(() => { dispose.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals(); document.body.replaceChildren() })
for (const optin of [false, true]) it(`actual composition qualifies one map and forwards matched optin/context: ${optin}`, async () => {
  const data = await testData(), sockets: MockSocket[] = []
  let issued = 0, engines = 0
  const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const request = input as Request, path = new URL(request.url).pathname
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.clone().json() as WorldTicketRequest, ++issued))
    return new Response(null, { status: 204 })
  } })
  dispose.push(() => auth.dispose())
  auth.start(); await vi.waitFor(() => expect(auth.read().emailEnabled).toBe(true))
  expect(await auth.login('fixture@example.test', 'fixture-password')).toBe(true)
  class Socket extends MockSocket {
    constructor() { super(); sockets.push(this); queueMicrotask(() => { if (this.readyState === 0) this.open() }) }
    override send(text: string) {
      super.send(text); const message = JSON.parse(text)
      if (message.type !== 'authenticate') return
      const frame = snapshot(message.roomId)
      queueMicrotask(() => this.receive(JSON.stringify(optin ? { ...frame,
        motionProfile: { ...negotiated.messages[1]!.message.motionProfile, geometryHash: data.map.geometry_hash },
        selfMotion: { serverTick: 0, position: frame.players[0]!.position, consumedInputSeq: 0, inputAppliedTicks: 0, droppedTicks: 0, heldInput: null },
      } : frame)))
    }
  }
  vi.stubGlobal('WebSocket', Socket)
  const engine: EngineFactory = host => {
    engines++; const canvas = document.createElement('canvas'); host.append(canvas)
    const camera: WorldCamera = { surface: () => canvas, center() {}, project: point => point, unproject: point => point,
      read: () => ({ zoom: 1, scrollX: 0, scrollY: 0, width: 390, height: 400 }) }
    let destroyed = false
    return { ready: Promise.resolve(), closed: Promise.resolve(), camera, resize() {}, setVisible() {},
      destroy() { if (!destroyed) { destroyed = true; engines--; canvas.remove() } } }
  }
  const composition = createWorldComposition({ auth, resolveRelease: resolverFor(auth, data), importEngine: async () => engine,
    ...(optin ? { motionProfileVersion: '1.0.0' as const } : {}) })
  dispose.push(() => composition.dispose())
  const host = document.createElement('div'); document.body.append(host)
  composition.mount(host, { scope: currentScope(auth), privacyScope: auth.privacy.capture(), bootstrap: await project(auth, data), signal: new AbortController().signal })
  await vi.waitFor(() => expect(composition.replication.read()?.phase, JSON.stringify(composition.read())).toBe('live'), { timeout: 5000 })
  expect(sockets).toHaveLength(1); expect(engines).toBe(1); expect(issued).toBe(1)
  const authenticate = JSON.parse(sockets[0]!.sent[0]!)
  if (optin) expect(authenticate.motionProfileVersion).toBe('1.0.0')
  else expect(authenticate).not.toHaveProperty('motionProfileVersion')
  expect(composition.replication.read()!.spatialPrediction).toBe(optin ? 'bounded' : 'unavailable')
  composition.dispose(); await composition.settled(); expect(engines).toBe(0)
  expect([...sockets[0]!.listeners.values()].every(listeners => listeners.size === 0)).toBe(true)
})
