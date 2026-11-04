import { z } from 'zod'
import { canonicalUuid } from '../../features/auth/model/contract'
import type { DecodeContext } from '../../shared/api/http'
import type { PrivacyScope } from '../../shared/query/privacy'

// C04 source3a089a3b: admin.OwnOperationsAccessV1 / OperationsAccessLeaseV1.
// This is a consumer decoder, not a new wire contract or an authority issuer.
const uuid = z.string().refine(canonicalUuid)
const utc = z.iso.datetime().refine(value => Number.isFinite(Date.parse(value))
  && new Date(value).toISOString().slice(0, 19) === value.slice(0, 19))
const lease = z.strictObject({ actor_id: uuid, event_id: uuid,
  role: z.enum(['MODERATOR', 'OPERATOR']), role_revision: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER),
  decision_id: uuid, checked_at: utc, expires_at: utc })
const response = z.strictObject({ server_time: utc, access: lease.nullable() })
function boundedLifetime(checked: string, expires: string) {
  const seconds = Date.parse(expires.slice(0, 19) + 'Z') - Date.parse(checked.slice(0, 19) + 'Z')
  const fraction = (value: string) => value.includes('.') ? value.slice(value.indexOf('.') + 1, -1) : ''
  const a = fraction(checked), b = fraction(expires), length = Math.max(a.length, b.length)
  const left = a.padEnd(length, '0'), right = b.padEnd(length, '0')
  return seconds === 0 ? right > left : seconds > 0 && (seconds < 60_000 || seconds === 60_000 && right <= left)
}
export type OperationsAccessResponse = z.infer<typeof response>
export const OPERATIONS_ACCESS_ID = 'getOwnConcertOperationsAccess'
export const operationsAccessPath = (event: string) => `/api/concert/events/${event}/me/operations-access`
export const sameOperationsScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
export const validOperationsScope = (scope: PrivacyScope) => canonicalUuid(scope.userId) && canonicalUuid(scope.eventId)
  && Number.isSafeInteger(scope.generation) && scope.generation >= 0
  && Number.isSafeInteger(scope.policyGeneration) && scope.policyGeneration >= 0 && scope.gate === 'open'

export function decodeOperationsAccess(value: unknown, context: DecodeContext, scope: PrivacyScope): OperationsAccessResponse {
  if (!validOperationsScope(scope) || context.status !== 200 || context.path !== operationsAccessPath(scope.eventId!)
    || context.scope.userId !== scope.userId || context.scope.eventId !== scope.eventId || context.scope.generation !== scope.generation) {
    throw Error('Unbound operations access')
  }
  const parsed = response.parse(value), access = parsed.access
  if (access && (access.actor_id !== scope.userId || access.event_id !== scope.eventId
    || access.checked_at !== parsed.server_time || !boundedLifetime(access.checked_at, access.expires_at))) throw Error('Invalid operations lease')
  return parsed
}
