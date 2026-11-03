import { validCapabilities, type ShellCapabilities } from '../../../app/capabilities'
import type { SessionController } from '../../auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { TransportError } from '../../../shared/api/errors'
import type { ReplicationView } from '../../../world/network/replication'
import type { WorldRooms } from '../../../world/network/WorldTransport'
import { immutable } from '../../../world/entities/replicated'
import { projectNearby, type NearbyPerson } from '../../world-nearby/projection'
import { canonicalId, validateWorldRooms } from './contract'
export interface ReplicationReader { read(): ReplicationView | null; subscribe(listener: () => void): () => void; switchRoom(roomId: string): Promise<boolean> }
/** Captured at canonical bootstrap/replication authorization. Never restamp an old roster. */
export interface WorldAccessRelease { readonly scope: PrivacyScope; readonly capabilities: ShellCapabilities; readonly replication: ReplicationReader }
export interface WorldAccessPort { read(): WorldAccessRelease | null; subscribe(listener: () => void): () => void }
export type RoomError = 'denied' | 'missing' | 'rate' | 'invalid' | 'unavailable'
export interface WorldAccessState {
  readonly rooms: { readonly phase: 'idle' | 'loading' | 'ready' | 'error'; readonly data: WorldRooms | null; readonly error: RoomError | null; readonly retryAt: number }
  readonly nearby: readonly NearbyPerson[]; readonly phase: ReplicationView['phase']; readonly currentRoomId: string | null
  readonly switch: { readonly phase: 'idle' | 'switching' | 'admitted' | 'rejected' | 'interrupted'; readonly targetId: string | null }
  readonly available: boolean; readonly partyNavigation: boolean
}
const initial = (): WorldAccessState => ({ rooms: { phase: 'idle', data: null, error: null, retryAt: 0 }, nearby: [], phase: 'unavailable', currentRoomId: null, switch: { phase: 'idle', targetId: null }, available: false, partyNavigation: false })
const same = (a: PrivacyScope, b: PrivacyScope) => a.generation === b.generation && a.userId === b.userId && a.eventId === b.eventId && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const sameBaseline = (a: NonNullable<ReplicationView['baseline']>, b: NonNullable<ReplicationView['baseline']>) => a.streamId === b.streamId && a.roomEpoch === b.roomEpoch && a.roomId === b.roomId
export const roomErrorText: Record<RoomError, string> = { denied: 'Your current access does not allow room discovery.', missing: 'Room information is unavailable or no longer accessible.', rate: 'Too many requests. Wait before reading rooms again.', invalid: 'Room information could not be verified.', unavailable: 'Rooms are temporarily unavailable. Try reading again later.' }
export function createWorldAccessController({ auth, world, now = Date.now }: { auth: Pick<SessionController, 'session' | 'privacy' | 'request'>; world: WorldAccessPort; now?: () => number }) {
  let state = immutable(initial()), disposed = false, started = false, roomsStarted = false, request: AbortController | null = null
  let release: WorldAccessRelease | null = null, stopReplication = () => {}, pending: { release: WorldAccessRelease; baseline: NonNullable<ReplicationView['baseline']>; targetId: string } | null = null
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<WorldAccessState>) => {
    if (disposed) return
    state = immutable({ ...state, ...patch }); const observers = Array.from(listeners)
    for (const observer of observers) { try { observer() } catch { /* Keep independent private views evictable. */ } }
  }
  function authorized(): WorldAccessRelease | null {
    if (disposed) return null
    const value = world.read(), scope = auth.privacy.capture()
    return value && canonicalId(scope.userId) && canonicalId(scope.eventId) && same(value.scope, scope) && auth.privacy.isCurrent(scope)
      && validCapabilities(value.capabilities) && value.capabilities.world.available ? value : null
  }
  function resetPrivate() { request?.abort(); request = null; pending = null; publish(initial()) }
  function clear() { stopReplication(); stopReplication = () => {}; release = null; resetPrivate() }
  function viewFor(value: WorldAccessRelease) {
    const view = value.replication.read(), s = value.scope
    if (!view) return null
    const b = view.baseline
    return view.scope.generation === s.generation && view.scope.userId === s.userId && view.scope.eventId === s.eventId
      && (!b || b.eventId === s.eventId && b.selfUserId === s.userId) ? view : null
  }
  function syncReplication() {
    const value = authorized()
    if (!release || !value || !same(value.scope, release.scope) || value.replication !== release.replication) { clear(); return }
    const view = viewFor(release)
    if (!view) { resetPrivate(); return }
    const b = view.baseline
    let switching = state.switch
    if (pending) {
      if (!b || !view.visible || !['live', 'switching'].includes(view.phase)) {
        switching = { phase: 'interrupted', targetId: pending.targetId }; pending = null
      } else if (!sameBaseline(b, pending.baseline)) {
        const old = pending.baseline
        const admitted = view.phase === 'live' && b.cause === 'ROOM_SWITCH' && canonicalId(b.requestId) && b.roomId === pending.targetId && b.streamId !== old.streamId
          && b.mapId === old.mapId && b.mapRevision === old.mapRevision && b.avatarManifestId === old.avatarManifestId && b.avatarManifestVersion === old.avatarManifestVersion
        switching = { phase: admitted ? 'admitted' : 'interrupted', targetId: pending.targetId }; pending = null
      } else if (view.phase === 'live' && view.problem === 'room-unavailable') {
        switching = { phase: 'rejected', targetId: pending.targetId }; pending = null
      }
    }
    publish({ phase: view.phase, nearby: projectNearby(view, release.scope), currentRoomId: b?.roomId ?? null, switch: switching,
      available: true, partyNavigation: value.capabilities.meet.available })
    if (roomsStarted && state.rooms.phase === 'idle') void loadRooms()
  }
  function bind() {
    const next = authorized()
    if (!next) { clear(); return }
    if (!release || !same(release.scope, next.scope) || release.replication !== next.replication) {
      clear(); release = next; stopReplication = next.replication.subscribe(syncReplication)
    }
    syncReplication()
    if (roomsStarted && state.rooms.phase === 'idle') void loadRooms()
  }
  const stopPrivacy = auth.privacy.registerCleanup(clear)
  const stopWorld = world.subscribe(() => { if (started) bind() })
  async function loadRooms() {
    if (disposed) return
    if (!started) { started = true; bind() }
    roomsStarted = true
    const value = authorized()
    if (!value || !viewFor(value) || value.replication !== release?.replication || state.rooms.phase === 'loading' || state.rooms.retryAt > now()) return
    request?.abort(); const attempt = new AbortController(); request = attempt
    const scope = value.scope, path = `/api/concert/events/${scope.eventId}/rooms`
    publish({ rooms: { phase: 'loading', data: null, error: null, retryAt: 0 } })
    try {
      const data = await auth.privacy.run(scope, async ({ signal }) => {
        const result = await auth.request({ path, method: 'GET', access: 'protected', errorFamily: 'core', signal, decode: validateWorldRooms(scope) })
        if (!auth.session.isCurrent(result.scope)) throw new Error('Expired room result')
        return result.data
      }, attempt.signal)
      if (!disposed && request === attempt && auth.privacy.isCurrent(scope) && authorized()?.replication === value.replication && viewFor(value)) publish({ rooms: { phase: 'ready', data, error: null, retryAt: 0 } })
    } catch (error) {
      if (disposed || request !== attempt || !auth.privacy.isCurrent(scope)) return
      const e = error instanceof TransportError ? error : null
      const reason: RoomError = e?.kind === 'invalid-response' ? 'invalid' : e?.status === 401 || e?.status === 403 ? 'denied' : e?.status === 404 ? 'missing' : e?.status === 429 ? 'rate' : 'unavailable'
      const delay = reason === 'rate' ? Math.min(300000, Math.max(1000, e?.retryAfterMs ?? 1000)) : 0
      publish({ rooms: { phase: 'error', data: null, error: reason, retryAt: delay ? now() + delay : 0 } })
    } finally { if (request === attempt) request = null }
  }
  function switchReason(roomId: string): string | null {
    const value = authorized(), view = value && viewFor(value)
    if (!value || value.replication !== release?.replication || !view || !view.visible || view.phase !== 'live' || !view.baseline) return 'Room switching needs a current live world connection.'
    if (pending || state.switch.phase === 'switching') return 'Wait for the current room switch to finish.'
    if (state.rooms.phase !== 'ready') return 'Read the current room information first.'
    const room = state.rooms.data?.items.find(r => r.room_id === roomId)
    if (!room || !canonicalId(roomId)) return 'This room is not in the current room catalog.'
    if (roomId === view.baseline.roomId) return 'You are in this room.'
    if (room.availability === 'FULL') return 'This room is full.'
    if (room.availability !== 'AVAILABLE') return 'This room is unavailable.'
    return null
  }
  async function switchRoom(roomId: string) {
    if (switchReason(roomId)) return
    const value = authorized()!, baseline = value.replication.read()?.baseline
    if (!baseline) return
    const attempt = { release: value, baseline, targetId: roomId }; pending = attempt
    publish({ switch: { phase: 'switching', targetId: roomId } })
    if (pending !== attempt || !auth.privacy.isCurrent(value.scope) || authorized()?.replication !== value.replication) return
    try {
      const sent = await value.replication.switchRoom(roomId)
      if (disposed || pending !== attempt || !auth.privacy.isCurrent(value.scope)) return
      syncReplication()
      if (!sent && pending === attempt) { pending = null; publish({ switch: { phase: 'rejected', targetId: roomId } }) }
      // A true result is dispatch only. Admission is exclusively the matching replacement baseline above.
    } catch {
      if (!disposed && pending === attempt && auth.privacy.isCurrent(value.scope)) { pending = null; publish({ switch: { phase: 'rejected', targetId: roomId } }) }
    }
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    start() { if (started || disposed) return; started = true; bind() },
    loadRooms, switchRoom, switchReason,
    dispose() { if (disposed) return; clear(); disposed = true; stopPrivacy(); stopWorld(); listeners.clear() },
  }
}
export type WorldAccessController = ReturnType<typeof createWorldAccessController>
