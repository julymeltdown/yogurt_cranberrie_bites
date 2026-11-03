import { describe, expect, it } from 'vitest'
import { createWorldMuralAuthority } from '../authority'
import { fixture, ids } from '../../../features/mural/sync/__tests__/fixture'
import type { ReplicationView } from '../../network/replication'
describe('FE33 observes the existing world release without taking transport ownership', () => {
  it('requires current live visible identity, exact baseline and explicit write capability', () => {
    const f = fixture(), listeners = new Set<() => void>()
    let view = { scope: f.privacy.capture(), phase: 'live', visible: true, baseline: { eventId: ids.event, roomId: ids.room, roomEpoch: ids.epoch, streamId: ids.stream } } as unknown as ReplicationView
    const world = { read: () => view, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } } }
    const authority = createWorldMuralAuthority({ privacy: f.privacy, world })
    expect(authority.read()).toMatchObject({ writable: false, binding: { roomEpoch: ids.epoch } })
    view = { ...view, visible: false }; expect(authority.read()).toBeNull()
    view = { ...view, visible: true, phase: 'switching' }; expect(authority.read()).toBeNull()
    view = { ...view, phase: 'live' }; expect(createWorldMuralAuthority({ privacy: f.privacy, world, writable: () => true }).read()!.writable).toBe(true)
    let notified = 0; const remove = authority.subscribe(() => notified++)
    expect(listeners.size).toBe(1); f.privacy.changePolicy(f.privacy.capture(), 'block')
    expect(authority.read()).toBeNull(); expect(notified).toBe(1); remove(); expect(listeners.size).toBe(0); f.dispose()
  })
})
