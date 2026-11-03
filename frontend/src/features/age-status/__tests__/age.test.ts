// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAgeStatusController, type AgeStatus } from '../controller'
import { deferred, eventId, harness, userId } from '../../people/__tests__/fixtures/harness'
const owned: (() => void)[] = []
afterEach(() => { owned.splice(0).forEach(cleanup => cleanup()); vi.useRealTimers() })
const now = Date.parse('2026-11-07T12:00:00Z'), verificationId = '00000000-0000-4000-8000-000000000050'
const status = (value: AgeStatus['status'] = 'UNKNOWN'): AgeStatus => ({ user_id: userId, event_id: eventId, status: value, verified_at: value === 'VERIFIED' ? '2026-11-07T08:00:00Z' : null, expires_at: value === 'VERIFIED' ? '2026-11-07T13:00:00Z' : null, verification_available: value !== 'UNSUPPORTED' && value !== 'PROVIDER_UNAVAILABLE' })
async function setup(value: AgeStatus = status(), allow = true) {
  const h = harness(), controller = createAgeStatusController({ ...h.options, now: () => now, ...(allow ? { allowVerificationOrigin: (origin: string) => origin === 'https://verifier.example' } : {}) }); h.respond(async () => Response.json(value)); owned.push(() => { controller.dispose(); h.dispose() }); controller.start(); await vi.waitFor(() => expect(controller.read().status.phase).toBe('ready')); return { h, controller }
}
const pending = (url = 'https://verifier.example/start') => ({ verification_id: verificationId, state: 'PENDING', expires_at: '2026-11-07T12:10:00Z', authorization_url: url })
describe('FE30 private canonical age status', () => {
  it.each(['UNKNOWN', 'VERIFIED', 'EXPIRED', 'UNSUPPORTED', 'PROVIDER_UNAVAILABLE', 'REVOKED'] as const)('renders only canonical %s status without any exact age', async value => {
    const { controller } = await setup(status(value)); expect(controller.read().effective).toBe(value); expect(JSON.stringify(controller.read())).not.toContain('age_years'); expect(controller.canVerify()).toBe(!['UNSUPPORTED', 'PROVIDER_UNAVAILABLE'].includes(value))
  })
  it('expires verified status locally at validity boundary and does not invent eligibility', async () => {
    vi.useFakeTimers(); vi.setSystemTime(now); const h = harness(), controller = createAgeStatusController({ ...h.options, now: Date.now }); owned.push(() => { controller.dispose(); h.dispose() }); h.respond(async () => Response.json({ ...status('VERIFIED'), expires_at: '2026-11-07T12:00:01Z' })); controller.start(); await vi.advanceTimersByTimeAsync(0); expect(controller.read().effective).toBe('VERIFIED'); await vi.advanceTimersByTimeAsync(1000); expect(controller.read().effective).toBe('EXPIRED')
  })
  it('has no provider defaults and never starts in read-only or unsupported mode', async () => {
    const { h, controller } = await setup(status(), false); expect(controller.canVerify()).toBe(false); expect(await controller.startVerification()).toBe(false); expect(h.requests).toHaveLength(1)
    const other = await setup(); other.h.access({ readOnly: true }); expect(await other.controller.startVerification()).toBe(false); expect(other.h.requests).toHaveLength(1)
  })
  it.each(['http://verifier.example/start', 'https://evil.example/start', 'https://user:pass@verifier.example/start', 'https://verifier.example/start#secret'])('rejects unapproved provider redirect %s without granting verification', async url => {
    const { h, controller } = await setup(); h.respond(async () => Response.json(pending(url))); expect(await controller.startVerification()).toBe(false); expect(controller.read().pending).toBeNull(); expect(controller.read().effective).toBe('UNKNOWN'); expect(controller.read().command.locked).toBe(true)
  })
  it('binds callback to pending user/session and sends proof tokens without client age', async () => {
    const { h, controller } = await setup(); h.respond(async () => Response.json(pending())); expect(await controller.startVerification()).toBe(true); expect(controller.read().effective).toBe('UNKNOWN')
    expect(await controller.completeVerification({ verification_id: userId, state_token: 'state', provider_response_token: 'proof' })).toBe(false)
    h.respond(async () => Response.json(status('VERIFIED'))); expect(await controller.completeVerification({ verification_id: verificationId, state_token: 'state', provider_response_token: 'proof' })).toBe(true)
    expect(await h.requests.at(-1)!.json()).toEqual({ verification_id: verificationId, state_token: 'state', provider_response_token: 'proof' }); expect(controller.read().effective).toBe('VERIFIED'); expect(JSON.stringify(controller.read())).not.toContain('proof')
  })
  it('clears pending redirect and drops old status across policy/identity changes', async () => {
    const { h, controller } = await setup(); h.respond(async () => Response.json(pending())); await controller.startVerification(); const late = deferred<Response>(); h.respond(() => late.promise); const response = controller.load(); h.privacy.changePolicy(h.privacy.capture(), 'suspended'); expect(controller.read().pending).toBeNull(); expect(controller.read().status.data).toBeNull(); late.resolve(Response.json(status('VERIFIED'))); await response; expect(controller.read().effective).toBe('UNKNOWN')
  })
  it('treats forged/private extended age DTOs and unavailable reads as unknown', async () => {
    const { h, controller } = await setup(); h.respond(async () => Response.json({ ...status('VERIFIED'), exact_age: 25 })); await controller.load(); expect(controller.read().effective).toBe('UNKNOWN'); expect(controller.read().status.phase).toBe('error')
  })
})
