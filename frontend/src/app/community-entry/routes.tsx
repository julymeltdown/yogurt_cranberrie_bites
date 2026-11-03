import { createContext, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from 'react'
import { Link, useParams } from 'react-router-dom'
import { PartyAuthoring } from '../../features/party-authoring'
import { PartyManagement, PartyRally } from '../../features/party-management'
import { GiveawayAuthoring } from '../../features/giveaway-authoring'
import { GiftPickup } from '../../features/gift-pickup'
import { canonicalId } from '../routes'
import { createCommunityEntry, unavailableCommunityEntry, type CommunityEntry } from './bridge'
import type { CommunityEntryAuth, CommunityEntryRelease } from './ports'

const Context = createContext<CommunityEntry | null>(null)
const noSubscribe = () => () => {}, empty = () => unavailableCommunityEntry
/** Mount inside the existing community owner, outside route panels. No new Auth/query/world owners. */
export function CommunityEntryProvider({ auth, release, children }: {
  readonly auth: CommunityEntryAuth | null; readonly release: CommunityEntryRelease | null; readonly children: ReactNode
}) {
  const [owner, setOwner] = useState<{ auth: typeof auth; release: typeof release; entry: CommunityEntry } | null>(null)
  useEffect(() => {
    const entry = createCommunityEntry({ auth, release }); setOwner({ auth, release, entry })
    return () => entry.dispose()
  }, [auth, release])
  const current = owner?.auth === auth && owner.release === release ? owner.entry : null
  return <Context value={current}>{children}</Context>
}
export function useCommunityEntry() {
  const entry = useContext(Context)
  const state = useSyncExternalStore(entry?.subscribe ?? noSubscribe, entry?.read ?? empty, empty)
  return { entry, state }
}
/** Constructors run in an effect. Never show the previous route while selection catches up. */
function useSelection<T>(key: string, select: (entry: CommunityEntry) => T | null, attempt = 0) {
  const { entry, state } = useCommunityEntry()
  const [selected, setSelected] = useState<{ entry: CommunityEntry; key: string; value: T | null } | null>(null)
  useEffect(() => {
    if (entry && state.phase === 'available') setSelected({ entry, key, value: select(entry) })
    else setSelected(null)
    // Selection is defined solely by route key and the current release; not render closure identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entry, key, state.phase, state.revision, attempt])
  return state.phase === 'available' && selected?.entry === entry && selected.key === key ? selected.value : null
}
function Unavailable() {
  return <section><h1>Community action unavailable</h1><p role="status">Current account, event and time information is required. If another pickup command is unfinished, return to its original panel to check its status.</p></section>
}
export function PartyAuthoringPanel({ editing = false }: { readonly editing?: boolean }) {
  const { partyId = '' } = useParams(), id = editing ? partyId : undefined
  const controller = useSelection(`party-author:${id ?? 'new'}`, e => !editing || canonicalId(partyId) ? e.partyAuthor(id) : null)
  return controller ? <PartyAuthoring controller={controller} editing={editing} /> : <Unavailable />
}
export function PartyManagementPanel({ rally = false }: { readonly rally?: boolean }) {
  const { partyId = '' } = useParams()
  const controller = useSelection(`party-manage:${partyId}`, e => e.partyManager(partyId))
  useEffect(() => { if (controller && rally) void controller.load() }, [controller, rally])
  if (!controller) return <Unavailable />
  return rally ? <PartyRally controller={controller} /> : <><PartyManagement controller={controller} />
    <details><summary>Edit party and meeting</summary><PartyAuthoringPanel editing /></details>
    <Link to={`/meet/parties/${partyId}/rally`}>Open rendezvous</Link></>
}
export function GiveawayAuthoringPanel({ editing = false }: { readonly editing?: boolean }) {
  const { giveawayId = '' } = useParams(), [reservationId, setReservationId] = useState<string | null>(null)
  const id = editing ? giveawayId : undefined
  const controller = useSelection(`giveaway-author:${id ?? 'new'}`, e => !editing || canonicalId(giveawayId) ? e.giveawayAuthor(id) : null)
  // Keep owner claim explicitly bound to both the giveaway and the selected authorized row.
  const [selection, setSelection] = useState<{ giveawayId: string; reservationId: string } | null>(null)
  useEffect(() => { setReservationId(null); setSelection(null) }, [giveawayId])
  if (!controller) return <Unavailable />
  return <><GiveawayAuthoring controller={controller} editing={editing} {...editing ? { onReservation: (id: string) => {
    setReservationId(id); setSelection({ giveawayId, reservationId: id })
  } } : {}} />{reservationId && selection?.giveawayId === giveawayId && selection.reservationId === reservationId
    && <GiftPickupTarget giveawayId={giveawayId} reservationId={reservationId} />}</>
}
function GiftPickupTarget({ giveawayId, reservationId }: { readonly giveawayId?: string; readonly reservationId?: string }) {
  const [attempt, setAttempt] = useState(0)
  const { entry, state } = useCommunityEntry()
  const controller = useSelection(`pickup:${giveawayId ?? ''}:${reservationId ?? ''}`, e => e.giftPickup(giveawayId, reservationId), attempt)
  const recovery = state.phase === 'available' ? entry?.pickupRecovery() ?? null : null
  return controller ? <GiftPickup controller={controller} /> : recovery
    ? <PickupRecovery controller={recovery} retry={() => setAttempt(v => v + 1)} /> : <Unavailable />
}
function PickupRecovery({ controller, retry }: { controller: NonNullable<ReturnType<CommunityEntry['pickupRecovery']>>; retry: () => void }) {
  const s = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <section className="gift-pickup"><h1>Finish the original pickup command</h1>
    <p>The requested pickup has not been opened. A command for {s.gift?.title ?? 'another pickup'} still needs recovery.</p>
    <p role="status">{s.message}</p><button disabled={!controller.canCheckStatus()} onClick={() => void controller.checkStatus()}>Check original command</button>
    <button onClick={retry}>Open requested pickup when resolved</button></section>
}
export function ReservationPanel() {
  const { giveawayId = '' } = useParams()
  return canonicalId(giveawayId) ? <GiftPickupTarget giveawayId={giveawayId} /> : <Unavailable />
}
export function PickupPanel() {
  const { reservationId = '' } = useParams()
  return canonicalId(reservationId) ? <GiftPickupTarget reservationId={reservationId} /> : <Unavailable />
}
/** Integrator may merge these elements into its existing panel map. This module registers no router. */
export const communityEntryPanels: Readonly<Record<string, ReactNode>> = {
  '/meet/parties/new': <PartyAuthoringPanel />,
  '/meet/parties/:partyId/manage': <PartyManagementPanel />,
  '/meet/parties/:partyId/rally': <PartyManagementPanel rally />,
  '/gifts/new': <GiveawayAuthoringPanel />,
  '/gifts/:giveawayId/manage': <GiveawayAuthoringPanel editing />,
  '/gifts/:giveawayId/reservation': <ReservationPanel />,
  '/gifts/pickups/:reservationId': <PickupPanel />,
}
