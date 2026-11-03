import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import { WorldTransportError, type AcquireWorldTicket } from '../network/WorldTransport'
import { decodeTicket, validate } from '../network/transport/contract'

/** C05 mutation: the canonical client supplies intent header, no-store and zero automatic replay.
 * Auth.request only refreshes authorized GETs; this POST never opts into another refresh layer.
 */
export function createTicketAcquirer(auth: SessionController, lease: PrivacyScope): AcquireWorldTicket {
  return async ({ scope, body, signal }) => {
    if (scope.userId !== lease.userId || scope.eventId !== lease.eventId || scope.generation !== lease.generation
      || !auth.privacy.isCurrent(lease)) throw new WorldTransportError('invalid-context')
    validate('WorldTicketRequest', body)
    return auth.privacy.run(lease, async context => {
      const response = await auth.request({ path: `/api/concert/events/${lease.eventId}/world-tickets`,
        method: 'POST', access: 'protected', errorFamily: 'core', body: { json: { ...body } }, signal: context.signal,
        decode: value => {
          const ticket = decodeTicket(value)
          if (ticket.event_id !== lease.eventId || ticket.room_id !== body.room_id || ticket.map_id !== body.map_id
            || ticket.map_revision !== body.map_revision || ticket.avatar_manifest_id !== body.avatar_manifest_id
            || ticket.avatar_manifest_version !== body.avatar_manifest_version) throw new WorldTransportError('invalid-context')
          return ticket
        } })
      if (!auth.session.isCurrent(response.scope) || response.scope.generation !== lease.generation
        || response.scope.userId !== lease.userId || response.scope.eventId !== lease.eventId) throw new WorldTransportError('invalid-context')
      return response.data
    }, signal)
  }
}
