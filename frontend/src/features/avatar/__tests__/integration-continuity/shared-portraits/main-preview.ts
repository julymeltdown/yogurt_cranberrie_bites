// TEST ONLY: normal main.tsx and current providers; controlled HTTP/C05, no live service claim.
import { preset } from '../../reviewed-production/fixtures'
import { admission } from '../../../../auth/__tests__/enrollment-consent/fixtures'
import type { MockSocket } from '../../../../../world/network/mock'
await import('../../../../../app/character-entry/__tests__/reviewed-production/default-entry/preview')
// Current production readiness requires the separate C05 policy receipt. Emit it
// as a controlled server frame through the retained actual transport, never a grant
// injected into the client provider or readiness state.
const RetainedSocket = window.WebSocket as unknown as typeof MockSocket
class PolicySocket extends RetainedSocket {
  override receive(text: string) {
    super.receive(text)
    const frame = JSON.parse(text) as { type: string; streamId?: string; eventId?: string }
    if (frame.type === 'snapshot') super.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'policy.baseline',
      streamId: frame.streamId, eventId: frame.eventId, scope: 'SOCIAL', policyRevision: 1 }))
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: PolicySocket })
const retainedFetch = window.fetch.bind(window)
let denied = false
const calls: { path: string; method: string; status: number }[] = []
function bindOwner(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(bindOwner)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, key === 'avatar' ? { revision: 12, config: preset(1) } : bindOwner(item)]))
  return value
}
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  const url = new URL(request.url), path = url.pathname
  if (path === '/api/auth/enrollment/admission') return Response.json(admission())
  const shared = /\/(feed|posts|giveaways|parties|people)(\/|$)/.test(path) && path.startsWith('/api/concert/')
  if (!shared) return retainedFetch(request)
  if (denied && /\/giveaways\//.test(path)) {
    calls.push({ path, method: request.method, status: 404 })
    return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'Unavailable to this viewer', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
  }
  let response: Response
  if (request.method === 'GET' && /\/(giveaways|parties)\/[0-9a-f-]{36}$/.test(path)) {
    const id = path.split('/').at(-1)
    url.pathname = path.slice(0, path.lastIndexOf('/'))
    const list = await retainedFetch(new Request(url, request)), data = await list.json() as { items?: { giveaway_id?: string; party_id?: string }[] }
    const item = data.items?.find(item => item.giveaway_id === id || item.party_id === id)
    response = item ? Response.json(item) : Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'No controlled resource', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
  } else response = await retainedFetch(request)
  calls.push({ path, method: request.method, status: response.status })
  return Response.json(bindOwner(await response.json()), { status: response.status })
}
const sharedPortraits = { denyGift() { denied = true }, state: () => ({ calls: structuredClone(calls), owner: { revision: 12, config: preset(1) }, denied }) }
Object.assign(window, { __SHARED_PORTRAITS__: sharedPortraits })
declare global { interface Window { __SHARED_PORTRAITS__: typeof sharedPortraits } }
