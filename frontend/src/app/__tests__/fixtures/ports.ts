import { createMemorySession } from '../../../shared/api/http'
import type { ActiveScope, SessionReadiness, ShellBootstrap, ShellPorts } from '../../ports'

/** Synthetic identities exist ONLY in this explicitly imported test/dev fixture. */
export const ids = {
  user: '00000000-0000-4000-8000-000000000001', other: '00000000-0000-4000-8000-000000000002',
  event: '00000000-0000-4000-8000-000000000003', nextEvent: '00000000-0000-4000-8000-000000000004',
  gift: '00000000-0000-4000-8000-0000000000a5',
} as const
export const giftPath = `/gifts/${ids.gift}`
export function bootstrapFor(scope: ActiveScope): ShellBootstrap {
  return { contractVersion: '0.1.0', event: { eventId: scope.eventId, title: 'Shell review · test event', phase: 'OPEN' },
    participant: { userId: scope.userId, eventId: scope.eventId }, worldAvailable: true }
}
export function createFixturePorts(initial: SessionReadiness = 'ready') {
  const session = createMemorySession()
  session.replace(0, { userId: ids.user, eventId: ids.event, accessToken: 'fixture-only-memory-token' })
  let readiness: SessionReadiness = initial
  const listeners = new Set<() => void>()
  const stats = { games: 0, connections: 0, peakGames: 0, peakConnections: 0, mounts: 0, disposals: 0,
    inputBlocked: true, visible: true, scopes: [] as string[], signals: [] as AbortSignal[] }
  const ports: ShellPorts = {
    session,
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
      session.replace(session.read().generation, { userId, eventId, accessToken: 'fixture-only-memory-token' })
    } }
}
