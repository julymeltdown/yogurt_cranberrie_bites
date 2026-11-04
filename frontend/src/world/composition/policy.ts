import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope, PolicyChange } from '../../shared/query/privacy'
import type { WorldTransport } from '../network/WorldTransport'

/** Metadata from the already authenticated World stream; never a second connection. */
export interface WorldSocialPolicy {
  readonly scope: PrivacyScope
  readonly eventId: string
  readonly streamId: string
  readonly roomEpoch: string
  readonly policyRevision: number
}
export interface WorldSocialPolicyPort {
  read(): WorldSocialPolicy | null
  subscribe(listener: () => void): () => void
}
export function attachWorldSocialPolicy({ auth, scope, transport, signal }: {
  auth: Pick<SessionController, 'session' | 'privacy'>
  scope: PrivacyScope
  transport: Pick<WorldTransport, 'subscribe'>
  signal: AbortSignal
}) {
  const captured = Object.freeze({ ...scope }), listeners = new Set<() => void>()
  let closed = false, stream: { id: string; epoch: string } | null = null
  let value: WorldSocialPolicy | null = null, revision = 0
  let stopTransport = () => {}, stopPrivacy = () => {}
  const current = () => !closed && !signal.aborted && auth.session.isCurrent(captured) && auth.privacy.isCurrent(captured)
  const notify = () => { for (const fn of Array.from(listeners)) { try { fn() } catch { /* Independent private consumers. */ } } }
  const withdraw = () => { if (value) { value = null; notify() } }
  function dispose() {
    if (closed) return
    closed = true; stream = null; value = null
    signal.removeEventListener('abort', dispose)
    try { stopTransport() } catch { /* Finish independent withdrawal. */ }
    try { stopPrivacy() } catch { /* Finish independent withdrawal. */ }
    notify(); listeners.clear()
  }
  const port: WorldSocialPolicyPort = {
    read: () => current() ? value : null,
    subscribe(fn) { if (!current()) return () => {}; listeners.add(fn); return () => { listeners.delete(fn) } },
  }
  if (!current()) { dispose(); return { ...port, dispose } }
  try {
    stopPrivacy = auth.privacy.registerCleanup(dispose)
    signal.addEventListener('abort', dispose, { once: true })
    stopTransport = transport.subscribe(event => {
      if (!current()) { dispose(); return }
      if (event.type !== 'frame') {
        if (event.type !== 'state' || event.state !== 'live') { stream = null; withdraw() }
        return
      }
      const frame = event.frame
      if (frame.type === 'snapshot') {
        withdraw()
        stream = frame.eventId === captured.eventId && frame.selfUserId === captured.userId
          ? { id: frame.streamId, epoch: frame.roomEpoch } : null
        return
      }
      if (frame.type === 'revoke' || frame.type === 'resyncRequired') { stream = null; withdraw(); return }
      if (frame.type !== 'policy.baseline' && frame.type !== 'policy.invalidate') return
      if (!stream || frame.streamId !== stream.id || frame.eventId !== captured.eventId
        || frame.scope !== 'SOCIAL' || !Number.isSafeInteger(frame.policyRevision) || frame.policyRevision < 1
        || frame.policyRevision < revision) { withdraw(); return }
      if (frame.type === 'policy.invalidate') {
        revision = frame.policyRevision
        withdraw()
        const reason: PolicyChange = frame.reason === 'BLOCK_CHANGED' ? 'block'
          : frame.reason === 'EXPIRED' ? 'consent-expired'
            : frame.reason === 'CONSENT_CHANGED' ? 'consent-revoked' : 'handle-replaced'
        if (current()) auth.privacy.changePolicy(captured, reason)
        return
      }
      if (value?.policyRevision === frame.policyRevision) return
      revision = frame.policyRevision
      value = Object.freeze({ scope: captured, eventId: frame.eventId, streamId: stream.id,
        roomEpoch: stream.epoch, policyRevision: revision })
      notify()
    })
    if (!current()) dispose()
  } catch { dispose() }
  return { ...port, dispose }
}
