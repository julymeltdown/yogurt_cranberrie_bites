import postFixture from '../../../../../contracts/fixtures/domain/public-post.json'
import { createSessionController } from '../../auth/model/session'
import { withAdmission } from '../../auth/__tests__/fixtures/admission'
import { createEventFeedController } from '../controller'
import type { FeedAccess } from '../runtime'
import type { Post } from '../contract'
export { deferred } from '../../../test-support/fixtures'
export const post = postFixture as Post
export const eventId = post.event_id, userId = post.author.user_id
export const id = (tail: number) => `00000000-0000-4000-8000-${String(tail).padStart(12, '0')}`
export const notice = { notice_id: id(60), event_id: eventId, title: 'Fixture notice', text: 'Test-only published notice.', source_uri: 'https://example.com/notice', confirmed_at: '2026-11-01T01:00:00Z', created_at: '2026-11-01T01:00:00Z', state: 'PUBLISHED', revision: 1 }
export function setup(fetch: typeof globalThis.fetch) {
  const auth = createSessionController({ eventId, fetch: withAdmission(fetch) })
  auth.session.replace(0, { eventId, userId, accessToken: 'test-only-token' })
  let access: FeedAccess = { available: true, readOnly: false }
  const subscribers = new Set<() => void>()
  const accessPort = { read: () => access, subscribe(fn: () => void) { subscribers.add(fn); return () => { subscribers.delete(fn) } } }
  const controller = createEventFeedController({ auth, access: accessPort })
  return { auth, controller, access: accessPort, setAccess(next: FeedAccess) { access = next; subscribers.forEach(fn => fn()) }, dispose() { controller.dispose(); auth.dispose() } }
}
