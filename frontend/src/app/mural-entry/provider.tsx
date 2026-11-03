import { createContext, useContext, useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { MuralSyncPanel } from '../../features/mural/sync'
import { ConcertWindow } from '../../shared/ui/concert-window'
import type { PrivacyScope } from '../../shared/query/privacy'
import { createMuralEntry, unavailableMuralEntry, type MuralEntry } from './owner'
import type { MuralEntryAuth, MuralEntryRelease } from './ports'

const Context = createContext<MuralEntry | null>(null), noSubscribe = () => () => {}, empty = () => unavailableMuralEntry
/** Mount above the panel outlet. Only identity/privacy changes replace the owner; release changes rebind it. */
export function MuralEntryProvider({ auth, scope, release, children }: {
  readonly auth: MuralEntryAuth | null; readonly scope: PrivacyScope | null
  readonly release: MuralEntryRelease | null; readonly children: ReactNode
}) {
  const [owner, setOwner] = useState<MuralEntry | null>(null)
  const userId = scope?.userId, eventId = scope?.eventId, generation = scope?.generation, policyGeneration = scope?.policyGeneration, gate = scope?.gate
  useLayoutEffect(() => {
    if (!auth || !scope || !auth.privacy.isCurrent(scope)) { setOwner(null); return }
    const candidate = createMuralEntry({ auth, scope }); setOwner(candidate)
    return candidate.dispose
    // Semantic scope fields prevent a newly allocated equivalent scope from resetting a command.
  }, [auth, userId, eventId, generation, policyGeneration, gate])
  const current = owner && !owner.isDisposed() && owner.auth === auth && scope && auth?.privacy.isCurrent(owner.scope)
    && owner.scope.userId === userId && owner.scope.eventId === eventId && owner.scope.generation === generation
    && owner.scope.policyGeneration === policyGeneration && owner.scope.gate === gate ? owner : null
  useLayoutEffect(() => { current?.bind(release) }, [current, release])
  return <Context value={current}>{children}</Context>
}
export function useMuralEntry() {
  const owner = useContext(Context)
  return useSyncExternalStore(owner?.subscribe ?? noSubscribe, owner?.read ?? empty, empty)
}
export function MuralWallPanel() {
  const owner = useContext(Context), { sync, generation } = useMuralEntry(), host = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => owner?.registerWithdrawal(() => {
    const element = host.current
    if (!element) return
    element.hidden = true
    for (const canvas of element.querySelectorAll('canvas')) canvas.width = canvas.height = 0
  }), [owner])
  return <><div key={generation} ref={host} hidden={!sync}>{sync && <MuralSyncPanel sync={sync} />}</div>{!sync && <ConcertWindow title="GRAFFITI WALL" headingLevel={1} icon="wall">
    <p role="status">Wall unavailable. Reconnect to a qualified world scene before opening the current wall.</p>
  </ConcertWindow>}</>
}
