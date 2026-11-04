// Explicitly synthetic Auth/admission/assets/anchor/eligibility. No product supplier.
import { createSessionController } from '../../../src/features/auth/model/session'
import { capabilities, authResponse, isolatedCookiePolicy } from '../../../src/features/auth/__tests__/fixtures/data'
import { createWorldMuralAuthority } from '../../../src/world/mural/authority'
import { MockSocket } from '../../../src/world/network/mock'
import type { WorldTicketRequest } from '../../../src/world/network/WorldTransport'
import { eventId, userId, testData, ticket, snapshot } from '../../../src/world/composition/__tests__/fixtures/data'
import type { MuralEntryQualification } from '../../../src/world/mural-entry-lifecycle'
import type { MuralQualificationContext, MuralQualificationFactory } from '../../../src/world/mural-entry-lifecycle/source'
export async function failureData(client: string) {
  const data = await testData(), requests: { path: string; method: string; aborted: boolean }[] = [], sockets: Socket[] = []
  let inFlight = 0, peakInFlight = 0
  const nativeFetch = globalThis.fetch.bind(globalThis)
  const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const request = input as Request, path = new URL(request.url).pathname
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
    if (path.endsWith('/rooms')) return Response.json(data.rooms)
    if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, sockets.length + 1))
    if (path.includes('/mural') || path.endsWith('/commands')) {
      const entry = { path, method: request.method, aborted: false }; requests.push(entry); inFlight++; peakInFlight = Math.max(peakInFlight, inFlight)
      try { const headers = new Headers(request.headers); headers.set('X-QA05-Client', client); return await nativeFetch(new Request(request, { headers })) }
      finally { inFlight--; entry.aborted = request.signal.aborted }
    }
    return new Response(null, { status: 404 })
  } })
  class Socket extends MockSocket {
    readonly expiry = new Date(Date.now() + 600000).toISOString()
    constructor() { super(); sockets.push(this); queueMicrotask(() => { if (this.readyState === 0) this.open() }) }
    override send(text: string) {
      super.send(text)
      const message = JSON.parse(text)
      if (message.type === 'authenticate') queueMicrotask(() => { if (this.readyState === 1) this.receive(JSON.stringify(snapshot(message.roomId, undefined, { accessExpiresAt: this.expiry }))) })
    }
  }
  let context: MuralQualificationContext | null = null, qualification: MuralEntryQualification | null = null, first = true
  let factoryCalls = 0, sourceStops = 0, lifetime = new AbortController()
  const listeners = new Set<() => void>()
  function qualify() {
    const view = context?.world.read(), baseline = view?.baseline
    if (!context || view?.phase !== 'live' || !view.visible || !baseline) return false
    if (lifetime.signal.aborted) lifetime = new AbortController()
    qualification = Object.freeze({ scope: context.scope, signal: AbortSignal.any([context.signal, lifetime.signal]),
      binding: Object.freeze({ eventId: baseline.eventId, roomId: baseline.roomId, roomEpoch: baseline.roomEpoch, streamId: baseline.streamId }),
      authority: createWorldMuralAuthority({ privacy: auth.privacy, world: context.world, writable: () => true }), scene: context.scene,
      anchor: Object.freeze({ x: 16, y: 24, width: 128, height: 128, depth: 2 }), textureKey: 'qa05-synthetic-wall' })
    for (const listener of Array.from(listeners)) listener()
    return true
  }
  const factory: MuralQualificationFactory = input => {
    context = input; factoryCalls++
    return { read: () => qualification, subscribe(listener) {
      listeners.add(listener)
      const stop = input.world.subscribe(() => { if (first && input.world.read()?.phase === 'live') { first = false; qualify() } })
      return () => { listeners.delete(listener); stop(); sourceStops++ }
    } }
  }
  return { auth, data, sockets, Socket, factory, qualify, requests, context: () => context,
    stats: () => ({ factoryCalls, sourceStops, sourceListeners: listeners.size, inFlight, peakInFlight, requestCount: requests.length }),
    withdraw() { lifetime.abort() },
    async login() {
      auth.start()
      for (let n = 0; n < 100 && !auth.read().emailEnabled; n++) await new Promise(resolve => setTimeout(resolve, 0))
      if (!await auth.login('synthetic-qa05@example.test', 'fixture-password')) throw Error('Synthetic login failed')
      if (auth.session.read().userId !== userId) throw Error('Canonical fixture identity differs')
    }, dispose() { lifetime.abort(); auth.dispose() },
  }
}
