import { parse, freeze } from '../../event-feed/contract'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import type { Decoder, SessionScope } from '../../../shared/api/http'
export { freeze }
export const uuid = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
export interface NotificationTarget {
  readonly target_type: 'PARTY' | 'APPLICATION' | 'GIVEAWAY' | 'RESERVATION' | 'SOCIAL_REQUEST' | 'POST' | 'NOTICE' | 'REPORT'
  readonly target_id: string
}
export interface InboxNotification {
  readonly notification_id: string; readonly event_id: string
  readonly kind: 'PARTY' | 'GIVEAWAY' | 'SOCIAL' | 'MODERATION' | 'NOTICE' | 'FEED'
  readonly target: NotificationTarget | null; readonly created_at: string; readonly read_at: string | null; readonly revision: number
}
export interface NotificationPage { readonly items: readonly InboxNotification[]; readonly next_cursor: string | null }
export interface ReadAllResult { readonly read_count: number; readonly read_at: string }
export interface UnreadCount { readonly actor_id: string; readonly event_id: string; readonly unread_total: number; readonly as_of: string }
export function unreadCount(value: unknown, scope: SessionScope): UnreadCount {
  const count = parse<UnreadCount>('Domain_feed_NotificationUnreadCount', value)
  if (count.actor_id !== scope.userId || count.event_id !== scope.eventId) fail()
  return count
}
export type NotificationOperation = 'markConcertNotification' | 'markAllConcertNotifications'
export interface CommandBinding {
  readonly scope: SessionScope; readonly operation: NotificationOperation; readonly key: string
  readonly notificationId: string | null; readonly revision: number | null
}
export interface Receipt {
  readonly receipt_id: string; readonly actor_id: string; readonly event_id: string | null
  readonly operation_id: string; readonly idempotency_key: string; readonly state: 'PENDING' | 'SUCCEEDED' | 'FAILED' | 'RETIRED'
  readonly resource_id: string | null; readonly resource_revision: number | null; readonly result_status: number | null
  readonly failure_reason: string | null; readonly created_at: string; readonly expires_at: string
}
const fail = (): never => { throw new Error('Invalid inbox response') }
export const prefix = (s: SessionScope) => { if (!uuid(s.userId) || !uuid(s.eventId)) fail(); return `/api/concert/events/${s.eventId}` }
export function notification(value: unknown, scope: SessionScope, id?: string): InboxNotification {
  const n = parse<InboxNotification>('Domain_feed_Notification', value)
  if (n.event_id !== scope.eventId || (id !== undefined && n.notification_id !== id) || (n.read_at !== null && Date.parse(n.read_at) < Date.parse(n.created_at))) fail()
  return n
}
export function page(value: unknown, scope: SessionScope, unreadOnly: boolean): NotificationPage {
  const p = parse<NotificationPage>('Domain_feed_NotificationList', value)
  if (p.items.length > 20) fail()
  p.items.forEach(n => { notification(n, scope); if (unreadOnly && n.read_at !== null) fail() })
  return p
}
export function receipt(value: unknown, b: CommandBinding): Receipt {
  const r = parse<Receipt>('Domain_common_CommandReceipt', value)
  if (r.actor_id !== b.scope.userId || r.event_id !== b.scope.eventId || r.operation_id !== b.operation || r.idempotency_key !== b.key
    || Date.parse(r.created_at) >= Date.parse(r.expires_at) || (b.notificationId && r.resource_id !== null && r.resource_id !== b.notificationId)) fail()
  if (r.state === 'PENDING' || r.state === 'RETIRED') {
    if (r.resource_id !== null || r.resource_revision !== null || r.result_status !== null || r.failure_reason !== null) fail()
  } else if (r.state === 'SUCCEEDED') {
    if (r.result_status !== 200 || r.failure_reason !== null || (b.notificationId && (r.resource_id !== b.notificationId || r.resource_revision === null || r.resource_revision < b.revision!))
      || (!b.notificationId && (r.resource_id !== null || r.resource_revision !== null))) fail()
  } else if (r.result_status !== null || !r.failure_reason || !(Object.values(DOMAIN_OPERATIONS[b.operation].errors).flat() as readonly string[]).includes(r.failure_reason)) fail()
  return r
}
export function decoder<T>(scope: SessionScope, path: string, read: (v: unknown, status: number) => T): Decoder<T> {
  return (v, c) => {
    if (c.path !== path || c.scope.userId !== scope.userId || c.scope.eventId !== scope.eventId || c.scope.generation !== scope.generation) fail()
    return read(v, c.status)
  }
}
export const readAllResult = (v: unknown) => parse<ReadAllResult>('Domain_feed_ReadAllResult', v)
export function combine(previous: readonly InboxNotification[], next: readonly InboxNotification[]): readonly InboxNotification[] {
  const items = new Map(previous.map(n => [n.notification_id, n]))
  for (const n of next) {
    const old = items.get(n.notification_id)
    if (old && (old.kind !== n.kind || old.created_at !== n.created_at || old.event_id !== n.event_id)) fail()
    if (!old || n.revision > old.revision) items.set(n.notification_id, n)
    else if (n.revision === old.revision && JSON.stringify(old) !== JSON.stringify(n)) fail()
  }
  const orderTime = (a: string, b: string) => {
    const seconds = a.slice(0, 19).localeCompare(b.slice(0, 19))
    if (seconds) return seconds
    const fraction = (v: string) => v.slice(19, -1).replace(/^\./, '')
    const left = fraction(a), right = fraction(b), length = Math.max(left.length, right.length)
    return left.padEnd(length, '0').localeCompare(right.padEnd(length, '0'))
  }
  return freeze([...items.values()].sort((a, b) => orderTime(b.created_at, a.created_at) || b.notification_id.localeCompare(a.notification_id)))
}
