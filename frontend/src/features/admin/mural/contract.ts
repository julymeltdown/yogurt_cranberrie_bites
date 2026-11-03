import { z } from 'zod'
import type { PrivacyScope, createQueryPrivacy } from '../../../shared/query/privacy'

export const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
export const reason = z.string().min(1).max(500).refine(v => v.trim().length > 0)
export const utc = z.string().refine(v => /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(v)
  && Number.isFinite(Date.parse(v)) && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19))
const rectangle = { x_min: z.number().int().min(0).max(127), y_min: z.number().int().min(0).max(127),
  x_max_exclusive: z.number().int().min(1).max(128), y_max_exclusive: z.number().int().min(1).max(128) }
const nonempty = (r: { x_min: number; y_min: number; x_max_exclusive: number; y_max_exclusive: number }) => r.x_min < r.x_max_exclusive && r.y_min < r.y_max_exclusive
export const boundsSchema = z.strictObject(rectangle).refine(nonempty)
export const muralStateSchema = z.strictObject({ event_id: uuid, state: z.enum(['EDITABLE', 'PAUSED', 'FROZEN']), revision, snapshot_id: uuid, locks: z.array(boundsSchema).max(128) })
export const controlSchema = z.strictObject({ expected_revision: revision, action: z.enum(['PAUSE', 'RESUME', 'FREEZE']), reason })
export const lockSchema = z.strictObject({ expected_revision: revision, ...rectangle, locked: z.boolean(), reason }).refine(nonempty)
export const restoreSchema = z.strictObject({ expected_revision: revision, snapshot_id: uuid, reason })
export type MuralAdminState = z.infer<typeof muralStateSchema>
export type MuralIntent =
  | { readonly operation: 'controlConcertMural'; readonly body: Readonly<z.infer<typeof controlSchema>> }
  | { readonly operation: 'lockConcertMuralRegion'; readonly body: Readonly<z.infer<typeof lockSchema>> }
  | { readonly operation: 'restoreConcertMural'; readonly body: Readonly<z.infer<typeof restoreSchema>> }
export type PrivacyPort = Pick<ReturnType<typeof createQueryPrivacy>, 'capture' | 'isCurrent' | 'registerCleanup'>
/** Supplied by the admitted shell's fresh server-qualified event role/settings owner. Never a role lookup endpoint. */
export interface OperatorContext {
  readonly scope: PrivacyScope
  readonly role: 'OPERATOR'
  readonly roleRevision: number
  readonly eventRevision: number
  readonly phase: 'DRAFT' | 'OPEN' | 'LIVE' | 'READ_ONLY' | 'ARCHIVED'
}
export interface OperatorContextPort { read(): OperatorContext | null; subscribe(listener: () => void): () => void }
export interface AdminCommand {
  readonly operation: MuralIntent['operation'] | 'closeConcertEvent'
  readonly key: string
  readonly body: Readonly<Record<string, string | number | boolean>>
  readonly scope: PrivacyScope
  readonly roleRevision: number
}
export const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation
  && a.policyGeneration === b.policyGeneration && a.gate === b.gate
export const sameContext = (a: OperatorContext, b: OperatorContext) => sameScope(a.scope, b.scope) && a.roleRevision === b.roleRevision
  && a.eventRevision === b.eventRevision && a.phase === b.phase
export function qualifiedContext(privacy: PrivacyPort, port: OperatorContextPort): OperatorContext | null {
  try {
    const value = port.read(), scope = privacy.capture()
    if (!value || value.role !== 'OPERATOR' || !privacy.isCurrent(scope) || !sameScope(scope, value.scope)
      || !uuid.safeParse(scope.userId).success || !uuid.safeParse(scope.eventId).success || !revision.safeParse(value.roleRevision).success
      || !revision.safeParse(value.eventRevision).success || !['DRAFT', 'OPEN', 'LIVE', 'READ_ONLY', 'ARCHIVED'].includes(value.phase)) return null
    return Object.freeze({ ...value, scope: Object.freeze({ ...value.scope }) })
  } catch { return null }
}
