import { z } from 'zod'
import { pixelOperationSchema, snapshotSchema } from '../editor/contract'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { PixelCommand } from '../editor/contract'
export { pixelOperationSchema, snapshotSchema }
export const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
export const publicPixelSchema = pixelOperationSchema.omit({ next_allowed_at: true })
export const deltaSchema = z.strictObject({ event_id: uuid, from_revision: revision, to_revision: revision,
  reset_required: z.boolean(), operations: z.array(publicPixelSchema).max(512) }).superRefine((d, ctx) => {
  const invalid = d.reset_required ? d.operations.length !== 0 : d.to_revision - d.from_revision !== d.operations.length
    || d.operations.some((p, index) => p.event_id !== d.event_id || p.revision !== d.from_revision + index + 1)
    || new Set(d.operations.map(p => p.operation_id)).size !== d.operations.length
  if (invalid) ctx.addIssue({ code: 'custom', message: 'Noncontiguous mural delta' })
})
export type PublicPixel = z.infer<typeof publicPixelSchema>
export type MuralDelta = z.infer<typeof deltaSchema>
export const bindingSchema = z.strictObject({ eventId: uuid, roomId: uuid, roomEpoch: uuid, streamId: uuid })
export type MuralBinding = Readonly<z.infer<typeof bindingSchema>>
export interface MuralAuthority { readonly binding: MuralBinding; readonly writable: boolean }
/** Observer of the incumbent world. No connect/send/heartbeat/engine capability. */
export interface MuralAuthorityPort { read(): MuralAuthority | null; subscribe(listener: () => void): () => void }
export interface MuralReadLease { readonly scope: PrivacyScope; readonly binding: MuralBinding; readonly generation: number }
export interface MuralReadRequest { readonly lease: MuralReadLease; readonly signal: AbortSignal }
export interface MuralReadPort {
  snapshot(request: MuralReadRequest): Promise<unknown>
  delta(request: MuralReadRequest & { readonly afterRevision: number }): Promise<unknown>
}
export interface Clock {
  now(): number
  set(ms: number, callback: () => void): unknown
  clear(timer: unknown): void
}
export const clock: Clock = { now: () => performance.now(), set: (ms, callback) => setTimeout(callback, ms), clear: timer => clearTimeout(timer as ReturnType<typeof setTimeout>) }
export const sameBinding = (a: MuralBinding | null, b: MuralBinding | null) => !!a && !!b
  && a.eventId === b.eventId && a.roomId === b.roomId && a.roomEpoch === b.roomEpoch && a.streamId === b.streamId
export const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
export const matchesPixel = (input: unknown, command: PixelCommand) => {
  const parsed = pixelOperationSchema.safeParse(input), p = parsed.data, b = command.body
  return parsed.success && p && p.event_id === command.scope.eventId && p.operation_id !== command.scope.eventId && p.revision === b.expected_revision + 1
    && p.x === b.x && p.y === b.y && p.color_index === b.color_index && Date.parse(p.next_allowed_at) >= Date.parse(p.committed_at) ? p : null
}
