// TEST ONLY: actual App/Auth/World; controlled canonical HTTP/C05, no live claims.
import { MockSocket } from '../../../../world/network/mock'
import type { Snapshot } from '../../../../world/network/WorldTransport'
import original from '../map-presentation-v1/source-inputs/operator-bootstrap.json'
import { authResponse } from '../../../../features/auth/__tests__/fixtures/data'
import { policyFixtureRelease, servicePolicyFixture } from '../../../pre-world-policy/__tests__/fixtures'
await import('../map-roof-street-uptake/preview')
const fallbackFetch = window.fetch.bind(window)
const policyReads: { path: string; method: string; authorized: boolean; recognized: boolean; qualification: string }[] = []
window.fetch = async (input, init) => {
  const request = input instanceof Request ? new Request(input, init) : new Request(new URL(String(input), location.origin), init)
  const url = new URL(request.url)
  if (url.pathname === `/api/concert/events/${original.event.event_id}/service-policy`) {
    const authorized = request.headers.get('Authorization') === 'Bearer ' + authResponse().accessToken
    const recognized = request.method === 'GET' && authorized && url.searchParams.size === 2
      && url.searchParams.get('client_version') === policyFixtureRelease.applicationVersion
      && url.searchParams.get('client_release') === policyFixtureRelease.buildId
    policyReads.push({ path: url.pathname, method: request.method, authorized, recognized,
      qualification: 'CANONICAL_TEST_ONLY_RELEASE_AND_FRESH_POLICY_NOT_PRODUCTION_AUTHORITY' })
    return Response.json(servicePolicyFixture(recognized ? 'NORMAL' : 'UNAVAILABLE', { ttl: 30_000 }))
  }
  return fallbackFetch(request)
}
const Original = window.WebSocket as unknown as typeof MockSocket
const sockets: ControlledSocket[] = []
class ControlledSocket extends Original {
  private snapshot: Snapshot | null = null
  private sequence = 0
  constructor() { super(); sockets.push(this) }
  override receive(text: string) {
    const frame = JSON.parse(text) as { type: string }
    if (frame.type === 'snapshot') { this.snapshot = structuredClone(frame as Snapshot); this.sequence = 0 }
    super.receive(text)
  }
  override send(text: string) {
    super.send(text)
    const frame = JSON.parse(text) as { type: string; roomId: string; requestId: string }
    if (frame.type === 'switchRoom' && this.snapshot) {
      const baseline = this.snapshot
      queueMicrotask(() => { if (this.readyState === 1) this.receive(JSON.stringify({ ...baseline,
        cause: 'ROOM_SWITCH', requestId: frame.requestId, roomId: frame.roomId,
        streamId: crypto.randomUUID(), roomEpoch: crypto.randomUUID(), streamSeq: 0,
        accessExpiresAt: new Date(Date.now() + 240000).toISOString(),
      })) })
    }
  }
  removePerson(id: string) {
    const baseline = this.snapshot
    if (!baseline || id === baseline.selfUserId || !baseline.players.some(p => p.userId === id)) throw Error('Only an authorized remote fixture actor may leave')
    this.snapshot = { ...baseline, players: baseline.players.filter(p => p.userId !== id) }
    this.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'delta', streamId: baseline.streamId,
      roomEpoch: baseline.roomEpoch, streamSeq: ++this.sequence, serverTick: baseline.serverTick + this.sequence,
      ackInputSeq: 0, upserts: [], removes: [id] }))
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: ControlledSocket })
const controls = Object.freeze({ qualification: 'ACTUAL_APP_CONTROLLED_HTTP_C05_NOT_LIVE', policyState: () => structuredClone(policyReads), removePerson(id: string) { sockets.at(-1)?.removePerson(id) } })
Object.assign(window, { __PI11_ROOMS__: controls })
declare global { interface Window { __PI11_ROOMS__: typeof controls } }
