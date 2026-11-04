import type { Decoder } from '../../../shared/api/http'
import { TransportError, type JsonValue } from '../../../shared/api/errors'
import { freeze, uuid } from '../event/model/contract'
import { createOperatorGuard } from '../event/model/lease'
import type { OperationsOptions } from '../event/model/ports'
import { operation, policyDecision, policyInput, policyReceipt, type PolicyDecision, type PolicyInput, type PolicyReceipt } from './contract'

export type ServicePolicyOptions = Pick<OperationsOptions, 'auth' | 'authority' | 'clock' | 'newKey'>
export interface PolicyDraft {
  readonly action: 'PUBLISH' | 'WITHDRAW'
  readonly mode: 'NORMAL' | 'MAINTENANCE'
  readonly minimumClientVersion: string
  readonly expiresAt: string
  readonly reason: string
}
export type PolicyStage = 'review' | 'sending' | 'pending' | 'unknown' | 'succeeded' | 'failed' | 'conflict' | 'retired' | 'expired'
export interface PolicyCommand {
  readonly stage: PolicyStage
  readonly input: PolicyInput
  readonly key: string
  readonly observedEventRevision: number
  readonly receipt: PolicyReceipt | null
  readonly message: string
}
export interface ServicePolicyState {
  readonly access: 'ready' | 'withdrawn'
  readonly eventId: string | null
  readonly loading: boolean
  readonly decision: PolicyDecision | null
  readonly draft: PolicyDraft
  readonly command: PolicyCommand | null
  readonly message: string | null
}
const blank = (): PolicyDraft => ({ action: 'PUBLISH', mode: 'NORMAL', minimumClientVersion: '', expiresAt: '', reason: '' })
const empty = (): ServicePolicyState => ({ access: 'withdrawn', eventId: null, loading: false, decision: null, draft: blank(), command: null, message: null })
const unresolved = (stage: PolicyStage) => ['sending', 'pending', 'unknown', 'retired', 'expired'].includes(stage)

/** Borrowed Admin-owner lifetime. No role producer, private cache, retry loop or credential store. */
export function createServicePolicyController(options: ServicePolicyOptions) {
  const { auth } = options, listeners = new Set<() => void>()
  let state = freeze(empty()), disposed = false, sequence = 0, expiry: ReturnType<typeof setTimeout> | undefined
  let eventHigh = 0, policyHigh = 0, lastDecision: PolicyDecision | null = null
  function emit(patch: Partial<ServicePolicyState>) {
    state = freeze({ ...state, ...patch })
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent consumers. */ } }
  }
  const guard = createOperatorGuard(options, () => { sequence++; clearTimeout(expiry); eventHigh = 0; policyHigh = 0; lastDecision = null; emit(empty()) })
  if (guard.valid()) emit({ access: 'ready', eventId: guard.scope.eventId })
  const prefix = `/api/concert/events/${guard.scope.eventId}`
  function alive() {
    if (disposed) return false
    if (!guard.valid()) { guard.withdraw(); return false }
    return true
  }
  function fresh() {
    if (!alive()) return false
    const decision = state.decision
    if (decision && Date.parse(decision.valid_until) <= guard.now()!) {
      clearTimeout(expiry); emit({ decision: null, message: 'The policy decision expired. Refresh before reviewing another change.' })
    }
    return state.decision !== null
  }
  function context<T>(decode: (value: unknown, status: number) => T, path: string): Decoder<T> {
    return (value, ctx) => {
      guard.assert()
      if (ctx.path !== prefix + path || !auth.session.isCurrent(ctx.scope) || ctx.scope.userId !== guard.scope.userId
        || ctx.scope.eventId !== guard.scope.eventId) throw Error('Unbound policy response')
      return decode(value, ctx.status)
    }
  }
  async function read<T>(id: 'getConcertServicePolicy' | 'getConcertCommandStatus', path: string, decode: Decoder<T>, query?: Record<string, string>) {
    guard.assert()
    return auth.privacy.run(guard.scope, async ({ signal }) => {
      const result = await auth.request({ path: prefix + path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: id, signal, decode, ...(query ? { query } : {}) })
      guard.assert(); return result.data
    }, guard.signal)
  }
  function feedback(error: unknown) {
    if (error instanceof TransportError) {
      if (error.status === 401 || error.status === 403) { guard.withdraw(); return 'Current operator access was withdrawn.' }
      if (error.server?.reason === 'VERSION_CONFLICT') return 'The policy revision changed. Refresh, compare your retained draft, and review again.'
      if (error.status === 429) return 'Too many requests. Wait before an explicit retry; no command was replayed.'
      if (error.server?.reason === 'EVENT_READ_ONLY') return 'The server rejected this change because the event is read only.'
    }
    return 'Current policy information is unavailable. No publication has been assumed.'
  }
  function setStage(stage: PolicyStage, message: string, receipt: PolicyReceipt | null = state.command?.receipt ?? null) {
    if (state.command) emit({ command: { ...state.command, stage, message, receipt } })
  }
  function adopt(receipt: PolicyReceipt) {
    // A receipt proves command outcome, never a fresh service/admission grant.
    const expired = Date.parse(receipt.expires_at) <= guard.now()!
    const stage = expired ? 'expired' : receipt.state === 'SUCCEEDED' ? 'succeeded' : receipt.state === 'FAILED' ? receipt.failure_reason === 'VERSION_CONFLICT' ? 'conflict' : 'failed'
      : receipt.state === 'RETIRED' ? 'retired' : 'pending'
    const messages = { expired: 'Receipt retention expired. No replay or replacement key is authorized.',
      succeeded: 'The server recorded this command as succeeded. Refresh to inspect the current policy decision; this receipt does not grant service access.',
      failed: 'The server recorded failure. Review the failure and refresh current policy before another explicit change.',
      conflict: 'The server recorded a revision conflict without publication. Refresh, compare your retained draft, and review again.',
      retired: 'The receipt is retired. No replay or replacement key is authorized.',
      pending: 'The command is pending. Check its status explicitly; nothing was replayed.' }
    if (receipt.state === 'SUCCEEDED') eventHigh = Math.max(eventHigh, receipt.resource_revision!)
    clearTimeout(expiry); emit({ decision: null }); setStage(stage, messages[stage], receipt)
  }
  async function refresh() {
    if (!alive() || state.loading || state.command?.stage === 'review' || state.command?.stage === 'sending') return
    const attempt = ++sequence
    clearTimeout(expiry); emit({ loading: true, decision: null, message: null })
    try {
      const decision = await read('getConcertServicePolicy', '/service-policy', context((value, status) => {
        if (status !== 200) throw Error('Invalid policy status')
        return policyDecision(value, guard.now()!)
      }, '/service-policy'))
      if (!alive() || attempt !== sequence) return
      const revision = decision.policy_revision ?? 0
      if (decision.event_revision < eventHigh || revision < policyHigh
        || state.command?.stage === 'succeeded' && revision <= state.command.input.expected_policy_revision
        || lastDecision && revision === policyHigh && lastDecision.minimum_client_version !== null
          && decision.minimum_client_version !== null && lastDecision.minimum_client_version !== decision.minimum_client_version) throw Error('Regressed immutable policy')
      eventHigh = decision.event_revision; policyHigh = revision; lastDecision = decision
      emit({ loading: false, decision })
      expiry = setTimeout(() => { fresh() }, Math.max(1, Date.parse(decision.valid_until) - guard.now()!))
    } catch (error) { const message = feedback(error); if (alive() && attempt === sequence) emit({ loading: false, decision: null, message }) }
  }
  function edit(patch: Partial<PolicyDraft>) {
    if (!alive() || state.loading || state.command !== null) return
    const draft = { ...state.draft, ...patch }
    if (!['PUBLISH', 'WITHDRAW'].includes(draft.action) || !['NORMAL', 'MAINTENANCE'].includes(draft.mode)
      || [draft.minimumClientVersion, draft.expiresAt, draft.reason].some(v => typeof v !== 'string' || v.length > 2000)) return
    emit({ draft, message: null })
  }
  function review() {
    if (!fresh() || state.loading || state.command) return false
    try {
      const d = state.draft
      const input = policyInput({ expected_policy_revision: state.decision!.policy_revision ?? 0, reason: d.reason,
        change: d.action === 'WITHDRAW' ? { action: 'WITHDRAW' } : { action: 'PUBLISH', mode: d.mode, minimum_client_version: d.minimumClientVersion, expires_at: d.expiresAt } }, guard.now()!)
      const key = (options.newKey ?? (() => crypto.randomUUID()))(); uuid.parse(key)
      emit({ command: { stage: 'review', input, key, observedEventRevision: state.decision!.event_revision, receipt: null, message: 'Confirm this exact event, revision and change. Nothing has been sent.' }, message: null }); return true
    } catch { emit({ message: 'Review the canonical version, trimmed reason (at most 500 UTF-8 bytes), and future UTC expiry within 24 hours.' }); return false }
  }
  function cancel() { if (alive() && state.command?.stage === 'review') emit({ command: null, message: 'Confirmation cancelled. Nothing was sent; your draft is retained.' }) }
  function dismiss() {
    if (!alive() || !state.command || unresolved(state.command.stage) || state.command.stage === 'review' || !fresh()) return
    emit({ command: null, message: 'Compare your retained draft with the current policy revision before reviewing another change.' })
  }
  async function confirm() {
    const command = state.command
    if (!command || command.stage !== 'review' || !alive() || state.loading) return
    if (!fresh() || (state.decision!.policy_revision ?? 0) !== command.input.expected_policy_revision) {
      emit({ command: null, message: 'Refresh current policy and review this draft again.' }); return
    }
    try { policyInput(command.input, guard.now()!) } catch { emit({ command: null, message: 'Publication expiry changed or elapsed. Review your draft again.' }); return }
    const attempt = ++sequence
    setStage('sending', 'Sending once. Do not repeat while the outcome is unresolved.')
    const binding = { actor: guard.scope.userId!, event: guard.scope.eventId!, key: command.key, eventRevision: command.observedEventRevision }
    try {
      const receipt = await auth.privacy.run(guard.scope, async ({ signal }) => {
        guard.assert()
        const result = await auth.request({ path: prefix + '/admin/service-policy', method: 'PUT', access: 'protected', errorFamily: 'domain', domainOperation: operation,
          idempotencyKey: command.key, body: { json: command.input as JsonValue }, signal,
          decode: context((value, status) => {
            if (status !== 200 && status !== 202) throw Error('Invalid command status')
            const r = policyReceipt(value, binding, guard.now()!)
            if (status === 202 && r.state !== 'PENDING') throw Error('Invalid pending command')
            if (status === 200 && !['SUCCEEDED', 'FAILED'].includes(r.state)) throw Error('Invalid terminal command')
            return r
          }, '/admin/service-policy') })
        guard.assert(); return result.data
      }, guard.signal)
      if (alive() && attempt === sequence && state.command?.key === command.key) adopt(receipt)
    } catch (error) {
      const message = feedback(error)
      if (!alive() || attempt !== sequence || state.command?.key !== command.key) return
      const reason = error instanceof TransportError ? error.server?.reason : null
      clearTimeout(expiry); emit({ decision: null })
      if (reason === 'VERSION_CONFLICT') setStage('conflict', message)
      else if (reason === 'IDEMPOTENCY_EXPIRED') setStage('expired', 'Receipt retention expired. No replay or replacement key is authorized.')
      else if (reason === 'IDEMPOTENCY_CONFLICT' || reason === 'RESOURCE_UNAVAILABLE') setStage('unknown', 'Command identity cannot be qualified. Check status; no replay or new key is authorized.')
      else if (error instanceof TransportError && error.status !== null && error.status < 500 && !error.uncertain) setStage('failed', message)
      else setStage('unknown', 'The outcome is unknown. Check command status before any further change; nothing will be replayed automatically.')
    }
  }
  async function checkStatus() {
    const command = state.command
    if (!command || !alive() || state.loading || !['pending', 'unknown'].includes(command.stage)) return
    const attempt = ++sequence; emit({ loading: true, message: null })
    try {
      const receipt = await read('getConcertCommandStatus', '/commands', context((value, status) => {
        if (status !== 200) throw Error('Invalid status read')
        const r = policyReceipt(value, { actor: guard.scope.userId!, event: guard.scope.eventId!, key: command.key, eventRevision: command.observedEventRevision }, guard.now()!)
        if (r.state === 'RETIRED') throw Error('Unavailable receipt')
        if (command.receipt && (r.receipt_id !== command.receipt.receipt_id || r.created_at !== command.receipt.created_at || r.expires_at !== command.receipt.expires_at)) throw Error('Replaced command receipt')
        return r
      }, '/commands'), { operation_id: operation, idempotency_key: command.key })
      if (alive() && attempt === sequence && state.command?.key === command.key) adopt(receipt)
    } catch (error) { const message = feedback(error); if (alive() && attempt === sequence) emit({ message: `Status remains unresolved. ${message}` }) }
    finally { if (alive() && attempt === sequence) emit({ loading: false }) }
  }
  guard.own(() => options.clock?.subscribe(() => { fresh() }) ?? (() => {}))
  return { read: () => state, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    refresh, edit, review, cancel, dismiss, confirm, checkStatus,
    dispose() { if (disposed) return; disposed = true; guard.withdraw(); listeners.clear() },
  }
}
export type ServicePolicyController = ReturnType<typeof createServicePolicyController>
