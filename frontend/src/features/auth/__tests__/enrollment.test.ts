import { afterEach, expect, it, vi } from 'vitest'
import { TransportError, parseServerError } from '../../../shared/api/errors'
import { decodePolicy, decodeAdmission, validEnrollmentPassword, validVerificationCode } from '../model/enrollment-contract'
import { fixture, defaultHandler, policy, attempt, accepted, admission, ack, email, password, attemptId, revision, userId } from './enrollment-consent/fixtures'
const live: ReturnType<typeof fixture>[] = []
const setup = (...args: Parameters<typeof fixture>) => { const item = fixture(...args); live.push(item); return item }
afterEach(() => { live.splice(0).forEach(item => item.dispose()); vi.restoreAllMocks(); vi.useRealTimers() })
const rejection = (reason: string, status: number, uncertain = false) => new TransportError('http', { status, uncertain,
  server: parseServerError({ code: 'TEST_ONLY', message: 'PRIVATE SERVER TEXT', details: { reason } }, 'auth', status) })
async function ready(item: ReturnType<typeof fixture>) { expect((await item.enrollment.loadPolicy()).kind).toBe('ready'); expect((await item.enrollment.beginAttempt()).kind).toBe('ready') }
it.each(['12345678', ' '.repeat(8), '가'.repeat(42) + 'aa', 'a'.repeat(128)])('preserves UTF8 boundary %j', value => expect(validEnrollmentPassword(value)).toBe(true))
it.each(['short', '가'.repeat(43), 'a'.repeat(129), '\ud80012345678'])('rejects malformed/oversized password %j', value => expect(validEnrollmentPassword(value)).toBe(false))
it.each(['12345', '1234567', '１２３４５６', ' 12345', 'abcdef'])('rejects invalid code %j', value => expect(validVerificationCode(value)).toBe(false))
it('never treats syntheticQualification or boolean acknowledgments as published consent', async () => {
  const f = setup()
  expect((await f.enrollment.signup(email, password, ack)).kind).toBe('policy-unavailable'); expect(f.requests).toHaveLength(0)
})
it('sends only bound attempt/revision/exact IDs/body UUID, neutral result and no identity/cache mutation', async () => {
  const f = setup(); await ready(f)
  for (const ids of [[], [...ack, ...ack], ['extra'], [...ack, 'extra']]) expect((await f.enrollment.signup(email, password, ids)).kind).toBe('acknowledgements-required')
  expect((await f.enrollment.signup(email, password, ack)).kind).toBe('created')
  const request = f.requests.at(-1)!
  expect(request.path).toBe('/api/auth/enrollment/signup'); expect(request.access).toBe('public')
  expect(request.body).toEqual({ json: { attemptId, policyRevision: revision, acknowledgementIds: ack, idempotencyKey: expect.any(String), email, password } })
  expect(request.idempotencyKey).toBeUndefined(); expect(request.refresh).toBeUndefined(); expect(f.session.read().userId).toBeNull()
  expect(f.client.getQueryCache().getAll()).toHaveLength(0); expect(f.client.getMutationCache().getAll()).toHaveLength(0)
  expect((await f.enrollment.signup(email, password, ack)).kind).toBe('idempotency-conflict')
})
it('reserves one command synchronously and rejects duplicate simultaneous dispatches', async () => {
  let release!: (value: { status: number; data: unknown }) => void
  const f = setup(request => request.path.endsWith('/signup') ? new Promise(resolve => { release = resolve }) : defaultHandler(request))
  await ready(f); const first = f.enrollment.signup(email, password, ack)
  expect(f.enrollment.pending).toBe(true); expect((await f.enrollment.signup(email, password, ack)).kind).toBe('busy')
  expect((await f.enrollment.resend(attemptId)).kind).toBe('busy'); expect(f.requests.filter(r => r.path.endsWith('/signup'))).toHaveLength(1)
  release({ status: 202, data: accepted() }); expect((await first).kind).toBe('created')
})
it('retains exact original key/payload for explicit unknown retry and never automatically replays', async () => {
  let calls = 0
  const f = setup(request => { if (request.path.endsWith('/signup') && ++calls === 1) throw new TransportError('network', { uncertain: true }); return defaultHandler(request) })
  await ready(f); expect((await f.enrollment.signup(email, password, ack)).kind).toBe('uncertain')
  expect(f.enrollment.read().unknown).toBe('signup'); expect(calls).toBe(1)
  expect((await f.enrollment.signup('other@example.invalid', 'changed password', ack)).kind).toBe('busy')
  expect((await f.enrollment.retryUnknown()).kind).toBe('created')
  const mutations = f.requests.filter(r => r.path.endsWith('/signup')); expect(mutations).toHaveLength(2)
  expect(mutations[1]!.body).toEqual(mutations[0]!.body); expect(f.enrollment.read().unknown).toBeNull()
})
it('does not extend an original unknown resend receipt using retry time', async () => {
  let tick = 1000, calls = 0
  vi.spyOn(performance, 'now').mockImplementation(() => tick)
  const receipt = accepted()
  const f = setup(request => { if (request.path.endsWith('/resend') && ++calls === 1) throw new TransportError('network', { uncertain: true }); return { status: 202, data: receipt } })
  expect((await f.enrollment.resend(attemptId)).kind).toBe('uncertain'); tick += 1000
  expect((await f.enrollment.retryUnknown()).kind).toBe('received')
  expect(f.enrollment.read().attemptDeadline).toBe(1000 + Date.parse(receipt.expiresAt) - Date.parse(receipt.serverTime))
})
it('bounds attempt/policy/countdown at request start, even if wall clock moves backwards', async () => {
  let tick = 1000
  vi.spyOn(performance, 'now').mockImplementation(() => tick)
  const f = setup(); await ready(f); await f.enrollment.signup(email, password, ack)
  const expiry = f.enrollment.read().attemptDeadline, count = f.requests.length
  expect((await f.enrollment.resend(attemptId)).kind).toBe('rate-limited')
  vi.spyOn(Date, 'now').mockReturnValue(0); tick = expiry + 1
  expect((await f.enrollment.verify(attemptId, '123456')).kind).toBe('attempt-expired'); expect(f.requests).toHaveLength(count)
})
it.each([['POLICY_UNAVAILABLE',503,'policy-unavailable'],['POLICY_STALE',409,'policy-stale'],['ATTEMPT_EXPIRED',410,'attempt-expired'],['ATTEMPT_INVALID',404,'attempt-invalid'],['ACKNOWLEDGEMENTS_REQUIRED',422,'acknowledgements-required'],['IDEMPOTENCY_CONFLICT',409,'idempotency-conflict'],['VERIFICATION_FAILED',422,'invalid-code'],['ACCOUNT_INACTIVE',403,'account-inactive']] as const)('maps authoritative %s and status, without raw server text', async (reason, status, kind) => {
  const f = setup(() => { throw rejection(reason, status) })
  expect(await f.enrollment.verify(attemptId, '123456')).toEqual({ kind }); expect(JSON.stringify(f.enrollment.read())).not.toContain('PRIVATE')
})
it('does not guess a duplicate account or verification completion from opaque statuses', async () => {
  const f = setup(() => { throw rejection('PRIVATE_UNKNOWN', 409) }); expect((await f.enrollment.verify(attemptId, '123456')).kind).toBe('unavailable')
})
it('honors bounded server retry metadata, no client invented duration or automatic request', async () => {
  let tick = 3000
  vi.spyOn(performance, 'now').mockImplementation(() => tick)
  const f = setup(() => { throw new TransportError('http', { status: 429, retryAfterMs: 2000 }) })
  expect((await f.enrollment.verify(attemptId, '123456')).kind).toBe('rate-limited'); expect(f.enrollment.read().retryDeadline).toBe(5000)
  tick = 4999; expect((await f.enrollment.verify(attemptId, '123456')).kind).toBe('rate-limited'); expect(f.requests).toHaveLength(1)
})
it('fences late completion after privacy/session/cancel and removes unknown command secrets', async () => {
  let release!: (value: {status:number;data:unknown}) => void
  const f = setup(request => request.path.endsWith('/signup') ? new Promise(resolve => { release = resolve }) : defaultHandler(request))
  await ready(f); const pending = f.enrollment.signup(email, password, ack)
  f.privacy.changePolicy(f.privacy.capture(), 'consent-revoked')
  expect(f.requests.at(-1)!.signal!.aborted).toBe(true); release({ status: 202, data: accepted() })
  expect((await pending).kind).toBe('cancelled'); expect(f.enrollment.read().accepted).toBeNull(); expect(f.enrollment.read().unknown).toBeNull()
  expect((await f.enrollment.retryUnknown()).kind).toBe('unavailable')
})
it('verifies only opaque attempt+code, and does not synthesize login', async () => {
  const f = setup(); expect((await f.enrollment.verify(attemptId, '123456')).kind).toBe('verified')
  expect(f.requests[0]!.body).toEqual({ json: { attemptId, code: '123456' } }); expect(f.session.read().accessToken).toBeNull()
})
it('never accepts a receipt bound to another attempt', async () => {
  const f = setup(() => ({ status: 200, data: { status: 'VERIFIED', attemptId: revision, serverTime: new Date().toISOString() } }))
  expect((await f.enrollment.verify(attemptId, '123456')).kind).toBe('uncertain')
})
it('uses existing canonical identity for protected consent and authoritative admission', async () => {
  const f = setup(); f.session.replace(0, { userId, eventId: null, accessToken: 'same-canonical-token' })
  await f.enrollment.loadPolicy(); expect((await f.enrollment.acceptPolicy(ack)).kind).toBe('admitted')
  expect((await f.enrollment.checkAdmission()).kind).toBe('admitted')
  expect(f.requests.slice(-2).every(request => request.access === 'protected')).toBe(true)
  expect(f.session.read().accessToken).toBe('same-canonical-token')
})
it.each(['CONSENT_REQUIRED','POLICY_UNAVAILABLE','ACCOUNT_INACTIVE'])('retains server admission denial %s', async decision => {
  const f = setup(() => ({ status: 200, data: admission(decision) })); f.session.replace(0, { userId, eventId: null, accessToken: 'canonical' })
  expect((await f.enrollment.checkAdmission()).kind).not.toBe('admitted'); expect(f.enrollment.read().admission?.decision).toBe(decision)
})
it('rejects wrong statuses, unknown fields, duplicate policy IDs and inconsistent admission', () => {
  const context = { status: 200, path: '/api/auth/enrollment/policy', scope: { generation: 0, userId: null, eventId: null } }
  expect(() => decodePolicy({ ...policy(), unapproved: true }, context)).toThrow()
  const p = policy(); expect(() => decodePolicy({ ...p, documents: [...p.documents,...p.documents] }, context)).toThrow()
  expect(() => decodePolicy(p, { ...context, status: 202 })).toThrow()
  expect(() => decodeAdmission({ ...admission(), acceptedRevision: null }, context)).toThrow()
})
it('rejects mismatched begin revision and delayed already-expired policy responses', async () => {
  const f = setup(request => request.path.endsWith('/attempts') ? { status: 201, data: { ...attempt(), policyRevision: attemptId } } : defaultHandler(request))
  await f.enrollment.loadPolicy(); expect((await f.enrollment.beginAttempt()).kind).toBe('uncertain')
})
it('does not create an attempt for malformed signup fields and does not retain commands when Auth is busy', async () => {
  const f = setup(); await f.enrollment.loadPolicy()
  expect((await f.enrollment.signup(email, 'short', ack)).kind).toBe('invalid-input'); expect(f.requests).toHaveLength(1)
  await f.enrollment.beginAttempt(); f.setCanStart(false)
  expect((await f.enrollment.signup(email, password, ack)).kind).toBe('busy')
  expect(f.enrollment.read().unknown).toBeNull(); f.setCanStart(true)
  expect((await f.enrollment.retryUnknown()).kind).toBe('unavailable')
})
it('does not extend bound attempt expiry when resend returns a later expiry', async () => {
  let later = false
  const f = setup(request => {
    if (request.path.endsWith('/resend') && later) { const response = accepted(); response.expiresAt = new Date(Date.parse(response.expiresAt) + 600_000).toISOString(); return { status: 202, data: response } }
    return defaultHandler(request)
  })
  let tick = 1000; vi.spyOn(performance, 'now').mockImplementation(() => tick)
  await ready(f); await f.enrollment.signup(email, password, ack)
  const bound = f.enrollment.read().attemptDeadline; tick += 2500; later = true
  expect((await f.enrollment.resend(attemptId)).kind).toBe('received'); expect(f.enrollment.read().attemptDeadline).toBeLessThanOrEqual(bound)
})
it('cancelling an unknown signup discards password and the local attempt without claiming server rollback', async () => {
  const f = setup(request => { if (request.path.endsWith('/signup')) throw new TransportError('network', { uncertain: true }); return defaultHandler(request) })
  await ready(f); await f.enrollment.signup(email, password, ack); f.enrollment.cancel()
  expect(f.enrollment.read().attempt).toBeNull(); expect(f.enrollment.read().result?.kind).toBe('cancelled')
  expect((await f.enrollment.retryUnknown()).kind).toBe('unavailable')
})
