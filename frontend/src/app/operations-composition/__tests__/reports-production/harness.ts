import { createSessionController } from '../../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { createAuthorityTimeline } from '../../../community-authority/timeline'
import { createDefaultAdmin } from '../../owner'
import { createReportsServer, wire } from './server'
export const owners: { dispose(): void }[] = []
export function setup(role: 'MODERATOR' | 'OPERATOR' | null = 'MODERATOR') {
  const server = createReportsServer(); server.state.role = role
  const auth = createSessionController({ eventId: wire.event, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 1000,
    fetch: input => server.handle(input as Request) })
  auth.session.replace(0, { userId: wire.actor, eventId: wire.event, accessToken: wire.auth.accessToken })
  let elapsed = 0, interrupt = () => {}
  const admin = createDefaultAdmin(auth, { timeline: signal => createAuthorityTimeline({ observe: () => ({ nowMs: elapsed, origin: 100 }), maxObservationGapMs: 120_000,
    onInterruption(invalidate) { interrupt = invalidate; signal.addEventListener('abort', invalidate, { once: true }); return () => signal.removeEventListener('abort', invalidate) } }) })
  const scope = { generation: auth.session.read().generation, userId: wire.actor, eventId: wire.event }
  const entry = admin.open(scope)!
  const h = { auth, admin, server, scope, entry, advance(n: number) { elapsed = n }, interrupt: () => interrupt(),
    dispose() { admin.dispose(); auth.dispose() } }
  owners.push(h); return h
}
export type Harness = ReturnType<typeof setup>
export async function review(h: Harness) {
  await h.entry.start()
  const child = h.entry.read().reports!, controller = child.borrow()!
  await controller.load(); await controller.select(h.server.reports.report.report_id)
  return controller
}
