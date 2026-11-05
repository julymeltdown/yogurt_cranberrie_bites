import type { HttpClient, SessionPort } from '../http'
import { TransportError } from '../errors'
import type { MeetingCanonicalTransport, MeetingRequest } from './meeting'

/** Borrow the existing canonical client and session; no Auth, retry or cache owner. */
export function createHttpMeetingTransport(http: HttpClient, session: SessionPort): MeetingCanonicalTransport {
  return Object.freeze({
    async request<T>(descriptor: MeetingRequest<T>, signal: AbortSignal) {
      const scope = session.read()
      if (signal.aborted || !scope.userId || !scope.eventId || !session.isCurrent(scope)) {
        throw new TransportError('session-changed')
      }
      return http.request({
        method: descriptor.method, path: descriptor.path,
        signal,
        access: 'protected', errorFamily: 'domain', domainOperation: descriptor.operationId,
        ...(descriptor.query ? { query: descriptor.query } : {}),
        ...(descriptor.body ? { body: descriptor.body } : {}),
        ...(descriptor.idempotencyKey ? { idempotencyKey: descriptor.idempotencyKey } : {}),
        decode: descriptor.decode,
      })
    },
  })
}
