import { describe, expect, it } from 'vitest'
import admin from '../../../../../../../contracts/domain/v1/admin.schema.json'
import operations from '../../../../../../../contracts/domain/v1/operations.json'
import { DOMAIN_OPERATIONS } from '../../../../../shared/api/domain-operations.generated'
import start from '../../../../../../../contracts/fixtures/domain/closure-start-command.json'
import resume from '../../../../../../../contracts/fixtures/domain/closure-resume-command.json'
import missing from '../../../../../../../contracts/fixtures/domain/closure-resume-missing-job.json'
import mixed from '../../../../../../../contracts/fixtures/domain/closure-start-with-job.json'
import extra from '../../../../../../../contracts/fixtures/domain/closure-resume-unknown-field.json'
import malformed from '../../../../../../../contracts/fixtures/domain/closure-resume-invalid-job.json'
import { closeEventSchema, closureCommandSchema, resumeClosureSchema } from '../../contract'
describe('published C04 closure union', () => {
  it('matches both closed source branches and generated route metadata without changing the retained START export', () => {
    for (const [index, schema] of [closeEventSchema, resumeClosureSchema].entries()) {
      const branch = admin.$defs.CloseEvent.oneOf[index]!
      expect(Object.keys(schema.shape).sort()).toEqual([...branch.required].sort()); expect(branch.additionalProperties).toBe(false)
    }
    expect(DOMAIN_OPERATIONS.closeConcertEvent.path).toBe('/api/concert/events/{eventId}/admin/closure')
    expect(DOMAIN_OPERATIONS.closeConcertEvent.success).toEqual([200, 202])
    const modes = operations.find(o => o.operationId === 'closeConcertEvent')!.command_modes!
    expect(modes.RESUME_CLOSURE.allowed_event_phases).toEqual(['READ_ONLY']); expect(modes.RESUME_CLOSURE.required_job_state).toBe('PARTIAL_FAILURE')
  })
  it.each([start, resume])('accepts the pinned $confirmation fixture', value => { expect(closureCommandSchema.safeParse(value).success).toBe(true) })
  it.each([missing, mixed, extra, malformed])('rejects the pinned malformed closed request', value => { expect(closureCommandSchema.safeParse(value).success).toBe(false) })
})
