import { createContext, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { usePorts } from '../access'
import { WorldPanels } from '../world-panels'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { WorldAccessBinding } from '../ports'
import { WorldRooms } from '../../features/world-rooms'
import { WorldNearby } from '../../features/world-nearby'
import { createWorldAccessBridge, type WorldAccessBridge } from './bridge'

const Context = createContext<WorldAccessBridge | null>(null)
const empty = () => null, noop = () => () => {}
/** Keep this provider's tree shape stable while WorldHost publishes/revokes its actual mount lease. */
export default function ProductionWorldFeatures({ children, privacyScope = null }: { children: ReactNode; privacyScope?: PrivacyScope | null }) {
  const ports = usePorts(), source = ports.worldAccess
  const binding = useSyncExternalStore(source?.subscribe ?? noop, source?.read ?? empty, empty)
  const [owner, setOwner] = useState<{ auth: typeof ports.auth; binding: WorldAccessBinding; bridge: WorldAccessBridge } | null>(null)
  useEffect(() => {
    if (!ports.auth || !binding) return
    let bridge: WorldAccessBridge
    try { bridge = createWorldAccessBridge({ auth: ports.auth, replication: binding.replication, release: binding.release }) } catch { return }
    setOwner({ auth: ports.auth, binding, bridge })
    return () => bridge.dispose()
  }, [ports.auth, binding])
  const current = owner?.auth === ports.auth && owner?.binding === binding && binding && !binding.release.signal.aborted && ports.auth?.privacy.isCurrent(binding.release.privacyScope) ? owner.bridge : null
  return <Context value={current}><WorldPanels source={binding?.presentation ?? null} scope={privacyScope}>{children}</WorldPanels></Context>
}
export function ProductionRoomsPanel() {
  const bridge = useContext(Context)
  return bridge ? <WorldRooms controller={bridge.controller} /> : <p role="status">Room access is unavailable until your current world access is verified.</p>
}
export function ProductionNearbyPanel() {
  const bridge = useContext(Context)
  return bridge ? <WorldNearby controller={bridge.controller} /> : <p role="status">Nearby people are unavailable until your current world access is verified.</p>
}
