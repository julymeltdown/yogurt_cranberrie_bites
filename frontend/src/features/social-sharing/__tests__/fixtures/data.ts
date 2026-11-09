import approved from '../../../../../../contracts/fixtures/domain/approved-social-request.json'
import { createSessionController } from '../../../auth/model/session'
import { withAdmission } from '../../../auth/__tests__/fixtures/admission'
import { createSocialSharingController } from '../../model/controller'
import type { SocialAccess } from '../../model/runtime'
import type { SocialRequest } from '../../model/contract'
export { deferred } from '../../../../test-support/fixtures'
export const request = approved as SocialRequest
export const eventId = request.event_id, requesterId = request.requester_id, recipientId = request.recipient_id
export const epoch = Date.parse('2026-11-07T09:00:00Z')
export const id = (tail: number) => `00000000-0000-4000-8000-${String(tail).padStart(12, '0')}`
export const handleStates = (userId = requesterId) => ({ items: [{ user_id: userId, platform: 'INSTAGRAM', version: userId === requesterId ? 2 : 3, configured: true }, { user_id: userId, platform: 'X', version: 0, configured: false }] })
export const pending = (): SocialRequest => ({ ...structuredClone(request), state: 'PENDING', revision: 1, approved_platforms: [] })
export function setup(fetch: typeof globalThis.fetch, userId = requesterId, qualifiedRefresh = false) {
  const auth = createSessionController({ eventId, fetch: withAdmission(fetch), qualifiedRefresh })
  auth.session.replace(0, { userId, eventId, accessToken: 'social-test-only-token' })
  const listeners = new Set<() => void>(); let access: SocialAccess = { available: true, readOnly: false }, clock = epoch
  const accessPort = { read: () => access, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } } }
  const controller = createSocialSharingController({ auth, access: accessPort, sensitiveRead: auth.requestSensitive, now: () => clock })
  controller.start()
  return { auth, controller, access: accessPort, setAccess(next: SocialAccess) { access = next; listeners.forEach(fn => fn()) }, setNow(value: number) { clock = value; controller.expire() }, dispose() { controller.dispose(); auth.dispose() } }
}
