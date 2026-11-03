import type { AdminTransport } from './transport'
import type { AdminCommand, MuralIntent, OperatorContext } from './contract'
import { controlSchema, lockSchema, muralStateSchema, restoreSchema } from './contract'

export function createMuralAdminApi(transport: AdminTransport) {
  function decode(input: unknown, context: OperatorContext) {
    const data = muralStateSchema.parse(input)
    if (data.event_id !== context.scope.eventId) throw new Error('Wrong mural event')
    data.locks.forEach(Object.freeze); Object.freeze(data.locks); return Object.freeze(data)
  }
  return {
    async read(context: OperatorContext, signal: AbortSignal) {
      return transport.request(context, signal, { method: 'GET', path: `/api/concert/events/${context.scope.eventId}/admin/mural`, domainOperation: 'getConcertAdminMural',
        decode(input, ctx) { if (ctx.status !== 200) throw new Error('Invalid mural status'); return decode(input, context) } })
    },
    async send(context: OperatorContext, signal: AbortSignal, command: AdminCommand) {
      const operation = command.operation as MuralIntent['operation']
      const schema = operation === 'controlConcertMural' ? controlSchema : operation === 'lockConcertMuralRegion' ? lockSchema : operation === 'restoreConcertMural' ? restoreSchema : null
      if (!schema?.safeParse(command.body).success) throw new Error('Invalid mural command')
      const suffix = operation === 'controlConcertMural' ? 'control' : operation === 'lockConcertMuralRegion' ? 'locks' : 'restore'
      return transport.post(context, signal, command, `/api/concert/events/${context.scope.eventId}/admin/mural/${suffix}`, input => {
        const data = decode(input, context)
        if (data.revision !== Number(command.body.expected_revision) + 1) throw new Error('Invalid mural revision')
        const action = command.body.action
        if (operation === 'restoreConcertMural' && data.state !== 'PAUSED' || action === 'PAUSE' && data.state !== 'PAUSED'
          || action === 'RESUME' && data.state !== 'EDITABLE' || action === 'FREEZE' && data.state !== 'FROZEN') throw new Error('Invalid mural transition result')
        return data
      })
    },
    status: transport.status,
  }
}
export type MuralAdminApi = ReturnType<typeof createMuralAdminApi>
