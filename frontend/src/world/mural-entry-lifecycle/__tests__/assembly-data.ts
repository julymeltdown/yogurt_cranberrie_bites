import { createSessionController } from '../../../features/auth/model/session'
import { capabilities, authResponse, isolatedCookiePolicy } from '../../../features/auth/__tests__/fixtures/data'
import { serverFixture, ids } from '../../../features/mural/sync/__tests__/fixture'
import { createWorldMuralAuthority } from '../../mural/authority'
import { MockSocket } from '../../network/mock'
import type { WorldTicketRequest } from '../../network/WorldTransport'
import { eventId, userId, testData, ticket, snapshot } from '../../composition/__tests__/fixtures/data'
import type { MuralEntryQualification } from '../index'
import type { MuralQualificationContext, MuralQualificationFactory } from '../source'

/** Only fixture IDs are translated to the existing world fixture's canonical identity.
 * The actual entry/HTTP decoders still consume closed C04 envelopes.
 */
export async function assemblyData() {
  const data = await testData(), server = serverFixture(), requests: Request[] = [], sockets: Socket[] = []
  const translate = (value: unknown) => JSON.parse(JSON.stringify(value).replaceAll(ids.event, eventId).replaceAll(ids.user, userId)) as unknown
  const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const request = input as Request, path = new URL(request.url).pathname; requests.push(request.clone())
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
    if (path.endsWith('/rooms')) return Response.json(data.rooms)
    if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, requests.length))
    if (path.includes('/mural') || path.endsWith('/commands')) {
      const headers = new Headers(request.headers); headers.set('Authorization', `Bearer ${ids.user}`)
      const response = await server.fetch(new Request(request, { headers }))
      return Response.json(translate(await response.json()), { status: response.status })
    }
    return new Response(null, { status: 404 })
  } })
  class Socket extends MockSocket {
    readonly expiry = new Date(Date.now() + 60000).toISOString()
    constructor() { super(); sockets.push(this); queueMicrotask(() => { if (this.readyState === 0) this.open() }) }
    override send(text: string) {
      super.send(text); const message = JSON.parse(text)
      if (message.type === 'authenticate') queueMicrotask(() => { if (this.readyState === 1) this.receive(JSON.stringify(snapshot(message.roomId, undefined, { accessExpiresAt: this.expiry }))) })
    }
  }
  let context: MuralQualificationContext | null = null, qualification: MuralEntryQualification | null = null
  let first = true, writable = true, factoryCalls = 0, sourceStops = 0
  const listeners = new Set<() => void>()
  let lifetime = new AbortController()
  function qualify() {
    const view = context?.world.read(), baseline = view?.baseline
    if (!context || view?.phase !== 'live' || !baseline) return false
    if (lifetime.signal.aborted) lifetime = new AbortController()
    const authority = createWorldMuralAuthority({ privacy: auth.privacy, world: context.world, writable: () => writable })
    qualification = Object.freeze({ scope: context.scope, signal: AbortSignal.any([context.signal, lifetime.signal]),
      binding: Object.freeze({ eventId: baseline.eventId, roomId: baseline.roomId, roomEpoch: baseline.roomEpoch, streamId: baseline.streamId }),
      authority, scene: context.scene, anchor: Object.freeze({ x: 16, y: 24, width: 128, height: 128, depth: 2 }), textureKey: 'fixture-caller-wall' })
    for (const listener of Array.from(listeners)) listener()
    return true
  }
  const factory: MuralQualificationFactory = input => {
    context = input; factoryCalls++
    return { read: () => qualification,
      subscribe(listener) {
        listeners.add(listener)
        const stop = input.world.subscribe(() => { if (first && input.world.read()?.phase === 'live') { first = false; qualify() } })
        return () => { listeners.delete(listener); stop(); sourceStops++ }
      } }
  }
  return { auth, data, server, requests, sockets, Socket, factory, qualify, context: () => context,
    stats: () => ({ factoryCalls, sourceStops, sourceListeners: listeners.size }),
    revoke() { lifetime.abort() }, setWritable(value: boolean) { writable = value; for (const listener of Array.from(listeners)) listener() },
    async login() {
      auth.start()
      for (let n = 0; n < 100 && !auth.read().emailEnabled; n++) await new Promise(resolve => setTimeout(resolve, 0))
      if (!await auth.login('fixture@example.test', 'fixture-password')) throw Error('Fixture login failed')
    },
    dispose() { lifetime.abort(); auth.dispose() },
  }
}
