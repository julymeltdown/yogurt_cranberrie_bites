import type { AdminCommand, OperatorContext } from '../mural/contract'
import { uuid } from '../mural/contract'
import type { AdminTransport } from '../mural/transport'
import { closureCommandSchema, closureJobSchema } from './contract'
export function createClosureApi(transport: AdminTransport) {
  function decode(input: unknown, context: OperatorContext, jobId?: string) {
    const job = closureJobSchema.parse(input)
    if (job.event_id !== context.scope.eventId || jobId && job.job_id !== jobId) throw new Error('Wrong closure job')
    job.steps.forEach(Object.freeze); Object.freeze(job.steps); return Object.freeze(job)
  }
  return {
    read(context: OperatorContext, signal: AbortSignal, jobId: string) {
      uuid.parse(jobId)
      return transport.request(context, signal, { method: 'GET', path: `/api/concert/events/${context.scope.eventId}/admin/closure/${jobId}`, domainOperation: 'getConcertClosure',
        decode(input, ctx) { if (ctx.status !== 200) throw new Error('Invalid closure status'); return decode(input, context, jobId) } })
    },
    async send(context: OperatorContext, signal: AbortSignal, command: AdminCommand) {
      const parsed = closureCommandSchema.safeParse(command.body)
      if (command.operation !== 'closeConcertEvent' || !parsed.success) throw new Error('Invalid closure command')
      const body = parsed.data, resume = body.confirmation === 'RESUME_CLOSURE'
      if (resume ? context.phase !== 'READ_ONLY' : !['OPEN', 'LIVE'].includes(context.phase) || body.expected_revision !== context.eventRevision) throw new Error('Invalid closure phase or revision')
      return transport.post(context, signal, command, `/api/concert/events/${context.scope.eventId}/admin/closure`, input => {
        const data = decode(input, context, resume ? body.job_id : undefined)
        if (resume && data.revision <= body.expected_revision) throw new Error('Resume admission did not advance the job')
        return data
      })
    },
    async status(context: OperatorContext, signal: AbortSignal, command: AdminCommand) {
      const body = closureCommandSchema.parse(command.body)
      if (command.operation !== 'closeConcertEvent') throw new Error('Invalid closure operation')
      const receipt = await transport.status(context, signal, command)
      if (body.confirmation === 'RESUME_CLOSURE' && receipt.state === 'SUCCEEDED'
        && (receipt.resource_id !== body.job_id || receipt.resource_revision! <= body.expected_revision)) throw new Error('Wrong resume receipt resource')
      return receipt
    },
  }
}
export type ClosureApi = ReturnType<typeof createClosureApi>
