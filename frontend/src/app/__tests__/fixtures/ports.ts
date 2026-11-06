import { createMemorySession } from '../../../shared/api/http'
import { createSessionController, type SessionController } from '../../../features/auth/model/session'
import { autoAdmit, withAdmission } from '../../../features/auth/__tests__/fixtures/admission'
import { CAPABILITIES, projectCapabilities } from '../../capabilities'
import { policyFixtureRelease } from '../../pre-world-policy/__tests__/fixtures'
import { testProfile } from '../../community-adoption/__tests__/fixtures'
import openBootstrap from '../../../../../contracts/fixtures/core/bootstrap-open.json'
import type { ActiveScope, SessionReadiness, ShellBootstrap, ShellPorts } from '../../ports'

/** Synthetic identities exist ONLY in this explicitly imported test/dev fixture. */
export const ids = {
  user: '00000000-0000-4000-8000-000000000001', other: '00000000-0000-4000-8000-000000000002',
  event: '00000000-0000-4000-8000-000000000003', nextEvent: '00000000-0000-4000-8000-000000000004',
  gift: '00000000-0000-4000-8000-0000000000a5',
} as const
export const giftPath = `/gifts/${ids.gift}`
function documentFor(scope: ActiveScope) {
  const document = structuredClone(openBootstrap)
  document.event.event_id = scope.eventId
  document.participant.user_id = scope.userId
  document.participant.event_id = scope.eventId
  const freeze = (value: unknown): unknown => {
    if (value !== null && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value) }
    return value
  }
  return freeze(document) as Readonly<Record<string, unknown>>
}
export function bootstrapFor(scope: ActiveScope): ShellBootstrap {
  return Object.freeze({ contractVersion: '0.1.0', event: Object.freeze({ eventId: scope.eventId, title: 'Shell review · test event', phase: 'OPEN' as const }),
    participant: Object.freeze({ userId: scope.userId, eventId: scope.eventId }), worldAvailable: true,
    capabilities: Object.freeze(projectCapabilities(Object.fromEntries(CAPABILITIES.map(key => [key, { available: true, reason: null }])))),
    document: documentFor(scope) })
}
export const testPreWorldPolicy = { release: () => policyFixtureRelease,
  async read() { const time = Date.now(); return { kind: 'normal' as const, eventRevision: 1, policyRevision: 1,
    minimumClientVersion: '0.1.0', serverTimeMs: time, validUntilMs: time + 25_000 } } }
export function createFixturePorts(initial: SessionReadiness = 'ready') {
  const session = createMemorySession()
  session.replace(0, { userId: ids.user, eventId: ids.event, accessToken: 'fixture-only-memory-token' })
  const controller = createSessionController({ eventId: ids.event, fetch: withAdmission(async () => new Response(null, { status: 503 })),
    cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
  controller.session.replace(0, { userId: ids.user, eventId: ids.event, accessToken: 'fixture-only-memory-token' })
  // Shell fixtures install an admitted synthetic identity synchronously; feature requests still
  // traverse canonical Auth and its server admission read before their first protected call.
  let source = controller.read(), admitted = controller.session.read().userId ? Object.freeze({ ...source, admission: 'ADMITTED' as const }) : source
  const auth: SessionController = Object.freeze({ ...controller, read: () => {
    const state = controller.read()
    if (state !== source) { source = state; admitted = controller.session.read().userId ? Object.freeze({ ...state, admission: 'ADMITTED' as const }) : state }
    return admitted
  } })
  autoAdmit(auth)
  let readiness: SessionReadiness = initial
  const listeners = new Set<() => void>()
  const stats = { games: 0, connections: 0, peakGames: 0, peakConnections: 0, mounts: 0, disposals: 0,
    inputBlocked: true, visible: true, scopes: [] as string[], signals: [] as AbortSignal[] }
  const ports: ShellPorts = {
    auth, session: auth.session, dispose: () => auth.dispose(), communityClock: testProfile,
    preWorldPolicy: testPreWorldPolicy,
    readiness: { read: () => readiness, subscribe: listener => { listeners.add(listener); return () => { listeners.delete(listener) } } },
    bootstrap: { load: async scope => bootstrapFor(scope) },
    admin: { authorize: async () => false },
    world: { mount: (host, context) => {
      stats.mounts++; stats.games++; stats.connections++
      stats.peakGames = Math.max(stats.peakGames, stats.games)
      stats.peakConnections = Math.max(stats.peakConnections, stats.connections)
      stats.scopes.push(`${context.scope.userId}/${context.scope.eventId}`)
      stats.signals.push(context.signal)
      const placeholder = document.createElement('p')
      placeholder.textContent = 'Instrumented test host · no live world'
      placeholder.className = 'fixture-host-label'
      host.append(placeholder)
      let disposed = false
      return {
        dispose() { if (disposed) return; disposed = true; stats.disposals++; stats.games--; stats.connections--; placeholder.remove() },
        setInputBlocked(value) { stats.inputBlocked = value }, setVisible(value) { stats.visible = value },
      }
    } },
  }
  return { ports, stats, setReadiness(value: SessionReadiness) { readiness = value; listeners.forEach(listener => listener()) },
    replace(userId: string = ids.other, eventId: string = ids.event) {
      auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'fixture-only-memory-token' })
    } }
}
