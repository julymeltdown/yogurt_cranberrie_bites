// TEST ONLY: actual DefaultAdmin owner/panel, canonical Auth and controlled HTTP/time.
// Does not initialize the older leaf fixture's independent OperationsEntry/controller.
import { createAuthorityTimeline } from '../../../community-authority/timeline'
import { createFixturePorts } from '../../../__tests__/fixtures/ports'
import { fixture, actor, eventId, time, uid } from '../../../../features/admin/service-policy/__tests__/fixture'
import { createDefaultAdmin } from '../../owner'
import wire from '../wire.json'

export function compositionFixture() {
  const policy = fixture(), { auth } = policy
  let elapsed = 0, settingsStatus = 200, role: 'OPERATOR' | 'MODERATOR' | null = 'OPERATOR', decision = 200
  let interruption = () => {}
  let intercept: ((request: Request) => Response | Promise<Response> | null) | null = null
  policy.override(request => {
    const custom = intercept?.(request); if (custom) return custom
    const path = new URL(request.url).pathname
    if (path.endsWith('/operations-access')) return Response.json({ server_time: time(elapsed), access: role ? {
      actor_id: actor, event_id: eventId, role, role_revision: 7, decision_id: uid(decision++),
      checked_at: time(elapsed), expires_at: time(elapsed + 60_000),
    } : null })
    if (path.endsWith('/admin/settings')) {
      const revision = policy.decision().event_revision
      return settingsStatus === 200 ? Response.json({ ...wire.settings, revision,
        event: { ...wire.settings.event, event_id: eventId, revision } }) : policy.error(settingsStatus, 'DEPENDENCY_UNAVAILABLE')
    }
    if (path.endsWith('/admin/overview')) return Response.json({ event_id: eventId,
      revision: policy.decision().event_revision, health: 'AVAILABLE', open_reports: 0, pending_jobs: 0 })
    if (['/admin/notices', '/admin/places', '/admin/audit', '/admin/reports'].some(suffix => path.endsWith(suffix))) {
      return Response.json({ items: [], next_cursor: null })
    }
    return null
  })
  const admin = createDefaultAdmin(auth, { timeline: signal => createAuthorityTimeline({
    observe: () => ({ nowMs: elapsed, origin: 1 }), maxObservationGapMs: 120_000,
    onInterruption(notify) { interruption = notify; signal.addEventListener('abort', notify, { once: true }); return () => signal.removeEventListener('abort', notify) },
  }) })
  const scope = () => ({ generation: auth.session.read().generation, userId: actor, eventId })
  const entry = () => admin.open(scope())
  const ports = { ...createFixturePorts().ports, session: auth.session, admin }
  return { auth, admin, policy, ports, scope, entry,
    async initialize() { policy.installUnitIdentity(); await auth.checkAdmission(); await entry()?.start() },
    role(value: typeof role) { role = value }, settingsStatus(value: number) { settingsStatus = value },
    intercept(value: typeof intercept) { intercept = value },
    advance(ms: number) { elapsed += ms; policy.advance(ms) }, interrupt() { interruption() },
    state() { return entry()?.read() ?? null },
    dispose() { admin.dispose(); policy.dispose() },
  }
}
export type CompositionFixture = ReturnType<typeof compositionFixture>
