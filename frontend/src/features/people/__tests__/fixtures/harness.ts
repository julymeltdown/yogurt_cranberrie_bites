import source from '../../../../../../contracts/fixtures/domain/public-intro.json'
import { createTransportHarness } from '../../../../test-support/harness'
import { deferred } from '../../../../test-support/fixtures'
import { createPeopleController } from '../../controller'
import type { SocialOptions } from '../../runtime'
export const eventId = source.event_id, userId = '00000000-0000-4000-8000-000000000003', introId = source.intro_id
export const publicIntro = () => structuredClone(source)
export { deferred }
export function harness() {
  const transport = createTransportHarness({
    userId,
    eventId,
    respond: async request => Response.json(new URL(request.url).pathname.endsWith('/intros') ? { items: [publicIntro()], next_cursor: null } : source.author),
  })
  const listeners = new Set<() => void>()
  let available = true, readOnly = false
  const options: SocialOptions = { auth: { privacy: transport.privacy, request: transport.http.request }, access: { read: () => ({ available, readOnly }), subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } } }
  const controller = createPeopleController(options)
  return { options, session: transport.session, privacy: transport.privacy, requests: transport.requests, controller, respond: transport.respond, access(value: { available?: boolean; readOnly?: boolean }) { available = value.available ?? available; readOnly = value.readOnly ?? readOnly; listeners.forEach(fn => fn()) }, dispose() { controller.dispose(); transport.dispose() } }
}
