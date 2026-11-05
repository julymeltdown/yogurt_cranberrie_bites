import type { PrivacyScope, createQueryPrivacy } from '../../query/privacy'
import type { HttpClient, SessionPort } from '../http'
import type { JsonValue } from '../errors'
import { notesOperations, notesSchemas, type NotesInputs, type NotesOperationId, type NotesResponses,
  type NoteCommand, type NoteError, type PlaceNote } from './notes.generated'
import { decodeWire, utcMilliseconds, VenueContractError, type WireSchema } from './schema'

export type * from './notes.generated'
export { notesProvenance } from './notes.generated'
export { VenueContractError } from './schema'

type Privacy = Pick<ReturnType<typeof createQueryPrivacy>, 'run' | 'isCurrent'>
interface Operation {
  readonly method: string
  readonly path: string
  readonly parameters: readonly { readonly key: string; readonly name: string; readonly location: string; readonly required: boolean }[]
  readonly input: WireSchema
  readonly success: Readonly<Record<string, WireSchema>>
  readonly errors: Readonly<Record<string, WireSchema>>
}
const definitions: Readonly<Record<string, WireSchema>> = notesSchemas
const operations: Readonly<Record<NotesOperationId, Operation>> = notesOperations
export class NotesApiError extends Error {
  constructor(readonly status: number, readonly code: NoteError['code'], readonly reason: NoteError['details']['reason']) {
    super('Venue notes request was rejected')
    this.name = 'NotesApiError'
  }
}

/** Declarative canonical-HTTP input. This module never fetches, logs in, caches, or retries. */
export interface NotesRequest<T> {
  readonly operationId: NotesOperationId
  readonly eventId: string
  readonly method: 'GET' | 'POST' | 'DELETE'
  readonly path: string
  readonly query: Readonly<Record<string, string | number | boolean>>
  readonly body?: JsonValue
  readonly idempotencyKey?: string
  readonly cache: 'no-store'
  decode(value: unknown, status: number): T
}
/** Implement only through the actual registered canonical HttpPort; absent registration is unavailable. */
export interface NotesTransport {
  send<T>(request: NotesRequest<T>, context: { readonly scope: PrivacyScope; readonly signal: AbortSignal }): Promise<{
    readonly status: number; readonly data: unknown
  }>
}

/** Borrow the shell's existing HTTP and session instances; no refresh or replay policy is added. */
export function createHttpNotesTransport(http: Pick<HttpClient, 'request'>, session: Pick<SessionPort, 'isCurrent'>): NotesTransport {
  return Object.freeze({
    async send<T>(request: NotesRequest<T>, context: {readonly scope: PrivacyScope; readonly signal: AbortSignal}) {
      if (!session.isCurrent(context.scope) || context.signal.aborted || request.eventId !== context.scope.eventId) {
        throw new VenueContractError('scope-expired')
      }
      const result = await http.request({path:request.path, method:request.method, access:'protected', errorFamily:'domain',
        domainOperation:request.operationId, query:request.query, signal:context.signal,
        ...(request.idempotencyKey ? {idempotencyKey:request.idempotencyKey} : {}),
        ...(request.body !== undefined ? {body:{json:request.body}} : {}),
        decode:(value, decodedContext) => ({status:decodedContext.status, data:request.decode(value, decodedContext.status)}),
      })
      if (!session.isCurrent(context.scope) || context.signal.aborted) throw new VenueContractError('scope-expired')
      return result.data
    },
  })
}

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid response object')
  return value as Record<string, unknown>
}
function responseContext(value: unknown, expected: Readonly<Record<string, unknown>>, id: NotesOperationId) {
  const response = record(value)
  if ('server_now' in response) {
    const now = utcMilliseconds(response.server_now as string)
    if (utcMilliseconds(response.access_until as string) <= now) throw new Error('Expired read lease')
    const notes: PlaceNote[] = []
    if (Array.isArray(response.items)) notes.push(...response.items as PlaceNote[])
    if (Array.isArray(response.replies)) notes.push(...response.replies as PlaceNote[])
    if (response.note) notes.push(response.note as PlaceNote)
    if (response.parent) notes.push(response.parent as PlaceNote)
    if (new Set(notes.map(n => n.id)).size !== notes.length) throw new Error('Duplicate note identity')
    for (const note of notes) {
      if (note.landmark_id !== expected.landmarkId || utcMilliseconds(note.expires_at) <= now
        || utcMilliseconds(note.expires_at) <= utcMilliseconds(note.created_at)) throw new Error('Wrong or expired place note')
    }
    if (id === 'getVenuePlaceNote') {
      const parent = response.note as PlaceNote
      if (parent.id !== expected.noteId || (response.replies as PlaceNote[]).some(reply => reply.parent_id !== parent.id
        || reply.expires_at !== parent.expires_at || reply.audience !== parent.audience)) throw new Error('Wrong note thread')
    }
    if (id === 'listVenuePlaceNotes' && (response.parent !== null || (response.items as PlaceNote[]).some(n => n.parent_id !== null))) {
      throw new Error('Wrong root-note page')
    }
    if (id === 'listVenuePlaceReplies') {
      const parent = response.parent as PlaceNote | null
      if (!parent || parent.id !== expected.noteId || (response.items as PlaceNote[]).some(reply => reply.parent_id !== parent.id
        || reply.expires_at !== parent.expires_at || reply.audience !== parent.audience)) throw new Error('Wrong reply page')
    }
  } else {
    const command = response as unknown as NoteCommand
    if (command.operation_id !== id || utcMilliseconds(command.expires_at) <= utcMilliseconds(command.created_at)) {
      throw new Error('Wrong command identity')
    }
    if (id === 'withdrawVenuePlaceNote' && command.resource && command.resource.id !== expected.noteId) throw new Error('Wrong withdrawal receipt')
  }
}

export function buildNotesRequest<K extends NotesOperationId>(id: K, input: NotesInputs[K]): NotesRequest<NotesResponses[K]> {
  if (!Object.hasOwn(operations, id)) throw new VenueContractError('invalid-request')
  const operation = operations[id]
  const args = decodeWire<Readonly<Record<string, unknown>>>(input, operation.input, definitions, 'invalid-request')
  if (id === 'createVenuePlaceNote') {
    const body = record(args.body), selection = record(body.landmark_selection)
    if (selection.landmark_id !== args.landmarkId) throw new VenueContractError('invalid-request')
  }
  let path = operation.path
  const query: Record<string, string | number | boolean> = Object.create(null)
  let idempotencyKey: string | undefined
  for (const parameter of operation.parameters) {
    const value = args[parameter.key]
    if (value === undefined) continue
    if (parameter.location === 'path') path = path.replace('{'+parameter.name+'}', encodeURIComponent(String(value)))
    else if (parameter.location === 'query') query[parameter.name] = value as string | number | boolean
    else if (parameter.location === 'header' && parameter.name.toLowerCase() === 'idempotency-key') idempotencyKey = value as string
    else throw new VenueContractError('invalid-request')
  }
  return Object.freeze({ operationId: id, eventId: args.eventId as string, method: operation.method as NotesRequest<never>['method'],
    path, query: Object.freeze(query), ...(args.body ? {body: args.body as JsonValue} : {}), ...(idempotencyKey ? {idempotencyKey} : {}), cache: 'no-store',
    decode(value: unknown, status: number): NotesResponses[K] {
      const success = operation.success[String(status)]
      if (!success) {
        const errorSchema = operation.errors[String(status)]
        if (!errorSchema) throw new VenueContractError('invalid-response')
        const error = decodeWire<NoteError>(value, errorSchema, definitions, 'invalid-response')
        throw new NotesApiError(status, error.code, error.details.reason)
      }
      const decoded = decodeWire<NotesResponses[K]>(value, success, definitions, 'invalid-response')
      try { responseContext(decoded, args, id) } catch { throw new VenueContractError('invalid-response') }
      return decoded
    } })
}

export function createNotesClient({ privacy, transport }: { readonly privacy: Privacy; readonly transport: NotesTransport }) {
  return Object.freeze({
    async request<K extends NotesOperationId>(id: K, input: NotesInputs[K], context: { readonly scope: PrivacyScope; readonly signal: AbortSignal }): Promise<NotesResponses[K]> {
      const request = buildNotesRequest(id, input)
      if (!context.scope.userId || request.eventId !== context.scope.eventId || !privacy.isCurrent(context.scope) || context.signal.aborted) {
        throw new VenueContractError('scope-expired')
      }
      return privacy.run(context.scope, async current => {
        if (current.signal.aborted || !privacy.isCurrent(context.scope)) throw new VenueContractError('scope-expired')
        const response = await transport.send(request, {scope: context.scope, signal: current.signal})
        if (current.signal.aborted || !privacy.isCurrent(context.scope)) throw new VenueContractError('scope-expired')
        return request.decode(response.data, response.status)
      }, context.signal)
    },
  })
}
