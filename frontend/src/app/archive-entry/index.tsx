import { useCallback, useEffect, useState, useSyncExternalStore } from 'react'
import { ArchivePage, createArchiveContext, createArchiveController, createPublicArchiveReader, type ArchiveController } from '../../features/archive'
import type { SessionPort } from '../../shared/api/http'
import { canonicalId } from '../routes'
import { usePorts } from '../access'

type Observation = Pick<SessionPort, 'read'> | undefined
interface OwnedArchive {
  readonly eventId: string | null
  readonly session: Observation
  readonly generation: number
  readonly controller: ArchiveController
}

/** Observe only canonical lifetime changes; public archive requests never borrow credentials. */
function useGeneration(session: Observation) {
  const subscribe = useCallback((listener: () => void) => {
    if (!session) return () => undefined
    let signal = session.read().signal
    const changed = () => {
      signal.removeEventListener('abort', changed)
      signal = session.read().signal
      signal.addEventListener('abort', changed, { once: true })
      listener()
    }
    signal.addEventListener('abort', changed, { once: true })
    return () => signal.removeEventListener('abort', changed)
  }, [session])
  const snapshot = useCallback(() => session?.read().generation ?? 0, [session])
  return useSyncExternalStore(subscribe, snapshot, snapshot)
}

/** Public route lifetime only: no private reader, second identity, world or media fallback. */
export function ArchiveRoutePanel() {
  const ports = usePorts()
  const eventId = typeof ports.eventId === 'string' && canonicalId(ports.eventId) ? ports.eventId : null
  const session = ports.session
  const generation = useGeneration(session)
  const [owned, setOwned] = useState<OwnedArchive | null>(null)

  useEffect(() => {
    if ((session?.read().generation ?? 0) !== generation) return
    // Allocate subscriptions after commit; StrictMode replay receives a fresh owner.
    const context = createArchiveContext(eventId, session)
    const controller = createArchiveController({ context, reader: createPublicArchiveReader() })
    setOwned({ eventId, session, generation, controller })
    return () => { controller.dispose(); context.dispose() }
  }, [eventId, session, generation])

  // Never render a previous event/account while passive owner cleanup is pending.
  if (!owned || owned.eventId !== eventId || owned.session !== session || owned.generation !== generation) {
    return <main className="event-archive"><p role="status">Opening archive…</p></main>
  }
  return <ArchivePage controller={owned.controller} />
}
