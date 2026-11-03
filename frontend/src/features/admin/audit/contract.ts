import { z } from 'zod'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import { cursor, text, utc, uuid } from '../event/model/contract'
export const auditSchema = z.strictObject({ audit_id: uuid, event_id: uuid, actor_id: uuid, operation_id: text(100), target_id: uuid.nullable(),
  outcome: z.enum(['PENDING', 'SUCCEEDED', 'FAILED']), receipt_id: uuid, created_at: utc })
  // Never retain/display an arbitrary free-text operation label as audit content.
  .transform(row => ({ ...row, operation_id: Object.hasOwn(DOMAIN_OPERATIONS, row.operation_id) ? row.operation_id : 'Other operation' }))
export const auditListSchema = z.strictObject({ items: z.array(auditSchema).max(100), next_cursor: cursor })
  .refine(s => new Set(s.items.map(row => row.audit_id)).size === s.items.length)
export const auditQuerySchema = z.strictObject({ actor_id: uuid.optional(), operation_id: text(100).refine(s => Object.hasOwn(DOMAIN_OPERATIONS, s)).optional(),
  outcome: z.enum(['PENDING', 'SUCCEEDED', 'FAILED']).optional(), from: utc.optional(), until: utc.optional() })
  .refine(q => !q.from || !q.until || Date.parse(q.from) < Date.parse(q.until))
export type AuditEntry = z.infer<typeof auditSchema>
export type AuditQuery = z.infer<typeof auditQuerySchema>
