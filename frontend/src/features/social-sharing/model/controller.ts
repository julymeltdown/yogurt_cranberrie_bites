import type { DomainOperationId } from '../../../shared/api/domain-operations.generated'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import { TransportError, type JsonValue } from '../../../shared/api/errors'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { blank, createRuntime, failure, type Failure, type Slot, type SocialOptions } from './runtime'
import { decode, freeze, handleList, handleState, offer, receipt, requestList, reveal, socialRequest, uuid, validHandle,
  type HandleState, type Platform, type PrivateReveal, type Receipt, type SharingOffer, type SocialRequest } from './contract'

export interface Mutation { readonly phase: 'idle' | 'sending' | 'pending' | 'uncertain' | 'verifying' | 'blocked' | 'succeeded' | 'failed'; readonly key: string | null; readonly operation: string | null; readonly failure: Failure | null }
type Direction = 'INCOMING' | 'OUTGOING'
export interface SocialState {
  readonly available: boolean; readonly readOnly: boolean; readonly userId: string | null; readonly revealAvailable: boolean
  readonly scopeKey: string | null
  readonly handles: Slot<{ items: HandleState[] }>; readonly handleDrafts: Readonly<Record<Platform, string>>
  readonly requests: Slot<{ items: SocialRequest[]; next_cursor: string | null }>; readonly direction: Direction
  readonly selectedId: string | null; readonly detail: Slot<SocialRequest>; readonly offer: Slot<SharingOffer>
  readonly requesterConsent: readonly Platform[]; readonly approvedPlatforms: readonly Platform[]; readonly recipientConsent: boolean
  readonly secret: Slot<PrivateReveal>; readonly mutation: Mutation; readonly now: number
}
interface Command {
  readonly operation: DomainOperationId; readonly method: 'PUT' | 'POST' | 'DELETE'; readonly suffix: string
  readonly body?: JsonValue; readonly query?: Readonly<Record<string, number>>; readonly resource: string | null; readonly revision: number
  readonly kind: 'handle' | 'request'; readonly platform?: Platform; readonly reason?: 'handle-replaced' | 'consent-revoked'
  readonly requestMatches?: (value: SocialRequest) => boolean
  readonly check: (value: unknown) => HandleState | SocialRequest; readonly key: string; readonly scope: PrivacyScope
}
function sameConsents(a: SocialRequest['consents'], b: SocialRequest['consents']) { return a.length === b.length && a.every(c => b.some(other => c.platform === other.platform && c.requester_handle_version === other.requester_handle_version && c.recipient_handle_version === other.recipient_handle_version && c.requester_consents === other.requester_consents)) }
const idleMutation = (): Mutation => ({ phase: 'idle', key: null, operation: null, failure: null })
export function createSocialSharingController(options: SocialOptions) {
  const now = options.now ?? (() => Date.now()), listeners = new Set<() => void>(), cursors = new Set<string>(), restricted = new Set<string>()
  const requestGuards = new Map<string, { minimum: number; matches: (r: SocialRequest) => boolean }>(), handleFloors = new Map<Platform, { minimum: number; configured: boolean }>()
  const initial = (): SocialState => ({ available: false, readOnly: true, userId: null, revealAvailable: false, scopeKey: null, handles: blank(), handleDrafts: { INSTAGRAM: '', X: '' }, requests: blank(), direction: 'INCOMING', selectedId: null, detail: blank(), offer: blank(), requesterConsent: [], approvedPlatforms: [], recipientConsent: false, secret: blank(), mutation: idleMutation(), now: now() })
  let state = freeze(initial()), readVersion = 0, detailVersion = 0, offerVersion = 0, handlesVersion = 0, secretVersion = 0, writeVersion = 0, command: Command | null = null
  let timer: ReturnType<typeof setTimeout> | null = null
  const notify = () => { const snapshot = Array.from(listeners); for (const fn of snapshot) { try { fn() } catch { /* One observer must not prevent another from scrubbing private output. */ } } }
  const publish = (patch: Partial<SocialState>) => { state = freeze({ ...state, ...patch }); notify() }
  const runtime = createRuntime(options, () => {
    readVersion++; detailVersion++; offerVersion++; handlesVersion++; secretVersion++; writeVersion++; command = null; cursors.clear(); restricted.clear(); requestGuards.clear(); handleFloors.clear(); stopTimer(); state = freeze(initial()); notify()
  }, () => {
    publish({ available: runtime.usable(), readOnly: runtime.access().readOnly, userId: runtime.scope()?.userId ?? null, scopeKey: runtime.usable() ? JSON.stringify([runtime.scope(), runtime.lifetime()]) : null, revealAvailable: runtime.usable() && !!options.sensitiveRead,
      ...(!runtime.usable() ? { handles: blank<{ items: HandleState[] }>(), handleDrafts: { INSTAGRAM: '', X: '' }, requesterConsent: [], approvedPlatforms: [], recipientConsent: false, detail: blank<SocialRequest>(), requests: blank<{ items: SocialRequest[]; next_cursor: string | null }>(), offer: blank<SharingOffer>(), secret: blank<PrivateReveal>() } : {}) })
  })
  const unsettled = () => ['sending', 'uncertain', 'pending', 'verifying'].includes(state.mutation.phase)
  const locked = () => command !== null || unsettled() || state.mutation.phase === 'blocked'
  const canWrite = (safety = false) => runtime.usable() && !locked() && (safety || !state.readOnly)
  const current = (scope: PrivacyScope) => options.auth.privacy.isCurrent(scope)
  const participant = (r: SocialRequest, scope: PrivacyScope) => r.event_id === scope.eventId && (r.requester_id === scope.userId || r.recipient_id === scope.userId)
  const verifiedRequest = (r: SocialRequest) => { const guard = requestGuards.get(r.request_id); return !guard || (r.revision >= guard.minimum && guard.matches(r)) }
  const verifiedHandle = (h: HandleState) => { const floor = handleFloors.get(h.platform); return !floor || (h.version >= floor.minimum && (h.version !== floor.minimum || h.configured === floor.configured)) }
  function stopTimer() { if (timer !== null) clearTimeout(timer); timer = null }
  function hideSecret() { secretVersion++; runtime.cancel('secret'); publish({ secret: blank(), now: now() }); schedule() }
  function schedule() {
    stopTimer()
    const dates = [...(state.requests.data?.items ?? []), ...(state.detail.data ? [state.detail.data] : [])].filter(r => ['PENDING', 'APPROVED'].includes(r.state)).map(r => Date.parse(r.expires_at))
    if (state.secret.data) dates.push(Date.parse(state.secret.data.expires_at))
    const next = Math.min(...dates.filter(date => date > now()))
    if (Number.isFinite(next)) timer = setTimeout(expire, Math.min(next - now(), 2_147_483_647))
  }
  function expire() {
    const time = now(), request = state.detail.data, secret = state.secret.data
    if ((secret && Date.parse(secret.expires_at) <= time) || (request && Date.parse(request.expires_at) <= time)) {
      secretVersion++; runtime.cancel('secret'); publish({ secret: blank(), now: time })
    } else publish({ now: time })
    schedule()
  }
  async function loadHandles() {
    if (!runtime.usable() || unsettled()) return
    const version = ++handlesVersion, scope = runtime.scope()!
    publish({ handles: { phase: 'loading', data: null, failure: null }, handleDrafts: { INSTAGRAM: '', X: '' } })
    try {
      const data = await runtime.run('handles', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listOwnConcertHandles', path: `/api/concert/events/${s.eventId}/me/social-handles`, decode: decode(handleList, v => v.items.every(h => h.user_id === s.userId && verifiedHandle(h))) }), false, scope)
      if (version === handlesVersion && current(scope)) publish({ handles: { phase: 'ready', data, failure: null } })
    } catch (error) { if (version === handlesVersion && current(scope)) publish({ handles: { phase: 'error', data: null, failure: failure(error) } }) }
  }
  async function loadRequests(more = false) {
    if (!runtime.usable() || (more && state.requests.phase === 'loading')) return
    const previous = more ? state.requests.data : null, cursor = previous?.next_cursor
    if (more && !cursor) return
    const version = ++readVersion, scope = runtime.scope()!, direction = state.direction
    publish({ requests: { phase: 'loading', data: previous, failure: null } })
    try {
      const data = await runtime.run('requests', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertSocialRequests', path: `/api/concert/events/${s.eventId}/me/social-requests`, query: { direction, limit: 20, ...(cursor ? { cursor } : {}) },
        decode: decode(requestList, page => page.items.every(r => participant(r, s) && verifiedRequest(r) && (direction === 'INCOMING' ? r.recipient_id === s.userId : r.requester_id === s.userId))) }), false, scope)
      if (version !== readVersion || !current(scope)) return
      const items = [...(previous?.items ?? []), ...data.items]
      if (new Set(items.map(r => r.request_id)).size !== items.length || (data.next_cursor && (data.next_cursor === cursor || cursors.has(data.next_cursor)))) throw new Error('Invalid request cursor')
      if (cursor) cursors.add(cursor); if (!more) cursors.clear()
      publish({ requests: { phase: 'ready', data: { items, next_cursor: data.next_cursor }, failure: null } }); schedule()
    } catch (error) { if (version === readVersion && current(scope)) publish({ requests: { phase: 'error', data: null, failure: failure(error) } }) }
  }
  async function loadRequest(id = state.selectedId) {
    if (!runtime.usable() || !id || unsettled()) return false
    const version = ++detailVersion, scope = runtime.scope()!; hideSecret()
    if (!current(scope) || version !== detailVersion) return false
    publish({ selectedId: id, detail: { phase: 'loading', data: null, failure: null }, approvedPlatforms: [], recipientConsent: false })
    try {
      if (!uuid.safeParse(id).success) throw new Error('Invalid request')
      const data = await runtime.run('detail', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertSocialRequest', path: `/api/concert/events/${s.eventId}/social-requests/${id}`, decode: decode(socialRequest, r => r.request_id === id && participant(r, s) && verifiedRequest(r)) }), false, scope)
      if (version !== detailVersion || !current(scope)) return false
      if (!['PENDING', 'APPROVED'].includes(data.state) || Date.parse(data.expires_at) <= now()) restricted.add(id)
      publish({ detail: { phase: 'ready', data, failure: null }, now: now() }); schedule(); return true
    } catch (error) {
      if (version === detailVersion && current(scope)) { restricted.add(id); publish({ detail: { phase: 'error', data: null, failure: failure(error) }, secret: blank() }) }
      return false
    }
  }
  async function loadOffer(introId: string) {
    if (!runtime.usable() || unsettled()) return
    const version = ++offerVersion, scope = runtime.scope()!; hideSecret()
    if (!current(scope) || version !== offerVersion) return
    publish({ offer: { phase: 'loading', data: null, failure: null }, requesterConsent: [] })
    try {
      if (!uuid.safeParse(introId).success) throw new Error('Invalid introduction')
      const data = await runtime.run('offer', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertSharingOffer', path: `/api/concert/events/${s.eventId}/intros/${introId}/sharing-offer`, decode: decode(offer, o => o.intro_id === introId && o.recipient_id !== s.userId) }), false, scope)
      if (version === offerVersion && current(scope)) publish({ offer: { phase: 'ready', data, failure: null } })
    } catch (error) { if (version === offerVersion && current(scope)) publish({ offer: { phase: 'error', data: null, failure: failure(error) } }) }
  }
  function boundReceipt(value: unknown, cmd: Command): Receipt {
    const r = receipt.parse(value)
    if (r.event_id !== cmd.scope.eventId || r.actor_id !== cmd.scope.userId || r.operation_id !== cmd.operation || r.idempotency_key !== cmd.key
      || (cmd.resource && r.resource_id && r.resource_id !== cmd.resource) || Date.parse(r.expires_at) <= Date.parse(r.created_at) || Date.parse(r.expires_at) <= now()
      || (r.failure_reason !== null && !Object.values(DOMAIN_OPERATIONS[cmd.operation].errors).some(reasons => (reasons as readonly string[]).includes(r.failure_reason!)))
      || (r.state === 'PENDING' && (r.result_status !== null || r.failure_reason !== null))
      || (r.state === 'SUCCEEDED' && (r.result_status !== 200 || !r.resource_id || r.resource_revision === null || r.resource_revision <= cmd.revision || r.failure_reason !== null))
      || (r.state === 'FAILED' && (r.failure_reason === null || r.result_status !== null))) throw new Error('Invalid social receipt')
    return r
  }
  function finish(cmd: Command, resourceId: string, minimum: number, data: { handles: { items: HandleState[] } } | { request: SocialRequest }) {
    command = null
    if (cmd.reason) options.auth.privacy.changePolicy(cmd.scope, cmd.reason)
    const scope = runtime.scope()
    if (!scope || !runtime.usable() || scope.generation !== cmd.scope.generation || scope.userId !== cmd.scope.userId || scope.eventId !== cmd.scope.eventId || scope.policyGeneration !== cmd.scope.policyGeneration + (cmd.reason ? 1 : 0)) return false
    if ('handles' in data) { handlesVersion++; runtime.cancel('handles'); handleFloors.set(cmd.platform!, { minimum, configured: cmd.method === 'PUT' }) }
    else { detailVersion++; runtime.cancel('detail'); requestGuards.set(resourceId, { minimum, matches: cmd.requestMatches! }); if (!['PENDING', 'APPROVED'].includes(data.request.state)) restricted.add(resourceId) }
    publish({ mutation: { phase: 'succeeded', key: cmd.key, operation: cmd.operation, failure: null }, handleDrafts: { INSTAGRAM: '', X: '' }, requesterConsent: [], approvedPlatforms: [], recipientConsent: false,
      ...('handles' in data ? { handles: { phase: 'ready' as const, data: data.handles, failure: null } } : { selectedId: resourceId, detail: { phase: 'ready' as const, data: data.request, failure: null } }), now: now() })
    schedule(); return true
  }
  async function confirm(cmd: Command, resourceId: string, minimum: number, version: number) {
    publish({ mutation: { phase: 'verifying', key: cmd.key, operation: cmd.operation, failure: null }, ...(cmd.kind === 'request' ? { selectedId: resourceId, detail: blank<SocialRequest>() } : {}) })
    try {
      let data: { handles: { items: HandleState[] } } | { request: SocialRequest }
      if (cmd.kind === 'handle') {
        const handles = await runtime.run('verification', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'listOwnConcertHandles', path: `/api/concert/events/${s.eventId}/me/social-handles`,
          decode: decode(handleList, page => page.items.every(h => h.user_id === cmd.scope.userId && verifiedHandle(h)) && page.items.some(h => h.platform === cmd.platform && h.version >= minimum && (h.version !== minimum || h.configured === (cmd.method === 'PUT')))) }), false, cmd.scope)
        data = { handles }
      } else {
        const request = await runtime.run('verification', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertSocialRequest', path: `/api/concert/events/${s.eventId}/social-requests/${resourceId}`,
          decode: decode(socialRequest, r => r.request_id === resourceId && participant(r, cmd.scope) && r.revision >= minimum && !!cmd.requestMatches?.(r) && verifiedRequest(r)) }), false, cmd.scope)
        data = { request }
      }
      if (version !== writeVersion || !current(cmd.scope) || command !== cmd) return false
      return finish(cmd, resourceId, minimum, data)
    } catch (error) {
      if (version === writeVersion && current(cmd.scope) && command === cmd) publish({ mutation: { phase: 'uncertain', key: cmd.key, operation: cmd.operation, failure: failure(error) }, ...(cmd.kind === 'request' ? { detail: { phase: 'error', data: null, failure: failure(error) } as Slot<SocialRequest> } : {}) })
      return false
    }
  }
  async function accept(cmd: Command, r: Receipt, version: number) {
    if (r.state === 'SUCCEEDED') return confirm(cmd, r.resource_id!, r.resource_revision!, version)
    if (r.state === 'FAILED') publish({ mutation: { phase: 'blocked', key: cmd.key, operation: cmd.operation, failure: 'failed' } })
    else publish({ mutation: { phase: r.state === 'PENDING' ? 'pending' : 'uncertain', key: cmd.key, operation: cmd.operation, failure: null } })
    return true
  }
  async function dispatch(input: Omit<Command, 'key' | 'scope'>, safety = false) {
    if (!canWrite(safety)) return false
    const cmd = freeze({ ...input, key: crypto.randomUUID(), scope: runtime.scope()! }), version = ++writeVersion; command = cmd
    hideSecret()
    if (version !== writeVersion || !current(cmd.scope) || command !== cmd) return false
    if (input.reason) restricted.add(input.resource ?? '*'); if (input.kind === 'handle') restricted.add('*')
    publish({ mutation: { phase: 'sending', key: cmd.key, operation: cmd.operation, failure: null } })
    if (version !== writeVersion || !current(cmd.scope) || command !== cmd) return false
    try {
      const result = await runtime.run('write', s => ({ method: cmd.method, access: 'protected', errorFamily: 'domain', domainOperation: cmd.operation, idempotencyKey: cmd.key, path: `/api/concert/events/${s.eventId}${cmd.suffix}`,
        ...(cmd.body !== undefined ? { body: { json: cmd.body } } : {}), ...(cmd.query ? { query: cmd.query } : {}),
        decode(value, context): { receipt: Receipt } | { data: HandleState | SocialRequest } {
          if (context.status === 202) { const r = boundReceipt(value, cmd); if (r.state !== 'PENDING') throw new Error('Expected pending receipt'); return { receipt: r } }
          return { data: cmd.check(value) }
        } }), false, cmd.scope)
      if (version !== writeVersion || !current(cmd.scope)) return false
      if ('receipt' in result) return accept(cmd, result.receipt, version)
      return confirm(cmd, 'request_id' in result.data ? result.data.request_id : result.data.user_id, 'revision' in result.data ? result.data.revision : result.data.version, version)
    } catch (error) {
      if (version !== writeVersion || !current(cmd.scope)) return false
      const uncertain = !(error instanceof TransportError) || error.uncertain
      if (!uncertain) command = null
      publish({ mutation: { phase: uncertain ? 'uncertain' : 'failed', key: cmd.key, operation: cmd.operation, failure: failure(error) } }); return false
    }
  }
  function currentRequest(expected?: 'PENDING' | 'APPROVED') {
    const r = state.detail.data
    return r && state.detail.phase === 'ready' && (!expected || r.state === expected) && Date.parse(r.expires_at) > now() ? r : null
  }
  function resultRequest(original: SocialRequest, stateExpected: SocialRequest['state'], approved?: readonly Platform[]) {
    return (value: unknown) => {
      const r = socialRequest.parse(value)
      if (!participant(r, runtime.scope()!) || r.request_id !== original.request_id || r.requester_id !== original.requester_id || r.recipient_id !== original.recipient_id
        || r.intro_id !== original.intro_id || r.intro_revision !== original.intro_revision || r.revision <= original.revision || r.state !== stateExpected
        || !sameConsents(r.consents, original.consents) || (approved && (r.approved_platforms.length !== approved.length || !r.approved_platforms.every(p => approved.includes(p))))) throw new Error('Invalid command result')
      return r
    }
  }
  function requestBinding(original: Pick<SocialRequest, 'event_id' | 'requester_id' | 'recipient_id' | 'intro_id' | 'intro_revision' | 'consents'> & { request_id?: string }, expected: SocialRequest['state'], approved?: readonly Platform[]) {
    const reachable = expected === 'PENDING' ? ['PENDING', 'APPROVED', 'REJECTED', 'CANCELLED', 'REVOKED', 'EXPIRED'] : expected === 'APPROVED' ? ['APPROVED', 'REVOKED', 'EXPIRED'] : [expected]
    return (r: SocialRequest) => (!original.request_id || r.request_id === original.request_id) && r.event_id === original.event_id && r.requester_id === original.requester_id && r.recipient_id === original.recipient_id
      && r.intro_id === original.intro_id && r.intro_revision === original.intro_revision && sameConsents(r.consents, original.consents) && reachable.includes(r.state)
      && (!approved || (r.approved_platforms.length === approved.length && r.approved_platforms.every(p => approved.includes(p))))
  }
  return {
    read: () => state, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } },
    start() { runtime.start(); globalThis.document?.addEventListener('visibilitychange', expire); globalThis.window?.addEventListener('focus', expire) },
    dispose() { stopTimer(); globalThis.document?.removeEventListener('visibilitychange', expire); globalThis.window?.removeEventListener('focus', expire); runtime.dispose() },
    loadHandles, loadRequests, loadRequest, loadOffer, hideSecret, expire,
    setHandleDraft(p: Platform, value: string) { if (canWrite() && ['X', 'INSTAGRAM'].includes(p)) publish({ handleDrafts: { ...state.handleDrafts, [p]: value } }) },
    saveHandle(p: Platform) {
      const h = state.handles.data?.items.find(h => h.platform === p), text = state.handleDrafts[p]?.trim().toLowerCase()
      if (!h || !text || !validHandle(p, text)) return Promise.resolve(false)
      return dispatch({ operation: 'setConcertHandle', method: 'PUT', suffix: `/me/social-handles/${p}`, body: { expected_revision: h.version, handle: text }, resource: state.userId!, revision: h.version, kind: 'handle', platform: p, reason: 'handle-replaced',
        check(value) { const r = handleState.parse(value); if (r.user_id !== state.userId || r.platform !== p || r.version <= h.version || !r.configured) throw new Error('Wrong handle result'); return r } })
    },
    removeHandle(p: Platform) {
      const h = state.handles.data?.items.find(h => h.platform === p)
      if (!h?.configured) return Promise.resolve(false)
      return dispatch({ operation: 'removeConcertHandle', method: 'DELETE', suffix: `/me/social-handles/${p}`, query: { expected_revision: h.version }, resource: state.userId!, revision: h.version, kind: 'handle', platform: p, reason: 'handle-replaced',
        check(value) { const r = handleState.parse(value); if (r.user_id !== state.userId || r.platform !== p || r.version <= h.version || r.configured) throw new Error('Wrong removed handle'); return r } }, true)
    },
    setDirection(direction: Direction) { if (unsettled() || !['INCOMING', 'OUTGOING'].includes(direction)) return; readVersion++; runtime.cancel('requests'); cursors.clear(); publish({ direction, requests: blank() }); void loadRequests() },
    setRequesterConsent(p: Platform, checked: boolean) { if (!canWrite()) return; publish({ requesterConsent: checked ? [...new Set([...state.requesterConsent, p])] : state.requesterConsent.filter(value => value !== p) }) },
    createRequest() {
      const o = state.offer.data, selected = state.requesterConsent, own = state.handles.data?.items
      if (!o || !selected.length || !own || selected.some(p => !o.available_handles.some(h => h.platform === p) || !own.some(h => h.platform === p && h.configured))) return Promise.resolve(false)
      const consents = selected.map(p => ({ platform: p, requester_handle_version: own.find(h => h.platform === p)!.version, recipient_handle_version: o.available_handles.find(h => h.platform === p)!.version, requester_consents: true as const }))
      return dispatch({ operation: 'createConcertSocialRequest', method: 'POST', suffix: '/social-requests', body: { recipient_id: o.recipient_id, intro_id: o.intro_id, intro_revision: o.intro_revision, consents }, resource: null, revision: 0, kind: 'request',
        requestMatches: requestBinding({ event_id: runtime.scope()!.eventId!, requester_id: state.userId!, recipient_id: o.recipient_id, intro_id: o.intro_id, intro_revision: o.intro_revision, consents }, 'PENDING'),
        check(value) { const r = socialRequest.parse(value); if (!participant(r, runtime.scope()!) || r.requester_id !== state.userId || r.recipient_id !== o.recipient_id || r.intro_id !== o.intro_id || r.intro_revision !== o.intro_revision || r.state !== 'PENDING' || !sameConsents(r.consents, consents)) throw new Error('Wrong created request'); return r } })
    },
    setDecisionPlatform(p: Platform, checked: boolean) { if (!canWrite()) return; publish({ approvedPlatforms: checked ? [...new Set([...state.approvedPlatforms, p])] : state.approvedPlatforms.filter(value => value !== p) }) },
    setRecipientConsent(checked: boolean) { if (canWrite()) publish({ recipientConsent: checked }) },
    decide(approve: boolean) {
      const r = currentRequest('PENDING'), selected = approve ? state.approvedPlatforms : []
      if (!r || r.recipient_id !== state.userId || (approve && (!state.recipientConsent || !selected.length || selected.some(p => !r.consents.some(c => c.platform === p))))) return Promise.resolve(false)
      return dispatch({ operation: 'decideConcertSocialRequest', method: 'POST', suffix: `/social-requests/${r.request_id}/decision`, body: { expected_revision: r.revision, decision: approve ? 'APPROVED' : 'REJECTED', approved_platforms: selected, recipient_consents: approve }, resource: r.request_id, revision: r.revision, kind: 'request', check: resultRequest(r, approve ? 'APPROVED' : 'REJECTED', selected), requestMatches: requestBinding(r, approve ? 'APPROVED' : 'REJECTED', selected) })
    },
    cancelRequest() {
      const r = currentRequest('PENDING'); if (!r || r.requester_id !== state.userId) return Promise.resolve(false)
      return dispatch({ operation: 'cancelConcertSocialRequest', method: 'DELETE', suffix: `/social-requests/${r.request_id}`, query: { expected_revision: r.revision }, resource: r.request_id, revision: r.revision, kind: 'request', reason: 'consent-revoked', check: resultRequest(r, 'CANCELLED'), requestMatches: requestBinding(r, 'CANCELLED') }, true)
    },
    revokeRequest() {
      const r = currentRequest(); if (!r || !['PENDING', 'APPROVED'].includes(r.state)) return Promise.resolve(false)
      return dispatch({ operation: 'revokeConcertSocialRequest', method: 'POST', suffix: `/social-requests/${r.request_id}/revoke`, body: { expected_revision: r.revision }, resource: r.request_id, revision: r.revision, kind: 'request', reason: 'consent-revoked', check: resultRequest(r, 'REVOKED'), requestMatches: requestBinding(r, 'REVOKED') }, true)
    },
    async revealHandle(p: Platform) {
      hideSecret(); const r = currentRequest('APPROVED')
      if (!runtime.usable() || locked() || !options.sensitiveRead || !r || !r.approved_platforms.includes(p) || restricted.has('*') || restricted.has(r.request_id)) return false
      const consent = r.consents.find(c => c.platform === p)!, owner = r.requester_id === state.userId ? r.recipient_id : r.requester_id
      const expectedVersion = owner === r.requester_id ? consent.requester_handle_version : consent.recipient_handle_version
      const scope = runtime.scope()!, version = ++secretVersion
      publish({ secret: { phase: 'loading', data: null, failure: null } })
      try {
        const data = await runtime.run('secret', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'revealConcertSocialHandle', path: `/api/concert/events/${s.eventId}/social-requests/${r.request_id}/reveal`, query: { platform: p },
          decode: decode(reveal, v => v.request_id === r.request_id && v.platform === p && v.owner_id === owner && v.handle_version === expectedVersion && Date.parse(v.expires_at) > now() && Date.parse(v.expires_at) <= Date.parse(r.expires_at)) }), true, scope)
        if (version !== secretVersion || !current(scope) || restricted.has('*') || restricted.has(r.request_id) || Date.parse(r.expires_at) <= now() || Date.parse(data.expires_at) <= now()) { if (version === secretVersion && current(scope)) hideSecret(); return false }
        publish({ secret: { phase: 'ready', data, failure: null }, now: now() }); schedule(); return true
      } catch (error) { if (version === secretVersion && current(scope)) publish({ secret: { phase: 'error', data: null, failure: failure(error) } }); return false }
    },
    async checkStatus() {
      const cmd = command; if (!cmd || !['pending', 'uncertain'].includes(state.mutation.phase) || !runtime.usable()) return false
      const version = ++writeVersion
      try {
        const r = await runtime.run('status', s => ({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus', path: `/api/concert/events/${s.eventId}/commands`, query: { operation_id: cmd.operation, idempotency_key: cmd.key }, decode: value => boundReceipt(value, cmd) }), false, cmd.scope)
        if (version === writeVersion && current(cmd.scope)) return accept(cmd, r, version)
        return false
      } catch (error) { if (version === writeVersion && current(cmd.scope)) publish({ mutation: { ...state.mutation, phase: 'uncertain', failure: failure(error) } }); return false }
    },
  }
}
export type SocialSharingController = ReturnType<typeof createSocialSharingController>
