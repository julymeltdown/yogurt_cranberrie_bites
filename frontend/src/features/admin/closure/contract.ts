import { z } from 'zod'
import { reason, revision, uuid } from '../mural/contract'
export const closeEventSchema = z.strictObject({ expected_revision: revision, confirmation: z.literal('CLOSE_EVENT'), reason })
export const stepNames = ['READ_ONLY', 'FREEZE_MURAL', 'SNAPSHOT', 'EXPIRE_GRANTS', 'EXPIRE_BEACONS', 'EXPORT', 'CLEANUP'] as const
export const closureJobSchema = z.strictObject({ job_id: uuid, event_id: uuid, state: z.enum(['PENDING', 'RUNNING', 'PARTIAL_FAILURE', 'COMPLETED']), revision,
  steps: z.array(z.strictObject({ name: z.enum(stepNames), state: z.enum(['PENDING', 'SUCCEEDED', 'FAILED']) })).length(7) }).superRefine((job, ctx) => {
    if (new Set(job.steps.map(s => s.name)).size !== 7 || job.state === 'COMPLETED' && job.steps.some(s => s.state !== 'SUCCEEDED')
      || job.state === 'PARTIAL_FAILURE' && !job.steps.some(s => s.state === 'FAILED')) ctx.addIssue({ code: 'custom', message: 'Inconsistent closure progress' })
  })
export type ClosureJob = z.infer<typeof closureJobSchema>
