import { createMuralEditor } from '../editor/controller'
import type { MuralCommandPort, PixelCommand, PrivacyPort } from '../editor/contract'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { bindingSchema, clock as defaultClock, matchesPixel, sameBinding, sameScope } from './contract'
import type { Clock, MuralAuthorityPort, MuralBinding, MuralReadLease, MuralReadPort } from './contract'
import { createMuralProjection } from './projection'
import type { MuralSink } from '../../../world/mural/sink'
import { matchActionReceipt } from '../editor/tools'
import type { WallActionCommand, WallToolPort } from '../editor/tools'

export interface MuralSyncState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'gap' | 'unavailable' | 'error' | 'disposed'
  readonly revision: number | null
  readonly queued: number
  readonly problem: 'authority' | 'snapshot' | 'delta' | 'gap' | 'reset' | 'overflow' | 'timeout' | 'sink' | null
}
/** One shell-owned mural consumer; HTTP polling uses the incumbent world's authority, never a second socket. */
export function createMuralSync({ privacy, authority, reads, commands, tools, sink, clock = defaultClock, pollMs = 2000, gapMs = 200, timeoutMs = 8000, newKey }: {
  privacy: PrivacyPort; authority: MuralAuthorityPort; reads: MuralReadPort; commands: MuralCommandPort; sink: MuralSink
  tools?: WallToolPort
  clock?: Clock; pollMs?: number; gapMs?: number; timeoutMs?: number; newKey?: () => string
}) {
  if (![pollMs, gapMs, timeoutMs].every(Number.isInteger) || pollMs < 1000 || pollMs > 60000 || gapMs < 1 || gapMs > 1000 || timeoutMs < 1 || timeoutMs > 60000) throw new Error('Invalid mural timing')
  const scope: PrivacyScope = Object.freeze({ ...privacy.capture() }), projection = createMuralProjection(scope.eventId ?? '')
  let started = false, ended = false, generation = 0, binding: MuralBinding | null = null, writable = false
  let phase: MuralSyncState['phase'] = 'idle', problem: MuralSyncState['problem'] = null
  let poll: unknown = null, gap: unknown = null, active: AbortController | null = null, refreshing = false
  let serverAnchor = 0, localAnchor = 0, lastSnapshot = ''
  let state: MuralSyncState = Object.freeze({ phase, problem, revision: null, queued: 0 })
  let stopAuthority = () => {}, stopPrivacy = () => {}, stopSink = () => {}
  const listeners = new Set<() => void>()
  const current = () => started && !ended && privacy.isCurrent(scope) && scope.userId !== null && scope.eventId !== null
  const freshAuthority = () => {
    const value = authority.read()
    return value && bindingSchema.safeParse(value.binding).success && value.binding.eventId === scope.eventId ? value : null
  }
  const lease = (): MuralReadLease | null => current() && binding ? Object.freeze({ scope, binding, generation }) : null
  const valid = (candidate: MuralReadLease | null): candidate is MuralReadLease => !!candidate && current() && candidate.generation === generation
    && sameScope(candidate.scope, scope) && sameBinding(candidate.binding, binding) && sameBinding(candidate.binding, freshAuthority()?.binding ?? null)
  const stopTimers = () => { if (poll !== null) clock.clear(poll); if (gap !== null) clock.clear(gap); poll = gap = null }
  const notify = () => {
    state = Object.freeze({ phase, problem, revision: projection.read()?.revision ?? null, queued: projection.queued() })
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* A faulty view cannot block privacy eviction. */ } }
  }
  const commandPort: MuralCommandPort = {
    place: (command, signal) => runCommand(command, signal, false),
    check: (command, signal) => runCommand(command, signal, true),
  }
  const toolPort: WallToolPort | undefined = tools ? {
    async load(expected, signal) {
      const candidate = lease()
      if (!valid(candidate) || !sameScope(expected, scope) || signal.aborted) throw new Error('Mural tool authority unavailable')
      const context = await tools.load(expected, signal)
      if (!valid(candidate) || signal.aborted) throw new Error('Mural tool authority changed')
      return context
    },
    submit: (command, signal) => runToolCommand(command, signal, false),
    check: (command, signal) => runToolCommand(command, signal, true),
  } : undefined
  const editor = createMuralEditor({ privacy, commands: commandPort, ...(toolPort ? { tools: toolPort, toolTimers: clock } : {}), now: clock.now, ...(newKey ? { newKey } : {}) })
  function withdraw() {
    projection.invalidate(); lastSnapshot = ''
    // FE32's closed DTO boundary atomically removes the confirmed raster while retaining unknown commands.
    editor.acceptSnapshot(scope, null)
    try { sink.replace(null) } catch { /* Continue releasing other resources. */ }
  }
  function render(reset = false) {
    if (!current()) return
    const snapshot = projection.read()
    if (!snapshot || !binding) { notify(); return }
    try {
      const serverTime = new Date(serverAnchor + Math.max(0, clock.now() - localAnchor)).toISOString()
      editor.acceptSnapshot(scope, { ...snapshot, server_time: serverTime }, { reset })
      const desired = phase !== 'ready' ? 'error' : writable ? 'writable' : 'read-only'
      if (editor.read().availability !== desired) editor.setAvailability(scope, desired)
      sink.replace({ binding, revision: snapshot.revision, state: snapshot.state, palette: snapshot.palette, pixels: snapshot.pixels })
    } catch { fail('sink'); return }
    notify()
  }
  function fail(reason: MuralSyncState['problem']) {
    if (!current()) return
    ++generation; active?.abort(); active = null; refreshing = false; stopTimers(); phase = 'error'; problem = reason
    withdraw(); notify()
  }
  function schedulePoll() {
    if (!current() || !binding || poll !== null || refreshing || active || projection.read()?.state === 'FROZEN') return
    poll = clock.set(pollMs, () => { poll = null; void fetchDelta() })
  }
  async function boundedRead<T>(owner: AbortController, work: () => Promise<T>): Promise<T> {
    let timer: unknown = null, onAbort = () => {}
    try {
      const stopped = new Promise<never>((_, reject) => {
        onAbort = () => reject(new Error('Mural read stopped'))
        owner.signal.addEventListener('abort', onAbort, { once: true })
        timer = clock.set(timeoutMs, () => owner.abort())
        if (owner.signal.aborted) onAbort()
      })
      if (owner.signal.aborted) throw new Error('Mural read stopped')
      return await Promise.race([work(), stopped])
    } finally { if (timer !== null) clock.clear(timer); owner.signal.removeEventListener('abort', onAbort) }
  }
  function consume(candidate: MuralReadLease, input: unknown) {
    if (!valid(candidate)) return false
    const outcome = projection.delta(input)
    if (outcome === 'invalid' || outcome === 'overflow' || outcome === 'reset') {
      phase = 'loading'; problem = outcome === 'invalid' ? 'delta' : outcome; void refresh(true); return false
    }
    if (outcome === 'gap') {
      phase = 'gap'; problem = 'gap'; editor.setAvailability(scope, 'error')
      if (!refreshing && gap === null) gap = clock.set(gapMs, () => { gap = null; if (valid(candidate)) { projection.invalidate(); problem = 'gap'; void refresh(true) } })
    } else {
      if (gap !== null) clock.clear(gap); gap = null
      phase = refreshing ? 'loading' : 'ready'; problem = null
    }
    render(); return true
  }
  async function refresh(reset = false): Promise<boolean> {
    if (!current() || !binding || !sameBinding(binding, freshAuthority()?.binding ?? null)) return false
    if (poll !== null) clock.clear(poll); if (gap !== null) clock.clear(gap); poll = gap = null
    active?.abort(); ++generation
    const candidate = lease()!, owner = new AbortController(); active = owner; refreshing = true; phase = 'loading'
    editor.setAvailability(scope, 'loading')
    if (reset) { projection.invalidate(); withdraw() }
    notify()
    try {
      const input = await boundedRead(owner, () => reads.snapshot({ lease: candidate, signal: owner.signal }))
      if (!valid(candidate) || owner.signal.aborted) return false
      const outcome = projection.seed(input), snapshot = projection.read()
      if (outcome === 'invalid' || !snapshot || outcome === 'stale' && !snapshot) { fail('snapshot'); return false }
      if (outcome === 'accepted') {
        const raw = input as { server_time: string; snapshot_id: string }
        serverAnchor = Date.parse(raw.server_time); localAnchor = clock.now()
        reset ||= lastSnapshot !== '' && lastSnapshot !== raw.snapshot_id
        lastSnapshot = raw.snapshot_id
      }
      refreshing = false; phase = projection.queued() ? 'gap' : 'ready'; problem = projection.queued() ? 'gap' : null
      render(reset)
      if (projection.queued() && gap === null) gap = clock.set(gapMs, () => { gap = null; if (valid(candidate)) { projection.invalidate(); void refresh(true) } })
      return true
    } catch { if (valid(candidate)) fail(owner.signal.aborted ? 'timeout' : 'snapshot'); return false }
    finally { if (active === owner) active = null; if (valid(candidate)) schedulePoll() }
  }
  async function fetchDelta() {
    const candidate = lease(), snapshot = projection.read()
    if (!valid(candidate) || !snapshot || active) return
    const owner = new AbortController(); active = owner
    try {
      const data = await boundedRead(owner, () => reads.delta({ lease: candidate, signal: owner.signal, afterRevision: snapshot.revision }))
      if (valid(candidate) && !owner.signal.aborted) consume(candidate, data)
    } catch { if (valid(candidate)) fail(owner.signal.aborted ? 'timeout' : 'delta') }
    finally { if (active === owner) active = null; if (valid(candidate)) schedulePoll() }
  }
  async function runCommand(command: PixelCommand, signal: AbortSignal, check: boolean) {
    const candidate = lease()
    if (!valid(candidate) || !sameScope(command.scope, scope) || !check && (phase !== 'ready' || !writable)) return { kind: 'unknown' as const }
    const response = await (check ? commands.check(command, signal) : commands.place(command, signal))
    if (!valid(candidate) || signal.aborted) return { kind: 'unknown' as const }
    if (response.kind === 'acknowledged') {
      const p = matchesPixel(response.pixel, command)
      if (!p) return { kind: 'unknown' as const }
      if (projection.own(p)) { serverAnchor = Math.max(serverAnchor + Math.max(0, clock.now() - localAnchor), Date.parse(p.committed_at)); localAnchor = clock.now(); render() }
      else { projection.invalidate(); void refresh(true) }
    }
    return response
  }
  async function runToolCommand(command: WallActionCommand, signal: AbortSignal, check: boolean) {
    const candidate = lease()
    if (!tools || !valid(candidate) || !sameScope(command.scope, scope) || signal.aborted
      || !check && (phase !== 'ready' || !writable)) return { kind: 'unknown' as const }
    const response = await (check ? tools.check(command, signal) : tools.submit(command, signal))
    if (!valid(candidate) || signal.aborted) return { kind: 'unknown' as const }
    if (response.kind === 'acknowledged') {
      if (!matchActionReceipt(response.receipt, command)) return { kind: 'unknown' as const }
      // Action receipts are never projected as fabricated single-pixel deltas.
      // All B actions cross the existing reset/snapshot/world-sink boundary.
      projection.invalidate(); void refresh(true)
    }
    return response
  }
  function changedAuthority() {
    if (!current()) return
    const next = freshAuthority()
    if (!next || !sameBinding(binding, next.binding)) {
      ++generation; active?.abort(); active = null; refreshing = false; stopTimers(); withdraw()
      binding = next ? Object.freeze({ ...next.binding }) : null; writable = next?.writable === true
      phase = binding ? 'loading' : 'unavailable'; problem = binding ? null : 'authority'; notify()
      if (binding) void refresh(true)
    } else if (writable !== next.writable) {
      writable = next.writable; editor.setAvailability(scope, writable && phase === 'ready' ? 'writable' : 'read-only'); render(true)
    }
  }
  function dispose() {
    if (ended) return
    ended = true; ++generation; active?.abort(); active = null; stopTimers(); binding = null; projection.dispose()
    for (const cleanup of [stopAuthority, stopPrivacy, stopSink, editor.dispose, () => sink.dispose()]) {
      try { cleanup() } catch { /* Continue releasing every independent owner after a failed resource cleanup. */ }
    }
    phase = 'disposed'; problem = null; notify(); listeners.clear()
  }
  return {
    editor, read: () => state, scope, lease,
    subscribe(listener: () => void) { if (ended) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    start() {
      if (started || ended) return
      started = true
      if (!privacy.isCurrent(scope) || !scope.userId || !scope.eventId) { dispose(); return }
      stopPrivacy = privacy.registerCleanup(dispose); stopAuthority = authority.subscribe(changedAuthority)
      stopSink = sink.subscribeFailure?.(() => fail('sink')) ?? (() => {})
      changedAuthority()
    },
    refresh: () => refresh(false),
    /** In-process qualified delta delivery, NOT a new public WS message. Lease is captured before the read/delivery. */
    receiveDelta: consume,
    dispose,
  }
}
export type MuralSyncController = ReturnType<typeof createMuralSync>
