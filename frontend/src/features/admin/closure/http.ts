import type { AdminCommand, OperatorContext } from '../mural/contract'
import { uuid } from '../mural/contract'
import type { AdminTransport } from '../mural/transport'
import { closeEventSchema, closureJobSchema } from './contract'
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
    send(context: OperatorContext, signal: AbortSignal, command: AdminCommand) {
      if (command.operation !== 'closeConcertEvent' || !closeEventSchema.safeParse(command.body).success) throw new Error('Invalid closure command')
      return transport.post(context, signal, command, `/api/concert/events/${context.scope.eventId}/admin/closure`, input => decode(input, context))
    },
    status: transport.status,
  }
}
export type ClosureApi = ReturnType<typeof createClosureApi>
