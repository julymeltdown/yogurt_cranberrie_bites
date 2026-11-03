import { deltaSchema, pixelOperationSchema, snapshotSchema } from './contract'
import type { PublicPixel } from './contract'
import type { MuralSnapshot } from '../editor/contract'

const immutable = (input: MuralSnapshot) => {
  Object.freeze(input.pixels); Object.freeze(input.palette); return Object.freeze(input)
}
const fingerprint = (p: PublicPixel) => JSON.stringify(p)
export type DeltaOutcome = 'applied' | 'duplicate' | 'gap' | 'reset' | 'invalid' | 'overflow'
/** Bounded reconciliation only. Epoch/request/privacy ownership is the caller's responsibility. */
export function createMuralProjection(eventId: string, maxQueue = 512) {
  if (!Number.isInteger(maxQueue) || maxQueue < 1 || maxQueue > 512) throw new Error('Invalid queue bound')
  let snapshot: MuralSnapshot | null = null, floor = 1, frozen = false
  const queue = new Map<number, PublicPixel>(), history = new Map<number, PublicPixel>()
  const remember = (p: PublicPixel) => { history.set(p.revision, p); if (history.size > 512) history.delete(history.keys().next().value!) }
  function drain(): boolean {
    if (!snapshot || !queue.has(snapshot.revision + 1)) return false
    if (snapshot.state !== 'EDITABLE') { invalidate(); return false }
    const pixels = [...snapshot.pixels]
    let next = snapshot.revision
    while (queue.has(next + 1)) {
      const p = queue.get(++next)!; queue.delete(next); pixels[p.y * 128 + p.x] = p.color_index; remember(p)
    }
    floor = Math.max(floor, next); snapshot = immutable({ ...snapshot, pixels, revision: next }); return true
  }
  function invalidate(minimum = floor) {
    floor = Math.max(floor, minimum, snapshot?.revision ?? 1); snapshot = null; queue.clear(); history.clear()
  }
  function delta(input: unknown): DeltaOutcome {
    // Reject oversize batches before invoking a per-item decoder.
    if (input && typeof input === 'object' && 'operations' in input && Array.isArray(input.operations) && input.operations.length > 512) { invalidate(); return 'overflow' }
    const parsed = deltaSchema.safeParse(input)
    if (!parsed.success || parsed.data.event_id !== eventId) { invalidate(); return 'invalid' }
    const d = parsed.data
    if (d.reset_required) { invalidate(d.to_revision); return 'reset' }
    let changed = false
    for (const p of d.operations) {
      if (p.revision <= (snapshot?.revision ?? floor)) {
        const old = history.get(p.revision)
        if (old && fingerprint(old) !== fingerprint(p)) { invalidate(); return 'invalid' }
        continue
      }
      const old = queue.get(p.revision)
      if (old && fingerprint(old) !== fingerprint(p)) { invalidate(); return 'invalid' }
      if (Array.from(history.values()).some(old => old.operation_id === p.operation_id && old.revision !== p.revision)
        || Array.from(queue.values()).some(old => old.operation_id === p.operation_id && old.revision !== p.revision)) { invalidate(); return 'invalid' }
      if (!old) {
        if (queue.size >= maxQueue) { invalidate(); return 'overflow' }
        queue.set(p.revision, Object.freeze(p)); changed = true
      }
    }
    const applied = drain()
    if (!snapshot || queue.size > 0 || d.to_revision > snapshot.revision) return 'gap'
    return applied ? 'applied' : changed ? 'gap' : 'duplicate'
  }
  return {
    read: () => snapshot, queued: () => queue.size, minimum: () => floor, invalidate,
    seed(input: unknown): 'accepted' | 'stale' | 'invalid' {
      const parsed = snapshotSchema.safeParse(input)
      if (!parsed.success || parsed.data.event_id !== eventId || frozen && parsed.data.state !== 'FROZEN') return 'invalid'
      const next = parsed.data
      if (next.revision < floor || snapshot && next.revision < snapshot.revision) return 'stale'
      if (snapshot && next.revision === snapshot.revision && (next.state !== snapshot.state
        || next.palette.some((v, i) => v !== snapshot!.palette[i]) || next.pixels.some((v, i) => v !== snapshot!.pixels[i]))) return 'invalid'
      frozen ||= next.state === 'FROZEN'; floor = Math.max(floor, next.revision); snapshot = immutable(next)
      for (const revision of queue.keys()) if (revision <= next.revision) queue.delete(revision)
      history.clear(); drain(); return 'accepted'
    },
    delta,
    own(input: unknown): boolean {
      const parsed = pixelOperationSchema.safeParse(input)
      if (!parsed.success || parsed.data.event_id !== eventId) return false
      const { next_allowed_at, ...operation } = parsed.data
      const result = delta({ event_id: eventId, from_revision: operation.revision - 1, to_revision: operation.revision, reset_required: false, operations: [operation] })
      if (!snapshot || !['applied', 'duplicate'].includes(result)) return false
      snapshot = immutable({ ...snapshot, server_time: Date.parse(snapshot.server_time) > Date.parse(operation.committed_at) ? snapshot.server_time : operation.committed_at,
        next_allowed_at: snapshot.next_allowed_at && Date.parse(snapshot.next_allowed_at) > Date.parse(next_allowed_at) ? snapshot.next_allowed_at : next_allowed_at }); return true
    },
    dispose() { snapshot = null; floor = 1; frozen = false; queue.clear(); history.clear() },
  }
}
