import { z } from 'zod'
import type { Decoder } from '../../shared/api/http'
import type { PolicyAttempt, PreWorldDecision } from './model'

/** Literal public INPUT source. Generated registry/runtime qualification is separate. */
export const servicePolicyContractRevision = '7c563cf09fe971b904ac9549f5e3e8fe1391d94a'
export const servicePolicyOperation = 'getConcertServicePolicy'
export const servicePolicyPath = (eventId: string) => `/api/concert/events/${eventId}/service-policy`
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
export function policyVersionTuple(value: unknown): readonly [number, number, number] | null {
  if (typeof value !== 'string' || value.length > 17 || /[^0-9.]/.test(value)
    || !/^(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})$/.test(value)) return null
  const parts = value.split('.').map(Number)
  if (parts.some(part => part > 65535)) return null
  return Object.freeze([parts[0]!, parts[1]!, parts[2]!])
}
const version = z.string().refine(value => policyVersionTuple(value) !== null)
const utc = z.iso.datetime().refine(value => Number.isFinite(Date.parse(value))
  && new Date(Date.parse(value)).toISOString().slice(0, 19) === value.slice(0, 19))
const fields = { event_revision: revision, policy_revision: revision, minimum_client_version: version,
  server_time: utc, valid_until: utc }
export const servicePolicySchema = z.discriminatedUnion('kind', [
  z.strictObject({ ...fields, kind: z.literal('NORMAL') }),
  z.strictObject({ ...fields, kind: z.literal('MAINTENANCE') }),
  z.strictObject({ ...fields, kind: z.literal('CLIENT_VERSION_REQUIRED') }),
  z.strictObject({ ...fields, kind: z.literal('UNAVAILABLE'), policy_revision: revision.nullable(), minimum_client_version: version.nullable() }),
]).refine(value => {
  const ttl = Date.parse(value.valid_until) - Date.parse(value.server_time)
  return ttl > 0 && ttl <= 30_000
})
function compare(a: readonly number[], b: readonly number[]) {
  for (let i = 0; i < 3; i++) { const difference = a[i]! - b[i]!; if (difference !== 0) return difference }
  return 0
}
function conservativeServerTime(value: string) {
  // RFC3339 may carry finer precision than Date.parse. Round the start upward
  // and the end downward so truncation can never extend the authority window.
  const subMilliseconds = /\.\d{3}([0-9]+)Z$/.exec(value)?.[1]
  return Date.parse(value) + (subMilliseconds && /[1-9]/.test(subMilliseconds) ? 1 : 0)
}
export function servicePolicyDecoder(binding: PolicyAttempt, isCurrent: () => boolean): Decoder<PreWorldDecision> {
  const expected = Object.freeze({ ...binding.scope })
  const clientVersion = policyVersionTuple(binding.release.applicationVersion)
  return (value, context) => {
    if (!isCurrent() || !clientVersion || context.status !== 200 || context.path !== servicePolicyPath(expected.eventId)
      || context.scope.userId !== expected.userId || context.scope.eventId !== expected.eventId
      || context.scope.generation !== expected.generation) throw Error('Service policy scope unavailable')
    const wire = servicePolicySchema.parse(value)
    if (wire.kind === 'NORMAL' || wire.kind === 'CLIENT_VERSION_REQUIRED') {
      const difference = compare(clientVersion, policyVersionTuple(wire.minimum_client_version)!)
      if (wire.kind === 'NORMAL' ? difference < 0 : difference >= 0) throw Error('Invalid service version decision')
    }
    return Object.freeze({ kind: wire.kind === 'NORMAL' ? 'normal' : wire.kind === 'MAINTENANCE' ? 'maintenance'
      : wire.kind === 'CLIENT_VERSION_REQUIRED' ? 'version' : 'unavailable',
    eventRevision: wire.event_revision, policyRevision: wire.policy_revision, minimumClientVersion: wire.minimum_client_version,
    serverTimeMs: conservativeServerTime(wire.server_time), validUntilMs: Date.parse(wire.valid_until) })
  }
}
