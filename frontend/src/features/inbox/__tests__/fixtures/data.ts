import { vi } from 'vitest'
import { createSessionController } from '../../../auth/model/session'
import { isolatedCookiePolicy } from '../../../auth/__tests__/fixtures/data'
import { createInboxController } from '../../model/controller'
import type { InboxNotification, Receipt } from '../../model/contract'
export const uid = (n: number) => `00000000-0000-4000-8000-${n.toString().padStart(12, '0')}`
export const user = uid(1), event = uid(2), id = uid(3), targetId = uid(4)
export const stamp = '2026-11-04T00:00:00Z', later = '2026-11-04T00:01:00Z'
export const update = (patch: Partial<InboxNotification> = {}): InboxNotification => ({ notification_id: id, event_id: event, kind: 'PARTY', target: { target_type: 'PARTY', target_id: targetId }, created_at: stamp, read_at: null, revision: 1, ...patch })
export const page = (items: readonly InboxNotification[] = [update()], next_cursor: string | null = null) => ({ items, next_cursor })
export const error = (status: number, reason: string) => Response.json({ code: reason, message: 'Raw detail must not render', details: { reason } }, { status })
export const pending = (operation: string, key: string, patch: Partial<Receipt> = {}): Receipt => ({ receipt_id: uid(8), actor_id: user, event_id: event, operation_id: operation,
  idempotency_key: key, state: 'PENDING', resource_id: null, resource_revision: null, result_status: null, failure_reason: null,
  created_at: stamp, expires_at: '2026-11-05T00:00:00Z', ...patch })
export function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
export function setup(handler: (request: Request) => Response | Promise<Response> = () => Response.json(page()), available = true) {
  const fetch = vi.fn<typeof globalThis.fetch>(async r => handler(r as Request))
  const auth = createSessionController({ eventId: event, fetch, cookiePolicy: isolatedCookiePolicy() })
  auth.session.replace(0, { userId: user, eventId: event, accessToken: 'memory-only-token' })
  const listeners = new Set<() => void>()
  let enabled = available, key = 100
  const navigate = vi.fn()
  const access = { read: () => enabled, subscribe(fn: () => void) { listeners.add(fn); return () => listeners.delete(fn) } }
  const controller = createInboxController({ auth, access, navigate, now: () => Date.parse(stamp), key: () => uid(key++) })
  return { auth, controller, fetch, navigate, setAvailable(value: boolean) { enabled = value; listeners.forEach(fn => fn()) },
    dispose() { controller.dispose(); auth.dispose() } }
}
