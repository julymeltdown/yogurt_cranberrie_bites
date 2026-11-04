import { createSessionController } from '../../src/features/auth/model/session'
import { createModeration, type ModeratorLease } from '../../src/features/admin/reports/model/controller'
import { check, assertAbsent, deferred, markers, roles, until, type Cell, type Role } from './oracle'

const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const eventId = id(1), epoch = Date.parse('2026-11-07T09:00:00Z')

/** Synthetic lease inputs exercise the existing frontend authority interface, not real role policy. */
export async function evidenceCells(role: Role): Promise<Cell[]> {
  const userId = id(500 + roles.indexOf(role)), calls: string[] = [], listeners = new Set<() => void>()
  let report = { report_id: id(803), event_id: eventId, target_type: 'USER', target_id: id(2), state: 'OPEN', revision: 1, created_at: '2026-11-07T08:00:00Z' }
  const privateDetail = () => ({ report, reporter_id: id(899), reason: 'PRIVACY', text: markers[2], evidence_media_ids: [id(804)] })
  const auth = createSessionController({ eventId, fetch: async input => {
    const request = input as Request
    calls.push(request.method)
    // A denied client must not dispatch: a synthetic backend does not grant by role name.
    check(role === 'admin', 'non-moderator evidence transport must remain unused')
    if (request.method === 'POST') {
      check(!!request.headers.get('Idempotency-Key'), 'moderation command uses idempotency')
      const body = await request.json() as { expected_revision: number; decision: string }
      check(body.expected_revision === report.revision && body.decision === 'RESOLVED', 'bound moderation revision')
      report = { ...report, revision: 2, state: 'RESOLVED' }
      return Response.json(report, { headers: { 'Cache-Control': 'no-store' } })
    }
    return Response.json(new URL(request.url).pathname.endsWith(report.report_id) ? privateDetail() : { items: [report], next_cursor: null }, { headers: { 'Cache-Control': 'no-store' } })
  } })
  auth.session.replace(0, { userId, eventId, accessToken: 'qa04-evidence-fixture-token' })
  let grant: ModeratorLease | null = role === 'admin' ? { scope: auth.privacy.capture(), role: 'MODERATOR', decisionId: 'qa04-synthetic-moderator-lease', expiresAt: epoch + 60000 } : null
  let delayed = false, opened = 0
  const late = deferred<{ mediaId: string; blob: Blob }>()
  const blob = new Blob([markers[2]], { type: 'image/png' }) // Typed test Blob only; no rendered/decoded-image claim.
  const controller = createModeration({ auth, lane: 'reports', now: () => epoch,
    access: { read: () => grant, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } },
    evidence: { async open(input) {
      opened++
      check(input.scope.userId === userId && input.reportId === report.report_id && input.mediaId === id(804), 'evidence request bound to lease/report/media')
      return delayed ? late.promise : { mediaId: id(804), blob }
    } },
  })
  try {
    await controller.load()
    if (role !== 'admin') {
      await controller.select(report.report_id); await controller.evidence(id(804))
      controller.draft('RESOLVED', markers[2]); controller.review(); await controller.submit()
      check(calls.length === 0 && opened === 0 && !controller.read().access, 'no evidence read/mutation without moderator lease')
      assertAbsent(controller.read())
    } else {
      check(controller.read().rows.length === 1, 'moderator list metadata available')
      assertAbsent(controller.read())
      await controller.select(report.report_id)
      check(controller.read().detail?.text === markers[2], 'authorized evidence detail retained')
      check(!JSON.stringify(controller.read()).includes(id(899)), 'reporter identity excluded from retained feature state')
      await controller.evidence(id(804))
      const url = controller.read().evidenceUrl
      check(typeof url === 'string' && url.startsWith('blob:'), 'private evidence uses native object URL')
      check(await (await fetch(url)).text() === markers[2], 'authorized synthetic Blob readable')
      // Actual feature cleanup must revoke the URL, not only hide a link in the UI.
      auth.privacy.changePolicy(auth.privacy.capture(), 'suspended')
      check(controller.read().evidenceUrl === null && controller.read().detail === null, 'privacy loss scrubs evidence feature state')
      check(await fetch(url).then(() => false, () => true), 'native evidence object URL revoked')
      assertAbsent(controller.read())
      auth.privacy.changePolicy(auth.privacy.capture(), 'revalidated')
      grant = { scope: auth.privacy.capture(), role: 'MODERATOR', decisionId: 'qa04-synthetic-revalidated-lease', expiresAt: epoch + 60000 }
      listeners.forEach(listener => listener())
      await controller.load(); await controller.select(report.report_id)
      delayed = true
      const pending = controller.evidence(id(804))
      await until(() => opened === 2)
      grant = null; listeners.forEach(listener => listener())
      late.resolve({ mediaId: id(804), blob }); await pending
      check(controller.read().evidenceUrl === null && controller.read().detail === null, 'late evidence cannot restore revoked lease')
      assertAbsent(controller.read())
      delayed = false
      grant = { scope: auth.privacy.capture(), role: 'MODERATOR', decisionId: 'qa04-synthetic-command-lease', expiresAt: epoch + 60000 }
      listeners.forEach(listener => listener())
      await controller.load(); await controller.select(report.report_id)
      controller.draft('RESOLVED', 'Synthetic privacy review'); controller.review()
      check(controller.read().confirm, 'moderation explicitly confirmed')
      await controller.submit()
      check(controller.read().phase === 'applied' && controller.read().selected?.revision === 2, 'moderation mutation bound to new state')
      assertAbsent(controller.read())
    }
    return (['list', 'detail', 'mutation', 'cache'] as const).map(operation => ({ role, operation, status: 'PASS', provenance: 'SYNTHETIC_FIXTURE',
      evidence: role === 'admin' ? 'Actual moderation controller; synthetic moderator lease; private report detail, native Blob URL revocation, delayed evidence fence and revision-bound mutation. Not real role policy, image decode, mounted UI or WS.' : 'Actual moderation controller with absent synthetic moderator lease; no private list/detail/evidence/mutation dispatch or retained data. Not server-role authorization.' }))
  } finally { controller.dispose(); auth.dispose() }
}
