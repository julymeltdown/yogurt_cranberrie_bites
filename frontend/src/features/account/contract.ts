import { z } from 'zod'
import { freeze, uuid } from '../safety/contract'

const timestamp = z.iso.datetime({ offset: true })
// Published contract d3b7c382; unknown targets never count toward completion.
export const deletionTargets = [
  'PROFILE', 'FRIEND', 'POST', 'NOTIFICATION',
  'CONCERT_TICKETS', 'CONCERT_SESSIONS', 'CONCERT_SHARING', 'CONCERT_PRIVATE_DATA',
] as const
const step = z.object({
  target: z.enum(deletionTargets),
  status: z.enum(['PENDING', 'IN_PROGRESS', 'RETRY', 'SUCCEEDED', 'DEAD_LETTERED']),
  attemptCount: z.number().int().safe().min(0),
  nextAttemptAt: timestamp,
  lastError: z.string().nullable(),
  completedAt: timestamp.nullable(),
})
// Inherited Auth schemas are open; strip extensions instead of exposing them.
const saga = z.object({
  id: uuid, userId: uuid,
  status: z.enum(['IN_PROGRESS', 'COMPLETED', 'DEAD_LETTERED']),
  retentionUntil: timestamp, createdAt: timestamp, updatedAt: timestamp,
  completedAt: timestamp.nullable(), steps: z.array(step).max(deletionTargets.length),
})
const accepted = z.object({ userId: uuid, deleted: z.boolean(), sagaId: uuid, saga })
export interface DeletionBinding {
  readonly actorId: string
  readonly sagaId: string | null
  readonly updatedAt: number | null
}
export interface DeletionProgress {
  readonly state: 'pending' | 'partial-failure' | 'complete'
  readonly steps: readonly { readonly target: typeof deletionTargets[number]; readonly state: z.infer<typeof step>['status'] }[]
}
export function decodeDeletion(value: unknown, status: 200 | 202, binding: DeletionBinding, now: number) {
  const envelope = status === 202 ? accepted.parse(value) : null
  const s = envelope?.saga ?? saga.parse(value)
  if (envelope && s.status === 'COMPLETED' && !envelope.deleted) throw new Error('Account not disabled')
  if (s.userId !== binding.actorId
    || envelope && (envelope.userId !== binding.actorId || envelope.sagaId !== s.id)
    || binding.sagaId !== null && s.id !== binding.sagaId) throw new Error('Changed deletion identity')
  const created = Date.parse(s.createdAt), updated = Date.parse(s.updatedAt)
  if (created > updated || updated > now || binding.updatedAt !== null && updated < binding.updatedAt
    || s.steps.length !== deletionTargets.length
    || new Set(s.steps.map(step => step.target)).size !== deletionTargets.length) throw new Error('Invalid deletion progress')
  const finished = s.steps.every(step => step.status === 'SUCCEEDED' && step.completedAt !== null
    && Date.parse(step.completedAt) >= created && Date.parse(step.completedAt) <= updated)
  const completed = s.completedAt === null ? null : Date.parse(s.completedAt)
  if (s.status === 'COMPLETED' && (!finished || completed === null || completed < created || completed > updated
    || s.steps.some(step => Date.parse(step.completedAt!) > completed))) throw new Error('Incomplete deletion')
  if (s.status !== 'COMPLETED' && completed !== null) throw new Error('Invalid pending deletion')
  const byTarget = new Map(s.steps.map(step => [step.target, step]))
  const progress: DeletionProgress = freeze({
    state: s.status === 'COMPLETED' ? 'complete'
      : s.status === 'DEAD_LETTERED' || s.steps.some(step => step.status === 'DEAD_LETTERED') ? 'partial-failure' : 'pending',
    steps: deletionTargets.map(target => ({ target, state: byTarget.get(target)!.status })),
  })
  return freeze({ binding: { actorId: binding.actorId, sagaId: s.id, updatedAt: updated }, progress })
}
