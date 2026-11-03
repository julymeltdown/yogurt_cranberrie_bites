import { CAPABILITIES, projectCapabilities } from '../../../../app/capabilities'
import { createSessionController } from '../../../auth/model/session'
import { createPartyController, type PartyAccess } from '../../model/controller'
import { activity, application, ids, party } from './data'
export function harness(handle?: (request: Request) => Promise<Response> | Response, limit = 20) {
  const requests: Request[] = [], listeners = new Set<() => void>()
  const auth = createSessionController({ eventId: ids.event, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} },
    fetch: async input => {
      const request = input as Request; requests.push(request)
      if (handle) return handle(request)
      const path = new URL(request.url).pathname
      return Response.json(path.endsWith('/me/parties') ? activity() : path.endsWith('/applications') ? application() : path.endsWith('/parties') ? { items: [party()], next_cursor: null } : party())
    } })
  auth.session.replace(0, { userId: ids.actor, eventId: ids.event, accessToken: 'fixture-token-memory-only' })
  let access: PartyAccess | null = { scope: auth.privacy.capture(), phase: 'OPEN', capabilities: projectCapabilities(Object.fromEntries(CAPABILITIES.map(k => [k, { available: true, reason: null }]))) }
  let key = 80
  const controller = createPartyController({ auth, limit, access: { read: () => access, subscribe(fn) { listeners.add(fn); return () => { listeners.delete(fn) } } },
    newKey: () => `00000000-0000-4000-8000-${String(++key).padStart(12, '0')}` })
  return { auth, controller, requests, access: () => access, update(value: PartyAccess | null) { access = value; [...listeners].forEach(fn => fn()) },
    dispose() { controller.dispose(); auth.dispose() } }
}
export async function ready(h: ReturnType<typeof harness>) { await Promise.all([h.controller.loadList(), h.controller.loadActivity(), h.controller.loadDetail(ids.party)]) }
