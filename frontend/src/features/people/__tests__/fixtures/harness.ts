import { QueryClient } from '@tanstack/react-query'
import source from '../../../../../../contracts/fixtures/domain/public-intro.json'
import { createMemorySession, createHttpClient } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createPeopleController } from '../../controller'
import type { SocialOptions } from '../../runtime'
export const eventId = source.event_id, userId = '00000000-0000-4000-8000-000000000003', introId = source.intro_id
export const publicIntro = () => structuredClone(source)
export function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes }); return { promise, resolve } }
export function harness() {
  const session = createMemorySession(); session.replace(0, { userId, eventId, accessToken: 'fixture-token' })
  const query = new QueryClient(), privacy = createQueryPrivacy({ client: query, session }), listeners = new Set<() => void>()
  let available = true, readOnly = false
  const requests: Request[] = []
  let respond: (request: Request) => Promise<Response> = async request => Response.json(new URL(request.url).pathname.endsWith('/intros') ? { items: [publicIntro()], next_cursor: null } : source.author)
  const http = createHttpClient({ session, fetch: async input => { const request = input as Request; requests.push(request); return respond(request) } })
  const options: SocialOptions = { auth: { privacy, request: http.request }, access: { read: () => ({ available, readOnly }), subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } } }
  const controller = createPeopleController(options)
  return { options, session, privacy, requests, controller, respond(value: typeof respond) { respond = value }, access(value: { available?: boolean; readOnly?: boolean }) { available = value.available ?? available; readOnly = value.readOnly ?? readOnly; listeners.forEach(fn => fn()) }, dispose() { controller.dispose(); privacy.dispose(); query.clear() } }
}
