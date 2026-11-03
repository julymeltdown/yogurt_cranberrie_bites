import { z } from 'zod'
import { blank, createSocialRuntime, failure, type Slot, type SocialOptions } from '../people/runtime'
import { freeze, timestamp, uuid } from '../people/contract'
import { createCommand, idleCommand, type CommandState } from '../intro/command'
import type { PrivacyScope } from '../../shared/query/privacy'
export const ageSchema = z.strictObject({ user_id: uuid, event_id: uuid, status: z.enum(['UNKNOWN', 'VERIFIED', 'EXPIRED', 'UNSUPPORTED', 'PROVIDER_UNAVAILABLE', 'REVOKED']), verified_at: timestamp.nullable(), expires_at: timestamp.nullable(), verification_available: z.boolean() })
export type AgeStatus = z.infer<typeof ageSchema>
export function decodeAge(value: unknown, scope: PrivacyScope): AgeStatus { const row = ageSchema.parse(value); if (row.user_id !== scope.userId || row.event_id !== scope.eventId || row.status === 'VERIFIED' && (!row.verified_at || !row.expires_at || Date.parse(row.verified_at) >= Date.parse(row.expires_at))) throw new Error('Invalid age status'); return freeze(row) }
const pendingSchema = z.strictObject({ verification_id: uuid, state: z.literal('PENDING'), expires_at: timestamp, authorization_url: z.string().max(2048).url().regex(/^https:\/\//) })
type Pending = z.infer<typeof pendingSchema>
export interface AgeOptions extends SocialOptions { allowVerificationOrigin?: (origin: string) => boolean }
export interface AgeState { status: Slot<AgeStatus>; effective: AgeStatus['status']; command: CommandState; available: boolean; readOnly: boolean; pending: Pending | null; message: string }
export function createAgeStatusController(options: AgeOptions) {
  const initial = (): AgeState => ({ status: blank(), effective: 'UNKNOWN', command: idleCommand(), available: false, readOnly: true, pending: null, message: '' })
  let state = freeze(initial()), initialized = false, version = 0, expiry: ReturnType<typeof setTimeout> | undefined, pendingScope: PrivacyScope | null = null
  const listeners = new Set<() => void>(), now = options.now ?? Date.now
  const emit = (patch: Partial<AgeState>) => { state = freeze({ ...state, ...patch }); [...listeners].forEach(fn => { try { fn() } catch { /* Isolate observers so privacy cleanup reaches every subscriber. */ } }) }
  const runtime = createSocialRuntime(options, () => { version++; initialized = false; clearTimeout(expiry); pendingScope = null; state = freeze(initial()); command.reset() }, () => { if (!runtime.usable()) { version++; initialized = false; clearTimeout(expiry); pendingScope = null; command.interrupt(); emit({ status: blank(), pending: null, effective: 'UNKNOWN' }) }; emit({ available: runtime.usable(), readOnly: runtime.readOnly() }); if (runtime.usable() && !initialized) { initialized = true; void load() } })
  function effective(row: AgeStatus): AgeStatus['status'] { return row.status !== 'VERIFIED' ? row.status : !row.verified_at || Date.parse(row.verified_at) > now() ? 'UNKNOWN' : !row.expires_at || Date.parse(row.expires_at) <= now() ? 'EXPIRED' : 'VERIFIED' }
  function accept(row: AgeStatus | Pending) {
    clearTimeout(expiry)
    if ('verification_id' in row) { pendingScope = runtime.scope(); emit({ pending: row, message: 'Verification is pending with the configured provider. No claim has been granted.' }); schedule(); return }
    pendingScope = null; emit({ status: { phase: 'ready', data: row, failure: null }, effective: effective(row), pending: null, message: '' }); schedule()
  }
  function schedule() {
    clearTimeout(expiry); const captured = runtime.scope(); if (!captured) return
    const deadlines = [state.status.data?.status === 'VERIFIED' ? state.status.data.expires_at : null, state.pending?.expires_at].filter((v): v is string => !!v).map(Date.parse).filter(v => v > now())
    if (!deadlines.length) return
    expiry = setTimeout(() => { if (!runtime.current(captured)) return
      if (state.status.data) emit({ effective: effective(state.status.data) })
      if (state.pending && Date.parse(state.pending.expires_at) <= now()) { pendingScope = null; emit({ pending: null, message: 'The verification session expired. No verification was granted.' }) }
      schedule()
    }, Math.min(2_147_483_647, Math.max(1, Math.min(...deadlines) - now())))
  }
  async function fetchAge() { return runtime.run('age-status', scope => ({ path: `/api/concert/events/${scope.eventId}/me/age-status`, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertAgeStatus', decode: value => decodeAge(value, scope) })) }
  const command = createCommand<AgeStatus | Pending>(runtime, options, c => emit({ command: c }), accept, async () => {
    // Receipt metadata cannot manufacture one-shot redirect/state tokens. Recheck safe status only.
    const row = await fetchAge(); return row
  })
  async function load() { if (!runtime.usable() || state.status.phase === 'loading' || state.command.locked) return
    const captured = runtime.scope()!, attempt = ++version; emit({ status: { phase: 'loading', data: null, failure: null }, effective: 'UNKNOWN' })
    try { const row = await fetchAge(); if (attempt === version && runtime.current(captured)) accept(row) }
    catch (error) { if (attempt === version && runtime.current(captured)) emit({ status: { phase: 'error', data: null, failure: failure(error) }, effective: 'UNKNOWN', message: 'Age verification status is unavailable. Restricted access is not granted.' }) }
  }
  return { read: () => state, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } }, start: runtime.start, load, checkStatus: command.check,
    canVerify() { return runtime.writable() && !!options.allowVerificationOrigin && state.status.phase === 'ready' && state.status.data?.verification_available === true && state.pending === null && !state.command.locked },
    startVerification() {
      if (!runtime.writable() || !options.allowVerificationOrigin || state.status.phase !== 'ready' || !state.status.data?.verification_available || state.pending || state.command.locked) return Promise.resolve(false)
      return command.submit({ operation: 'startConcertAgeVerification', path: `/api/concert/events/${runtime.scope()!.eventId}/me/age-verifications`, method: 'POST', target: null, json: { return_target: 'AGE_PRIVACY' }, decode(value) {
        const pending = pendingSchema.parse(value), url = new URL(pending.authorization_url)
        if (url.username || url.password || url.hash || !options.allowVerificationOrigin!(url.origin) || Date.parse(pending.expires_at) <= now() || Date.parse(pending.expires_at) > now() + 600_000) throw new Error('Unapproved verifier')
        return freeze(pending)
      } })
    },
    async completeVerification(input: { verification_id: string; state_token: string; provider_response_token: string }) {
      if (!pendingScope || !runtime.current(pendingScope) || !state.pending || state.pending.verification_id !== input.verification_id || Date.parse(state.pending.expires_at) <= now()) return false
      const data = z.strictObject({ verification_id: uuid, state_token: z.string().min(1).max(512), provider_response_token: z.string().min(1).max(4096) }).safeParse(input)
      if (!data.success) return false
      const captured = pendingScope
      return command.submit({ operation: 'completeConcertAgeVerification', path: `/api/concert/events/${captured.eventId}/me/age-verifications/callback`, method: 'POST', target: null, json: { ...data.data }, decode: value => decodeAge(value, captured) })
    },
    dispose() { clearTimeout(expiry); runtime.dispose() },
  }
}
export type AgeStatusController = ReturnType<typeof createAgeStatusController>
