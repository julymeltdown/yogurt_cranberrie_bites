import { z } from 'zod'
import type { Decoder, HttpResult, SessionScope } from '../http'
import type { JsonValue } from '../errors'
import { TransportError } from '../errors'

/** Public JSON contract; no server implementation or generated C04 operation casts. */
export const MEETING_CONTRACT = Object.freeze({
  revision: '0981774d48eab79c15666ef27612310d470b4f69',
  openapiSha256: '08503641a551fbacaf134ecab71bbce1102c1f7da4ba560e5d15311694e0c2de',
})
const uuid = z.string().length(36).regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
  .refine(value => value !== '00000000-0000-0000-0000-000000000000')
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const cursor = z.string().regex(/^(0|[1-9][0-9]{0,15})$/)
  .refine(value => Number.isSafeInteger(Number(value)) && String(Number(value)) === value)
const identity = z.string().min(1).max(128).refine(value =>
  /^[A-Za-z0-9._~-]+$/.exec(value)?.[0] === value && value !== '.' && value !== '..')
const contextId = z.string().length(68).regex(/^vm1_[0-9a-f]{64}$/)
const instant = z.string().datetime({ offset: true }).refine(value => Number.isFinite(Date.parse(value)))
const text = z.string().refine(value => {
  const scalars = Array.from(value)
  return scalars.length >= 1 && scalars.length <= 2000 && value.trim().length > 0
    && new TextEncoder().encode(value).length <= 8000
    && scalars.every(char => {
      const code = char.codePointAt(0)!
      return !(code >= 0xd800 && code <= 0xdfff) && (code === 9 || code === 10 || !/\p{Cc}/u.test(char))
    })
})
const landmark = z.object({ landmarkId: identity, catalogRevision: revision, landmarkRevision: revision }).strict().readonly()
const selection = z.object({ selectedFriendId: uuid, landmark: landmark.nullable().optional() }).strict().readonly()
const invitationSelection = z.object({ selectedFriendId: uuid, landmark }).strict().readonly()
const decision = z.enum(['ACCEPT', 'DECLINE', 'CANCEL'])
const invitation = z.object({ id: uuid, eventId: uuid, senderId: uuid, recipientId: uuid, landmarkId: identity,
  revision, state: z.enum(['PENDING', 'ACCEPTED', 'DECLINED', 'CANCELLED', 'EXPIRED']), createdAt: instant,
  expiresAt: instant, allowedInvitationActions: z.array(decision).max(3).refine(items => new Set(items).size === items.length).readonly(),
}).strict().refine(value => Date.parse(value.createdAt) < Date.parse(value.expiresAt) && value.senderId !== value.recipientId)
  .readonly()
const message = z.object({ id: uuid, sequence: revision, senderId: uuid, body: text, invitationId: uuid.nullable(),
  createdAt: instant, expiresAt: instant,
}).strict().refine(value => Date.parse(value.createdAt) < Date.parse(value.expiresAt)).readonly()
const messageEffect = z.object({ id: uuid, conversationId: uuid, sequence: revision, createdAt: instant, expiresAt: instant })
  .strict().refine(value => Date.parse(value.createdAt) < Date.parse(value.expiresAt)).readonly()
const effect = z.union([
  z.object({ kind: z.literal('INVITATION'), invitation }).strict().readonly(),
  z.object({ kind: z.literal('MESSAGE'), message: messageEffect }).strict().readonly(),
])
const context = z.object({ contextId, eventId: uuid, actorId: uuid, selection, conversationId: uuid.nullable(),
  canRead: z.boolean(), canSend: z.boolean(), canInvite: z.boolean(), relationshipRevision: revision, policyRevision: revision,
  serverTime: instant, validUntil: instant, eventTimezone: z.string().min(1).refine(value => {
    try { if (/^[+-]/.test(value)) return false; new Intl.DateTimeFormat('en', { timeZone: value }); return true } catch { return false }
  }), invitationExpiresAt: instant.nullable(), contentExpiresAt: instant, purgeAt: instant,
}).strict().refine(value => Date.parse(value.serverTime) < Date.parse(value.validUntil)
  && Date.parse(value.serverTime) < Date.parse(value.contentExpiresAt)
  && Date.parse(value.purgeAt) >= Date.parse(value.contentExpiresAt)
  && (value.selection.landmark != null || (!value.canInvite && value.invitationExpiresAt === null))).readonly()
const messagePage = z.object({ contextId, conversationId: uuid.nullable(), items: z.array(message).max(50).readonly(),
  cursor, hasMore: z.boolean(), gap: z.boolean(), serverTime: instant,
}).strict().refine(value => {
  const ids = new Set<string>(); let previous = 0
  for (const item of value.items) {
    if (ids.has(item.id) || item.sequence <= previous) return false
    ids.add(item.id); previous = item.sequence
  }
  return Number(value.cursor) >= previous && (value.items.length === 0 || value.conversationId !== null)
}).readonly()
const inboxPage = z.object({ items: z.array(z.object({ selection, invitation }).strict().readonly()).max(50).readonly(),
  nextCursor: cursor.nullable(), serverTime: instant,
}).strict().readonly()
const createInvitation = z.object({ selection: invitationSelection }).strict().readonly()
const decideInvitation = z.object({ selection: invitationSelection, expectedRevision: revision, decision }).strict().readonly()
const sendMessage = z.object({ selection, body: text, invitationId: uuid.nullable().optional() }).strict().readonly()
const appliedReceipt = z.object({ commandKey: uuid, state: z.literal('APPLIED'), result: effect, replayed: z.boolean() }).strict().readonly()
const commandOperation = z.enum(['createVenueMeetingInvitation', 'decideVenueMeetingInvitation', 'sendVenueMeetingMessage'])
const statusApplied = z.object({ commandKey: uuid, state: z.literal('APPLIED'), operation: commandOperation,
  result: effect, serverTime: instant }).strict().readonly()
const unknownReceipt = z.object({ commandKey: uuid, state: z.literal('UNKNOWN'), serverTime: instant }).strict().readonly()
const commandReceipt = z.union([statusApplied, unknownReceipt])

const reasons = z.enum(['INVALID_INPUT', 'AUTHENTICATION_REQUIRED', 'PERMISSION_DENIED', 'ORIGIN_NOT_ALLOWED',
  'BROWSER_INTENT_REQUIRED', 'RESOURCE_UNAVAILABLE', 'REVISION_OR_KEY_CONFLICT', 'RATE_LIMITED', 'DEPENDENCY_UNAVAILABLE', 'DEADLINE_EXCEEDED'])
const errorResponse = z.object({ code: z.string().min(1).max(64), message: z.string().min(1).max(512),
  traceId: z.string().max(128).nullable().optional(),
  timestamp: z.string().max(64).datetime().nullable().optional(),
  details: z.object({ reason: reasons }).strict().readonly(),
}).strict().readonly()
export const meetingSchemas = Object.freeze({ ErrorResponse: errorResponse, Uuid: uuid, Revision: revision, Cursor: cursor, OpaqueIdentity: identity,
  Instant: instant, LandmarkSelection: landmark, Selection: selection, InvitationSelection: invitationSelection,
  MeetingContext: context, MeetingInvitation: invitation, CreateInvitation: createInvitation, DecideInvitation: decideInvitation,
  SendMessage: sendMessage, MeetingMessage: message, MeetingMessagePage: messagePage, MeetingInboxPage: inboxPage,
  MessageEffect: messageEffect, Effect: effect, AppliedReceipt: appliedReceipt, StatusAppliedReceipt: statusApplied,
  UnknownReceipt: unknownReceipt, CommandReceipt: commandReceipt,
})
export type MeetingDtos = { [K in keyof typeof meetingSchemas]: z.infer<(typeof meetingSchemas)[K]> }
export type MeetingLandmarkSelection = MeetingDtos['LandmarkSelection']
export type MeetingSelection = MeetingDtos['Selection']
export type MeetingInvitationSelection = MeetingDtos['InvitationSelection']
export type MeetingContext = MeetingDtos['MeetingContext']
export type MeetingInvitation = MeetingDtos['MeetingInvitation']
export type MeetingMessage = MeetingDtos['MeetingMessage']
export type MeetingMessagePage = MeetingDtos['MeetingMessagePage']
export type MeetingInboxPage = MeetingDtos['MeetingInboxPage']
export type MeetingAppliedReceipt = MeetingDtos['AppliedReceipt']
export type MeetingCommandReceipt = MeetingDtos['CommandReceipt']
export type MeetingSendMessage = MeetingDtos['SendMessage']
export type MeetingDecision = z.infer<typeof decision>
export type MeetingErrorCode = 'INVALID_INPUT' | 'INVALID_RESPONSE' | 'RESOURCE_UNAVAILABLE' | 'REVISION_OR_KEY_CONFLICT'
  | 'RATE_LIMITED' | 'DEPENDENCY_UNAVAILABLE' | 'STALE_SCOPE' | 'UNKNOWN_OUTCOME'
  | 'AUTHENTICATION_REQUIRED' | 'PERMISSION_DENIED' | 'DEADLINE_EXCEEDED'

/** Contains no input, credentials, underlying cause, server message or response body. */
export class MeetingClientError extends Error {
  constructor(readonly code: MeetingErrorCode, readonly uncertain = false, readonly commandKey: string | null = null) {
    super(`Meeting request unavailable (${code}).`); this.name = 'MeetingClientError'
  }
}
function refuse(code: MeetingErrorCode = 'INVALID_INPUT'): never { throw new MeetingClientError(code) }
/** Reject non-JSON values before schemas can drop explicit undefined or inherited data. */
function jsonValue(value: unknown, depth = 0, budget = { count: 0 }): asserts value is JsonValue {
  if (++budget.count > 4096 || depth > 12) refuse()
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return
  if (typeof value === 'number' && Number.isFinite(value)) return
  if (Array.isArray(value)) {
    if (Object.keys(value).length !== value.length) refuse()
    for (const item of value) jsonValue(item, depth + 1, budget)
    return
  }
  if (typeof value !== 'object' || Object.getPrototypeOf(value) !== Object.prototype || Reflect.ownKeys(value).length !== Object.keys(value).length) refuse()
  for (const key of Object.keys(value)) {
    const property = Object.getOwnPropertyDescriptor(value, key)!
    if (!('value' in property)) refuse()
    jsonValue(property.value, depth + 1, budget)
  }
}
export function decodeMeeting<K extends keyof MeetingDtos>(name: K, value: unknown): MeetingDtos[K] {
  try { jsonValue(value); return meetingSchemas[name].parse(value) as MeetingDtos[K] } catch { return refuse('INVALID_RESPONSE') }
}
function input<K extends keyof MeetingDtos>(name: K, value: unknown): MeetingDtos[K] {
  try { return decodeMeeting(name, value) } catch { return refuse() }
}
const statusReasons = { 400: 'INVALID_INPUT', 401: 'AUTHENTICATION_REQUIRED', 403: 'PERMISSION_DENIED',
  404: 'RESOURCE_UNAVAILABLE', 409: 'REVISION_OR_KEY_CONFLICT', 429: 'RATE_LIMITED',
  503: 'DEPENDENCY_UNAVAILABLE', 504: 'DEADLINE_EXCEEDED' } as const
/** Validate the canonical closed envelope, then discard all server text and diagnostics. */
export function decodeMeetingError(status: number, value: unknown): Readonly<{ reason: z.infer<typeof reasons> }> {
  const dto = decodeMeeting('ErrorResponse', value)
  const expected = Object.hasOwn(statusReasons, status) ? statusReasons[status as keyof typeof statusReasons] : null
  const allowed: readonly (z.infer<typeof reasons> | null)[] = status === 403 ? ['PERMISSION_DENIED', 'ORIGIN_NOT_ALLOWED', 'BROWSER_INTENT_REQUIRED'] : [expected]
  if (!expected || !allowed.includes(dto.details.reason)) return refuse('INVALID_RESPONSE')
  return Object.freeze({ reason: dto.details.reason })
}

export type MeetingQueryKind = 'selection' | 'invitation' | 'messages' | 'inbox'
export interface MeetingQuery { readonly selectedFriendId?: string; readonly landmarkId?: string; readonly catalogRevision?: number;
  readonly landmarkRevision?: number; readonly cursor?: string; readonly direction?: 'INCOMING' | 'OUTGOING' }
/** The wire forbids percent/plus aliases, duplicate keys, partial triples and numeric coercion. */
export function parseMeetingQuery(raw: string, kind: MeetingQueryKind): Readonly<MeetingQuery> {
  if (!['selection', 'invitation', 'messages', 'inbox'].includes(kind)) return refuse()
  if (typeof raw !== 'string' || raw.length > 512 || /[%+?#]/.test(raw)) return refuse()
  const pairs: Record<string, string> = Object.create(null)
  if (raw !== '') for (const item of raw.split('&')) {
    const parts = item.split('='); const key = parts[0]!
    if (parts.length !== 2 || !parts[1] || Object.hasOwn(pairs, key)) return refuse()
    pairs[key] = parts[1]
  }
  const allowed = kind === 'inbox' ? ['direction', 'cursor'] : ['selectedFriendId', 'landmarkId', 'catalogRevision', 'landmarkRevision', ...(kind === 'messages' ? ['cursor'] : [])]
  if (Object.keys(pairs).some(key => !allowed.includes(key))) return refuse()
  const result: Record<string, string | number> = {}
  if (kind === 'inbox') {
    if (pairs.direction !== 'INCOMING' && pairs.direction !== 'OUTGOING') return refuse()
    result.direction = pairs.direction
  } else {
    result.selectedFriendId = input('Uuid', pairs.selectedFriendId)
    const count = ['landmarkId', 'catalogRevision', 'landmarkRevision'].filter(key => Object.hasOwn(pairs, key)).length
    if ((count !== 0 && count !== 3) || (kind === 'invitation' && count !== 3)) return refuse()
    if (count) {
      result.landmarkId = input('OpaqueIdentity', pairs.landmarkId)
      for (const key of ['catalogRevision', 'landmarkRevision']) {
        const value = input('Cursor', pairs[key]); result[key] = input('Revision', Number(value))
      }
    }
  }
  if (Object.hasOwn(pairs, 'cursor')) result.cursor = input('Cursor', pairs.cursor)
  return Object.freeze(result)
}
function selectionQuery(value: MeetingSelection): Readonly<Record<string, string | number>> {
  return Object.freeze({ selectedFriendId: value.selectedFriendId, ...(value.landmark == null ? {} : {
    landmarkId: value.landmark.landmarkId, catalogRevision: value.landmark.catalogRevision, landmarkRevision: value.landmark.landmarkRevision,
  }) })
}
const operations = ['resolveVenueMeetingContext', 'listVenueMeetingInvitations', 'getVenueMeetingInvitation',
  'createVenueMeetingInvitation', 'decideVenueMeetingInvitation', 'listVenueMeetingMessages', 'sendVenueMeetingMessage', 'getVenueMeetingCommand'] as const
export type MeetingOperationId = (typeof operations)[number]
export const MEETING_OPERATION_IDS = Object.freeze(operations)
export interface MeetingRequest<T> {
  readonly operationId: MeetingOperationId
  readonly method: 'GET' | 'POST'
  readonly path: string
  readonly access: 'protected'
  readonly errorFamily: 'domain'
  readonly cache: 'no-store'
  readonly retry: 'never'
  readonly query?: Readonly<Record<string, string | number>>
  readonly body?: Readonly<{ json: JsonValue }>
  readonly idempotencyKey?: string
  readonly decode: Decoder<T>
}
const sealed = new WeakSet<object>()
function request<K extends keyof MeetingDtos>(operationId: MeetingOperationId, event: string, suffix: string, name: K,
  options: { query?: Readonly<Record<string, string | number>>; body?: unknown; key?: string;
    bind?: (value: MeetingDtos[K], scope: SessionScope) => void } = {}): MeetingRequest<MeetingDtos[K]> {
  const eventId = input('Uuid', event), path = `/api/concert/events/${eventId}/venue-meeting/${suffix}`
  const key = options.key === undefined ? undefined : input('Uuid', options.key)
  let body: Readonly<{ json: JsonValue }> | undefined
  if (options.body !== undefined) {
    jsonValue(options.body)
    if (new TextEncoder().encode(JSON.stringify(options.body)).length > 12000 || !key) return refuse()
    body = Object.freeze({ json: options.body })
  }
  const decode: Decoder<MeetingDtos[K]> = (value, ctx) => {
    if (ctx.status !== 200 || ctx.path !== path || ctx.scope.eventId !== eventId || !uuid.safeParse(ctx.scope.userId).success) return refuse('INVALID_RESPONSE')
    const dto = decodeMeeting(name, value); options.bind?.(dto, ctx.scope); return dto
  }
  const value = Object.freeze({ operationId, method: body ? 'POST' as const : 'GET' as const, path,
    access: 'protected' as const, errorFamily: 'domain' as const, cache: 'no-store' as const, retry: 'never' as const,
    ...(options.query ? { query: Object.freeze({ ...options.query }) } : {}), ...(body ? { body, idempotencyKey: key! } : {}), decode })
  sealed.add(value); return value
}
function sameSelection(a: MeetingSelection, b: MeetingSelection): boolean {
  return a.selectedFriendId === b.selectedFriendId && (a.landmark == null ? b.landmark == null : b.landmark != null
    && a.landmark.landmarkId === b.landmark.landmarkId && a.landmark.catalogRevision === b.landmark.catalogRevision
    && a.landmark.landmarkRevision === b.landmark.landmarkRevision)
}
function bindInvitation(value: MeetingInvitation, event: string, selected: MeetingInvitationSelection, scope: SessionScope): void {
  if (value.eventId !== event || value.landmarkId !== selected.landmark.landmarkId
    || !((value.senderId === scope.userId && value.recipientId === selected.selectedFriendId)
      || (value.recipientId === scope.userId && value.senderId === selected.selectedFriendId))) refuse('INVALID_RESPONSE')
}
function bindReceipt(value: MeetingAppliedReceipt, key: string, kind: 'MESSAGE' | 'INVITATION'): void {
  if (value.commandKey !== key || value.result.kind !== kind) refuse('INVALID_RESPONSE')
}
export const meetingRequests = Object.freeze({
  resolveContext(event: string, value: MeetingSelection) {
    const selected = input('Selection', value)
    return request('resolveVenueMeetingContext', event, 'context', 'MeetingContext', { query: selectionQuery(selected), bind(dto, scope) {
      if (dto.eventId !== event || dto.actorId !== scope.userId || !sameSelection(dto.selection, selected)) refuse('INVALID_RESPONSE')
    } })
  },
  listInvitations(event: string, direction: 'INCOMING' | 'OUTGOING', after?: string) {
    const query = parseMeetingQuery(`direction=${direction}${after === undefined ? '' : `&cursor=${after}`}`, 'inbox')
    return request('listVenueMeetingInvitations', event, 'invitations', 'MeetingInboxPage', { query: query as Readonly<Record<string, string | number>>, bind(dto, scope) {
      for (const row of dto.items) {
        const selected = input('InvitationSelection', row.selection)
        bindInvitation(row.invitation, event, selected, scope)
        if ((direction === 'INCOMING' ? row.invitation.recipientId : row.invitation.senderId) !== scope.userId) refuse('INVALID_RESPONSE')
      }
    } })
  },
  getInvitation(event: string, id: string, value: MeetingInvitationSelection) {
    const selected = input('InvitationSelection', value), invitationId = input('Uuid', id)
    return request('getVenueMeetingInvitation', event, `invitations/${invitationId}`, 'MeetingInvitation', { query: selectionQuery(selected), bind(dto, scope) {
      if (dto.id !== invitationId) refuse('INVALID_RESPONSE'); bindInvitation(dto, event, selected, scope)
    } })
  },
  createInvitation(event: string, key: string, value: MeetingDtos['CreateInvitation']) {
    const body = input('CreateInvitation', value)
    return request('createVenueMeetingInvitation', event, 'invitations', 'AppliedReceipt', { body, key, bind(dto, scope) {
      bindReceipt(dto, key, 'INVITATION'); if (dto.result.kind === 'INVITATION') bindInvitation(dto.result.invitation, event, body.selection, scope)
    } })
  },
  decideInvitation(event: string, id: string, key: string, value: MeetingDtos['DecideInvitation']) {
    const body = input('DecideInvitation', value), invitationId = input('Uuid', id)
    return request('decideVenueMeetingInvitation', event, `invitations/${invitationId}/decision`, 'AppliedReceipt', { body, key, bind(dto, scope) {
      bindReceipt(dto, key, 'INVITATION')
      if (dto.result.kind === 'INVITATION') {
        if (dto.result.invitation.id !== invitationId) refuse('INVALID_RESPONSE'); bindInvitation(dto.result.invitation, event, body.selection, scope)
      }
    } })
  },
  listMessages(event: string, value: MeetingSelection, after?: string, expectedContextId?: string) {
    const selected = input('Selection', value), afterCursor = after === undefined ? undefined : input('Cursor', after)
    if (expectedContextId !== undefined && !contextId.safeParse(expectedContextId).success) return refuse()
    return request('listVenueMeetingMessages', event, 'messages', 'MeetingMessagePage', {
      query: Object.freeze({ ...selectionQuery(selected), ...(afterCursor === undefined ? {} : { cursor: afterCursor }) }), bind(dto, scope) {
        if (expectedContextId !== undefined && dto.contextId !== expectedContextId) refuse('INVALID_RESPONSE')
        if (dto.items.some(item => item.senderId !== scope.userId && item.senderId !== selected.selectedFriendId)) refuse('INVALID_RESPONSE')
      },
    })
  },
  sendMessage(event: string, key: string, value: MeetingSendMessage) {
    const body = input('SendMessage', value)
    return request('sendVenueMeetingMessage', event, 'messages', 'AppliedReceipt', { body, key, bind(dto) { bindReceipt(dto, key, 'MESSAGE') } })
  },
  getCommand(event: string, key: string, value: MeetingSelection) {
    const selected = input('Selection', value), originalKey = input('Uuid', key)
    return request('getVenueMeetingCommand', event, `commands/${originalKey}`, 'CommandReceipt', { query: selectionQuery(selected), bind(dto, scope) {
      if (dto.commandKey !== originalKey) refuse('INVALID_RESPONSE')
      if (dto.state === 'APPLIED') {
        if ((dto.operation === 'sendVenueMeetingMessage') !== (dto.result.kind === 'MESSAGE')) refuse('INVALID_RESPONSE')
        if (dto.result.kind === 'INVITATION') {
          if (selected.landmark == null) refuse('INVALID_RESPONSE')
          bindInvitation(dto.result.invitation, event, { selectedFriendId: selected.selectedFriendId, landmark: selected.landmark }, scope)
        }
      }
    } })
  },
})

/** Root must supply the actual registered canonical binding. This module creates none. */
export interface MeetingCanonicalTransport {
  request<T>(request: MeetingRequest<T>, signal: AbortSignal): Promise<HttpResult<T>>
}
export interface MeetingLease {
  readonly scope: SessionScope
  readonly signal: AbortSignal
  /** Must include the caller's real Auth and full privacy scope fences. */
  isCurrent(): boolean
  /** Real clock supplied by the caller; no invented timezone, TTL or retention. */
  now(): number
}
function current(lease: MeetingLease): boolean { try { return !lease.signal.aborted && lease.isCurrent() } catch { return false } }
function sameScope(a: SessionScope, b: SessionScope): boolean {
  return a.generation === b.generation && a.userId === b.userId && a.eventId === b.eventId
}
export function createMeetingClient(transport: MeetingCanonicalTransport | null, lease: MeetingLease) {
  const scope = Object.freeze({ ...lease.scope })
  return Object.freeze({
    async execute<T>(descriptor: MeetingRequest<T>): Promise<HttpResult<T>> {
      if (!descriptor || !sealed.has(descriptor) || !uuid.safeParse(scope.userId).success || !uuid.safeParse(scope.eventId).success
        || !Number.isSafeInteger(scope.generation) || scope.generation < 0
        || !descriptor.path.startsWith(`/api/concert/events/${scope.eventId}/venue-meeting/`)) return refuse()
      if (!current(lease) || !sameScope(scope, lease.scope)) return refuse('STALE_SCOPE')
      if (!transport) return refuse('DEPENDENCY_UNAVAILABLE')
      const mutation = descriptor.method === 'POST', key = descriptor.idempotencyKey ?? null
      try {
        const result = await transport.request(descriptor, lease.signal)
        if (!current(lease) || !sameScope(scope, lease.scope) || !sameScope(result.scope, scope)) throw new MeetingClientError('STALE_SCOPE', mutation, key)
        const data = descriptor.decode(result.data, { scope, status: 200, path: descriptor.path })
        const now = lease.now()
        if (!Number.isFinite(now)) throw new MeetingClientError('RESOURCE_UNAVAILABLE', mutation, key)
        // Only actual owner-returned deadlines cap disclosure. No private cache is held.
        const value: unknown = data
        if (value !== null && typeof value === 'object') {
          if ('validUntil' in value && typeof value.validUntil === 'string' && Date.parse(value.validUntil) <= now) return refuse('RESOURCE_UNAVAILABLE')
          if ('contentExpiresAt' in value && typeof value.contentExpiresAt === 'string' && Date.parse(value.contentExpiresAt) <= now) return refuse('RESOURCE_UNAVAILABLE')
          if (descriptor.operationId === 'getVenueMeetingInvitation' && 'state' in value && value.state === 'PENDING'
            && 'expiresAt' in value && typeof value.expiresAt === 'string' && Date.parse(value.expiresAt) <= now) return refuse('RESOURCE_UNAVAILABLE')
          if ('items' in value && Array.isArray(value.items) && descriptor.operationId === 'listVenueMeetingInvitations'
            && value.items.some(item => item.invitation.state === 'PENDING' && Date.parse(item.invitation.expiresAt) <= now)) return refuse('RESOURCE_UNAVAILABLE')
          if ('items' in value && Array.isArray(value.items) && descriptor.operationId === 'listVenueMeetingMessages'
            && value.items.some(item => Date.parse(item.expiresAt) <= now)) return refuse('RESOURCE_UNAVAILABLE')
        }
        // Caller clock hooks may synchronously revoke the full privacy scope.
        if (!current(lease) || !sameScope(scope, lease.scope)) throw new MeetingClientError('STALE_SCOPE', mutation, key)
        return Object.freeze({ data, scope })
      } catch (error) {
        if (error instanceof MeetingClientError) {
          if (mutation && error.code === 'INVALID_RESPONSE') throw new MeetingClientError('UNKNOWN_OUTCOME', true, key)
          throw error
        }
        if (!current(lease) || !sameScope(scope, lease.scope)) throw new MeetingClientError('STALE_SCOPE', mutation, key)
        if (mutation && error instanceof TransportError && error.uncertain) throw new MeetingClientError('UNKNOWN_OUTCOME', true, key)
        if (error instanceof TransportError && error.kind === 'http' && error.status !== null && Object.hasOwn(statusReasons, error.status)) {
          throw new MeetingClientError(statusReasons[error.status as keyof typeof statusReasons])
        }
        throw new MeetingClientError(mutation ? 'UNKNOWN_OUTCOME' : 'DEPENDENCY_UNAVAILABLE', mutation, key)
      }
    },
  })
}
