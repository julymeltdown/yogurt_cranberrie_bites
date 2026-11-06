import { schema } from '../../../generated/api-client/runtime'
import type { Schemas } from '../../../generated/api-client/types'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import { freeze, utc, uuid } from '../event/model/contract'

export type PolicyDecision = Schemas['Domain_admin_ServicePolicyDecision']
export type PolicyInput = Schemas['Domain_admin_ServicePolicyInput']
export type PolicyReceipt = Schemas['Domain_common_CommandReceipt']
const decisionSchema = schema('Domain_admin_ServicePolicyDecision')
const inputSchema = schema('Domain_admin_ServicePolicyInput')
const receiptSchema = schema('Domain_common_CommandReceipt')
export const operation = 'setConcertServicePolicy' as const

export function policyInput(value: unknown, now: number): PolicyInput {
  const input = inputSchema.parse(value)
  if (new TextEncoder().encode(input.reason).length > 500) throw Error('Reason exceeds byte limit')
  if (input.change.action === 'PUBLISH') {
    const expiry = Date.parse(utc.parse(input.change.expires_at))
    if (expiry <= now || expiry > now + 86_400_000) throw Error('Invalid publication lifetime')
  }
  return freeze(structuredClone(input))
}
export function policyDecision(value: unknown, now: number): PolicyDecision {
  const decision = decisionSchema.parse(value)
  const start = Date.parse(utc.parse(decision.server_time)), end = Date.parse(utc.parse(decision.valid_until))
  if (start > now || end <= now || end <= start || end > start + 30_000) throw Error('Stale policy decision')
  return freeze(structuredClone(decision))
}
export function policyReceipt(value: unknown, binding: { actor: string; event: string; key: string; eventRevision: number }, now: number): PolicyReceipt {
  const receipt = receiptSchema.parse(value)
  uuid.parse(receipt.receipt_id); uuid.parse(receipt.idempotency_key)
  const created = Date.parse(utc.parse(receipt.created_at)), expires = Date.parse(utc.parse(receipt.expires_at))
  if (receipt.actor_id !== binding.actor || receipt.event_id !== binding.event || receipt.operation_id !== operation
    || receipt.idempotency_key !== binding.key || created > now || created >= expires) throw Error('Unbound policy receipt')
  if (receipt.state === 'SUCCEEDED') {
    if (receipt.resource_id !== binding.event || receipt.resource_revision === null || receipt.resource_revision <= binding.eventRevision
      || receipt.failure_reason !== null || receipt.result_status !== 200) throw Error('Unbound publication result')
  } else if (receipt.state === 'FAILED') {
    if (receipt.resource_id !== null || receipt.resource_revision !== null || receipt.result_status !== null
      || !receipt.failure_reason || !(Object.values(DOMAIN_OPERATIONS[operation].errors).flat() as string[]).includes(receipt.failure_reason)) throw Error('Invalid policy failure')
  } else if (receipt.resource_id !== null || receipt.resource_revision !== null || receipt.failure_reason !== null || receipt.result_status !== null) throw Error('Invalid pending receipt')
  return freeze(structuredClone(receipt))
}
