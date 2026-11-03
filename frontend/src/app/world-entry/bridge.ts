import { createWorldAccessController, type WorldAccessPort, type WorldAccessRelease, type ReplicationReader } from '../../features/world-rooms'
import type { ReplicationView } from '../../world/network/replication'
import { assertEntry, captureEntry, sameScope, validateEntry, type EntryAuth, type WorldEntryRequest } from './authority'

/** One bootstrap lease. Reauthorization constructs a new bridge; an old one is never restamped. */
export function createWorldAccessBridge({ auth, replication, release: input }: {
  auth: EntryAuth; replication: ReplicationReader; release: WorldEntryRequest
}) {
  const captured = captureEntry(input), lifetime = new AbortController()
  const combined = AbortSignal.any([captured.signal, lifetime.signal])
  const listeners = new Set<() => void>(), observers = new Set<() => void>()
  let authorized = false, withdrawn = false, disposed = false
  const valid = () => {
    if (withdrawn || disposed || combined.aborted || !authorized) return false
    try { assertEntry(auth, captured); return true } catch { return false }
  }
  const notify = (set: Set<() => void>) => { for (const listener of Array.from(set)) { try { listener() } catch { /* Independent private observers. */ } } }
  const reader: ReplicationReader = {
    read(): ReplicationView | null {
      if (!valid()) return null
      let value: ReplicationView | null
      try { value = replication.read() } catch { return null }
      const b = value?.baseline
      return valid() && value && sameScope(value.scope, captured.scope) && (!b || b.eventId === captured.scope.eventId && b.selfUserId === captured.scope.userId) ? value : null
    },
    subscribe(listener) { if (withdrawn || disposed) return () => {}; observers.add(listener); return () => { observers.delete(listener) } },
    async switchRoom(roomId) {
      if (!valid() || !reader.read() || !valid()) return false
      const sent = await replication.switchRoom(roomId)
      return valid() && !!reader.read() && sent
    },
  }
  const release: WorldAccessRelease = Object.freeze({ scope: captured.privacyScope, capabilities: captured.bootstrap.capabilities, replication: reader })
  const world: WorldAccessPort = {
    read: () => valid() ? release : null,
    subscribe(listener) { if (withdrawn || disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
  }
  const controller = createWorldAccessController({ auth, world })
  let stopReplication = () => {}, stopPrivacy = () => {}
  function withdraw() {
    if (withdrawn) return
    withdrawn = true; authorized = false; lifetime.abort()
    for (const stop of [stopReplication, stopPrivacy]) { try { stop() } catch { /* Cleanup failure must not retain private presentation. */ } }
    captured.signal.removeEventListener('abort', withdraw)
    notify(listeners); notify(observers); observers.clear(); listeners.clear()
  }
  captured.signal.addEventListener('abort', withdraw, { once: true })
  stopPrivacy = auth.privacy.registerCleanup(withdraw)
  try { stopReplication = replication.subscribe(() => { if (valid()) notify(observers) }) }
  catch (error) { withdraw(); controller.dispose(); throw error }
  if (captured.signal.aborted) withdraw()
  const ready = auth.privacy.run(captured.privacyScope, async () => {
    await validateEntry(auth, captured)
    if (withdrawn || combined.aborted) return false
    authorized = true; notify(listeners)
    return valid()
  }, combined).catch(() => { withdraw(); return false })
  return { world, controller, ready, withdraw,
    dispose() { if (disposed) return; withdraw(); controller.dispose(); disposed = true },
  }
}
export type WorldAccessBridge = ReturnType<typeof createWorldAccessBridge>
