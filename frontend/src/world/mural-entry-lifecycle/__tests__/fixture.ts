import { entryFixture, ids, drain } from '../../../app/mural-entry/__tests__/fixtures'
import { snapshot as worldSnapshot } from '../../composition/__tests__/fixtures/data'
import type { ReplicationView } from '../../network/replication'
import { createMuralEntryLifecycle, type MuralEntryQualification } from '../index'
export { ids }
export const settle = async () => { for (let i = 0; i < 20; i++) await drain() }

/** Synthetic observations of one already admitted world. No connection/engine is created. */
export function fixture(connectEntry = true) {
  const f = entryFixture(), binding = { eventId: ids.event, roomId: ids.room, roomEpoch: ids.epoch, streamId: ids.stream }
  let view: ReplicationView | null = { scope: f.scope, phase: 'live', visible: true, baseline: { ...worldSnapshot(ids.room), ...binding },
    entities: [], pendingInputs: 0, ackInputSeq: 0, spatialPrediction: 'unavailable', problem: null }
  const listeners = new Set<() => void>(), calls: { anchor: MuralEntryQualification['anchor']; key: string | undefined }[] = []
  const world = { read: () => view, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } } }
  const lifecycle = createMuralEntryLifecycle({ privacy: f.auth.privacy, world }), owner = f.create()
  const stop = connectEntry ? lifecycle.subscribe(() => owner.bind(lifecycle.read())) : () => {}
  function qualify() {
    const r = f.release()
    const input: MuralEntryQualification = { scope: f.scope, binding: { ...binding }, signal: r.lifetime.signal, authority: r.value.authority,
      anchor: { x: 16, y: 24, width: 128, height: 128, depth: 1 }, textureKey: 'fixture-only-wall',
      scene(anchor, key) { calls.push({ anchor, key }); return r.value.scene } }
    return { ...r, input }
  }
  return { ...f, binding, world, lifecycle, owner, qualify, calls, worldCount: () => listeners.size,
    view: () => view!, setView(next: ReplicationView | null, notify = true) { view = next; if (notify) for (const fn of Array.from(listeners)) fn() },
    dispose() { lifecycle.dispose(); stop(); f.dispose() } }
}
