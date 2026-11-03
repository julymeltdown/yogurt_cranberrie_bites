import operations from '../../../../../contracts/realtime/v1/operations.json'
import type { Decoder, SessionScope } from '../../../shared/api/http'
import { validate } from '../../../world/network/transport/contract'
import type { WorldRooms } from '../../../world/network/WorldTransport'
import { immutable } from '../../../world/entities/replicated'
export const canonicalId = (v: unknown): v is string => typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v)
const operation = operations.find(o => o.operationId === 'listConcertRooms')
if (!operation || operation.method !== 'get' || operation.path !== '/api/concert/events/{eventId}/rooms' || operation.response !== 'WorldRooms' || operation.response_status !== 200 || operation.retry !== 'READ_ONCE_AFTER_REFRESH' || operation.cache !== 'NO_STORE') throw new Error('Room contract changed')
/** Reuse the frozen C05 oracle; add request/own-scope and collection invariants. */
export function validateWorldRooms(scope: SessionScope): Decoder<WorldRooms> { return (value, context) => {
  if (!canonicalId(scope.userId) || !canonicalId(scope.eventId) || context.status !== 200 || context.path !== `/api/concert/events/${scope.eventId}/rooms`
    || context.scope.userId !== scope.userId || context.scope.eventId !== scope.eventId || context.scope.generation !== scope.generation) throw new Error('Invalid room context')
  const data = validate<WorldRooms>('WorldRooms', value)
  if (data.event_id !== scope.eventId || new Set(data.items.map(r => r.room_id)).size !== data.items.length || !data.items.some(r => r.room_id === data.default_room_id)) throw new Error('Invalid room catalog')
  return immutable(structuredClone(data))
} }
