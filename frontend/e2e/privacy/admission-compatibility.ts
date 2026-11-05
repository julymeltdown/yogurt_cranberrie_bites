import approved from '../../../contracts/fixtures/domain/approved-social-request.json'
import { createSocialSharingController } from '../../src/features/social-sharing/model/controller'
import { harness, beaconId } from '../../src/features/meet-beacon/__tests__/fixtures/harness'
import { createFixtureSessionController } from './auth-fixture'
import { assertAbsent, check, markers } from './oracle'

/** Only the changed admission/fixture seam. Does not replay the seven-role matrix. */
export async function runAdmissionCompatibility() {
  const calls: Request[] = []
  let denied = false
  const auth = createFixtureSessionController({ eventId: approved.event_id, admissionDecision: () => denied ? 'CONSENT_REQUIRED' : 'ADMITTED', fetch: async input => {
    const request = input as Request
    calls.push(request.clone())
    return Response.json(approved, { headers: { 'Cache-Control': 'no-store' } })
  } })
  auth.session.replace(0, { userId: approved.requester_id, eventId: approved.event_id, accessToken: 'qa04-admission-fixture-only' })
  const controller = createSocialSharingController({ auth, sensitiveRead: auth.requestSensitive,
    access: { read: () => ({ available: true, readOnly: false }), subscribe: () => () => undefined }, now: () => Date.parse(approved.created_at) })
  controller.start()
  try {
    check(auth.read().admission === 'unchecked', 'synthetic identity alone does not imply admission')
    await controller.loadRequest(approved.request_id)
    check(auth.read().admission === 'ADMITTED' && controller.read().detail.phase === 'ready', 'actual Auth admits owner receipt before protected social read')
    check(calls.length === 1 && calls[0]!.method === 'GET' && new URL(calls[0]!.url).pathname.endsWith(approved.request_id), 'only the explicit feature read reaches the feature transport')
    const identity = auth.session.read(), scope = auth.privacy.capture()
    auth.queryClient.setQueryData(['qa04-admission-private-taint'], { handle: markers[0] })
    denied = true
    check(!await auth.checkAdmission(), 'actual Auth revalidation consumes the owner consent-required receipt')
    await controller.loadRequest(approved.request_id)
    check(auth.read().admission === 'CONSENT_REQUIRED' && auth.session.read() === identity, 'actual consent withdrawal preserves canonical identity')
    check(!auth.privacy.isCurrent(scope), 'actual Auth withdrawal retires previous private lifetime')
    assertAbsent(controller.read())
    assertAbsent(auth.queryClient.getQueryCache().getAll().map(query => query.state))
    check(calls.length === 1 && calls.every(request => request.method === 'GET'), 'withdrawn admission prevents feature dispatch, enrollment, refresh and mutation replay')
  } finally { controller.dispose(); auth.dispose() }

  const fixture = await harness()
  try {
    check(fixture.auth.read().admission === 'ADMITTED' && fixture.placesController.read().list.phase === 'ready', 'retained beacon factory finishes admission before FE19 initialization')
    check(!fixture.requests.some(request => new URL(request.url).pathname === '/api/auth/enrollment/admission'), 'admission setup preserves feature request accounting')
    await fixture.controller.select(beaconId)
    check(fixture.controller.read().phase === 'current', 'beacon positive fixture remains usable')
    fixture.setMode('denied')
    await fixture.controller.refresh()
    check(fixture.controller.read().beacon === null && fixture.controller.read().phase === 'unavailable', 'existing negative mode still denies beacon guidance')
    check(fixture.requests.every(request => request.method === 'GET'), 'fixture compatibility performs no writes')
  } finally { fixture.dispose() }
  return { task: 'QA04', result: 'MOCK_READY', qualification: 'SYNTHETIC_ADMISSION_COMPATIBILITY_ONLY',
    checks: ['actual Auth admission before social protected read', 'actual consent withdrawal clears private state without changing identity', 'retained beacon admission/FE19 initialization', 'retained beacon denial mode and request accounting'],
    originalTask: 'IN_PROGRESS', realService: 'NOT_RUN', originalMatrixReplayed: false }
}
