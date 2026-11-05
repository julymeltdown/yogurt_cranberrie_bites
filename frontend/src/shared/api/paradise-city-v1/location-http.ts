import { DOMAIN_OPERATIONS, type DomainOperationId } from '../domain-operations.generated'
import { domainOperation, matchesDomainRequest } from '../domain'
import { TransportError } from '../errors'
import type { HttpClient, SessionPort } from '../http'
import { locationOperations, LocationUnavailableError, type LocationCanonicalTransport, type LocationRequestDescriptor } from './location'

function registered(id: string): id is DomainOperationId {
  return Object.hasOwn(DOMAIN_OPERATIONS, id)
}

/** Borrow the canonical client/session. Root owns actual Location registration. */
export function createHttpLocationTransport(http: HttpClient, session: SessionPort): LocationCanonicalTransport {
  return Object.freeze({
    async request<T>(descriptor: LocationRequestDescriptor<T>, signal: AbortSignal) {
      const scope = session.read()
      if (signal.aborted) throw new TransportError('aborted')
      if (!scope.userId || !scope.eventId || !session.isCurrent(scope)) throw new TransportError('session-changed')
      const id = descriptor.operationId
      if (!Object.hasOwn(locationOperations, id)) throw new TransportError('invalid-request')
      if (!registered(id)) throw new LocationUnavailableError()
      const operation = domainOperation(id)
      if (!operation) throw new LocationUnavailableError()
      if (descriptor.access !== 'protected' || descriptor.errorFamily !== 'domain' || descriptor.cache !== 'no-store'
        || !matchesDomainRequest(operation, descriptor.method, descriptor.path, descriptor.query)
        || operation.idempotency !== (descriptor.method !== 'GET')
        || (descriptor.method === 'GET' && (descriptor.body !== undefined || descriptor.idempotencyKey !== undefined))) {
        throw new TransportError('invalid-request')
      }
      const result = await http.request<T>({
        method: descriptor.method, path: descriptor.path,
        access: 'protected', errorFamily: 'domain', domainOperation: id, signal,
        ...(descriptor.query === undefined ? {} : { query: descriptor.query }),
        ...(descriptor.body === undefined ? {} : { body: descriptor.body }),
        ...(descriptor.idempotencyKey === undefined ? {} : { idempotencyKey: descriptor.idempotencyKey }),
        decode: descriptor.decode,
      })
      const uncertain = descriptor.method !== 'GET'
      if (!session.isCurrent(scope) || result.scope.generation !== scope.generation
        || result.scope.userId !== scope.userId || result.scope.eventId !== scope.eventId) {
        throw new TransportError('session-changed', { uncertain })
      }
      if (signal.aborted) throw new TransportError('aborted', { uncertain })
      return result
    },
  })
}
