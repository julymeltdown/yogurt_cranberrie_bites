import type { SessionPort, SessionScope } from '../../shared/api/http'
import type { Input, Resync, Scheduler, Snapshot, WorldEvent, WorldResources, WorldState, WorldTransport } from './WorldTransport'
import { policy, validate } from './transport/contract'
import { createEntityProjection, immutable, type EntitySink, type ReplicatedEntity } from '../entities/replicated'
import { createMotionPrediction, qualifyMotionContext, type MotionContext, type MotionInput, type QualifiedMotionContext } from './prediction'

export interface ReplicationView {
  readonly scope: SessionScope
  readonly phase: WorldState | 'suspended' | 'unavailable'
  readonly visible: boolean
  readonly baseline: (Omit<Snapshot, 'players' | 'streamSeq'> & { readonly streamSeq: number }) | null
  readonly entities: readonly ReplicatedEntity[]
  readonly pendingInputs: number
  readonly ackInputSeq: number
  /** Available means bounded tentative presentation, never server movement authority. */
  readonly spatialPrediction: 'unavailable' | 'bounded'
  readonly problem: 'connection' | 'room-unavailable' | 'authority' | 'map-version' | 'sink' | null
}
export interface ReplicationOptions {
  readonly session: SessionPort
  /** Exclusive ownership transfers to this controller; pass an idle transport, not a shared connection. */
  readonly transport: WorldTransport
  readonly resources: WorldResources
  readonly sink: EntitySink<ReplicationView>
  readonly scheduler?: Scheduler
  readonly requestId?: () => string
  readonly motion?: MotionContext
}
const clock: Scheduler = { now: () => performance.now(), set: (ms, fn) => setTimeout(fn, ms), clear: h => clearTimeout(h as ReturnType<typeof setTimeout>) }
const emptyIntent = () => ({ axisX: 0, axisY: 0, interact: false })
const cadence = 1000 / policy.simulation_hz_target

/** One FE16 consumer. No socket, HTTP mutation, heartbeat timer, reconnect loop or renderer. */
export function createWorldReplication(options: ReplicationOptions) {
  const transport = options.transport, scheduler = options.scheduler ?? clock
  const captured = options.session.read()
  const scope = immutable<SessionScope>({ generation: captured.generation, userId: captured.userId, eventId: captured.eventId })
  let resources = { ...options.resources }, started = false, disposed = false, ended = false, visible = true, resume = false
  let generation = 0, phase: ReplicationView['phase'] = 'idle', problem: ReplicationView['problem'] = null
  let baseline: ReplicationView['baseline'] = null, sequence = 0, ack = 0
  let intent = emptyIntent(), pending: MotionInput[] = [], resyncPending = false, lastResync = -Infinity
  let motionContext: QualifiedMotionContext | null = null
  let prediction: ReturnType<typeof createMotionPrediction> = null
  // Clone before the first async boundary; a caller cannot swap geometry during qualification.
  const motion = options.motion ? structuredClone(options.motion) : null
  let motionReady: Promise<void> | null = null
  let lastSent: { x: number; y: number } | null = null
  let inputTimer: unknown = null, resyncTimer: unknown = null, connection: AbortController | null = null
  let removeTransport = () => {}, removeScope = () => {}, withdrawing = false
  const listeners = new Set<() => void>(), entities = createEntityProjection(1000 / policy.delta_hz_max_target)
  let view = snapshot()
  function current() { return started && !disposed && !ended && options.session.isCurrent(scope) }
  function snapshot(): ReplicationView {
    const predicted = prediction?.sample(scheduler.now(), pending) ?? null
    return immutable({ scope, phase, visible, baseline, entities: entities.render(scheduler.now(), predicted), pendingInputs: pending.length,
      ackInputSeq: ack, spatialPrediction: prediction ? 'bounded' : 'unavailable', problem })
  }
  function stopTimers() {
    if (inputTimer !== null) scheduler.clear(inputTimer)
    if (resyncTimer !== null) scheduler.clear(resyncTimer)
    inputTimer = resyncTimer = null
  }
  function clear(keepBaseline = false, keepHeld = false) {
    stopTimers(); entities.clear(); pending = []; prediction = null; sequence = ack = 0; resyncPending = false; lastSent = null
    if (!keepBaseline) baseline = null
    intent = keepHeld ? { ...intent, interact: false } : emptyIntent()
  }
  function publish() {
    const g = generation
    view = snapshot()
    try { options.sink.replace(view) } catch { terminate('sink'); return }
    for (const listener of Array.from(listeners)) {
      if (g !== generation) break
      try { listener() } catch { /* Independent observers cannot strand authority. */ }
    }
  }
  function terminate(reason: 'authority' | 'sink', final = false) {
    if (disposed || ended) return
    ended = true; disposed = final; ++generation; resume = false; clear()
    removeTransport(); removeScope(); connection?.abort(); connection = null
    transport.dispose(); phase = final ? 'disposed' : 'unavailable'; problem = final ? null : reason
    view = snapshot()
    try { options.sink.replace(view) } catch { /* A failing sink is detached permanently. */ }
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Best effort eviction. */ } }
    listeners.clear()
  }
  function active() { return current() && visible && baseline !== null && ['live', 'switching'].includes(transport.state) && phase !== 'resync' }
  function requestResync(reason: Resync['reason']) {
    if (!current() || !visible || !baseline || resyncPending) return
    clear(true, true); phase = 'resync'; resyncPending = true; publish()
    if (!current()) return
    const g = generation
    const send = () => {
      resyncTimer = null
      if (!current() || generation !== g || !visible || !resyncPending) return
      try {
        lastResync = scheduler.now()
        transport.resync((options.requestId ?? (() => crypto.randomUUID()))(), reason)
      } catch { connectionFailure() }
    }
    const wait = Math.max(0, policy.resync_min_interval_ms - (scheduler.now() - lastResync))
    if (wait) resyncTimer = scheduler.set(wait, send); else send()
  }
  function connectionFailure() {
    if (!current()) return
    ++generation; clear(); phase = 'closed'; problem = 'connection'; resume = false
    withdraw(); publish()
  }
  function withdraw() {
    withdrawing = true
    try { connection?.abort(); connection = null; transport.disconnect() }
    finally { withdrawing = false }
  }
  function scheduleInput() {
    const needed = !lastSent || intent.interact || intent.axisX !== 0 || intent.axisY !== 0
      || intent.axisX !== lastSent.x || intent.axisY !== lastSent.y
    if (!active() || inputTimer !== null || !needed) return
    const g = generation
    inputTimer = scheduler.set(cadence, () => {
      inputTimer = null
      if (!active() || generation !== g) return
      if (pending.length >= policy.outgoing_queue_frames || sequence >= Number.MAX_SAFE_INTEGER) { requestResync('PROTOCOL_RECOVERY'); return }
      const sent = { seq: ++sequence, axisX: intent.axisX, axisY: intent.axisY, buttons: intent.interact ? ['interact'] as const : [] }
      intent.interact = false
      try { transport.sendInput(sent) } catch { connectionFailure(); return }
      if (!active() || generation !== g) return
      lastSent = { x: sent.axisX, y: sent.axisY }
      pending.push({ seq: sent.seq, axisX: sent.axisX, axisY: sent.axisY, sentAt: scheduler.now() }); publish()
      if (generation === g) scheduleInput()
    })
  }
  function onEvent(event: WorldEvent) {
    if (!current() || withdrawing) return
    if (event.type === 'resync-needed') { requestResync(event.reason); return }
    if (event.type === 'error') {
      ++generation; clear(); resume = false; phase = 'closed'; problem = problem === 'map-version' ? problem : 'connection'; publish(); return
    }
    if (event.type === 'state') {
      if (event.state === 'live') {
        // FE16 emits live immediately BEFORE its replacement snapshot. Do not
        // commit the previous room/stream as newly live in that interval. A
        // pre-send switch failure has no replacement frame, so settle it once
        // this delivery finishes. New baseline/disposal invalidates this job.
        const g = generation
        queueMicrotask(() => {
          if (!current() || generation !== g || transport.state !== 'live' || !visible) return
          phase = 'live'; publish(); scheduleInput()
        })
      } else if (event.state === 'resync') {
        // A repeated state during our own resync must not clear its single pending request/timer.
        if (phase !== 'resync') { clear(true, true); phase = 'resync'; publish() }
      } else if (['closed', 'disposed'].includes(event.state)) {
        ++generation; clear(); phase = visible ? event.state : 'suspended'; publish()
      } else {
        phase = event.state; publish()
        if (active()) scheduleInput()
      }
      return
    }
    const frame = event.frame
    if (frame.type === 'ping') {
      try { transport.clientPong(frame.heartbeatId) } catch { connectionFailure() }
    } else if (frame.type === 'snapshot') {
      ++generation; clear(false, true)
      const { players, ...metadata } = frame
      baseline = immutable(structuredClone(metadata)); resources = { ...resources, roomId: frame.roomId }
      entities.reset(players, frame.selfUserId, scheduler.now()); phase = 'live'; problem = null
      if (motionContext && frame.mapId === motionContext.mapId && frame.mapRevision === motionContext.mapRevision && frame.motionProfile && frame.selfMotion) {
        const self = players.find(player => player.userId === frame.selfUserId)
        prediction = createMotionPrediction(motionContext, frame.motionProfile)
        if (!self || self.position.x !== frame.selfMotion.position.x || self.position.y !== frame.selfMotion.position.y
          || !prediction?.correct(frame.selfMotion, frame.serverTick, frame.ackInputSeq, sequence, scheduler.now())) prediction = null
      }
      publish(); scheduleInput()
    } else if (frame.type === 'delta') {
      if (!active() || !baseline) return
      // Ordering/epochs/filtering/roster are FE16's authority, never room-global sequence inference.
      entities.delta(frame.upserts, frame.removes, scheduler.now())
      if (prediction && (!frame.selfMotion || !prediction.correct(frame.selfMotion, frame.serverTick, frame.ackInputSeq, sequence, scheduler.now()))) prediction = null
      // A validated complete anchor remains authoritative even without an upsert or prediction context.
      if (frame.selfMotion) entities.anchorSelf(frame.selfMotion.position)
      baseline = immutable({ ...baseline, streamSeq: frame.streamSeq, serverTick: frame.serverTick, ackInputSeq: frame.ackInputSeq,
        ...(frame.selfMotion ? { selfMotion: frame.selfMotion } : {}) })
      ack = frame.ackInputSeq
      const consumed = frame.selfMotion ? frame.selfMotion.consumedInputSeq : ack
      pending = pending.filter(input => input.seq > consumed); publish()
    } else if (frame.type === 'switchRejected') { phase = 'live'; problem = 'room-unavailable'; publish(); scheduleInput() }
    else if (frame.type === 'revoke') { terminate('authority') }
    else if (frame.type === 'resyncRequired' && ['MAP_VERSION_MISMATCH', 'BACKPRESSURE'].includes(frame.reason)) {
      ++generation; clear(); phase = 'closed'; problem = frame.reason === 'MAP_VERSION_MISMATCH' ? 'map-version' : 'connection'; resume = false; publish()
    }
  }
  async function connect(): Promise<boolean> {
    if (!current() || !visible || !['idle', 'closed'].includes(transport.state)) return false
    ++generation; clear(); phase = 'ticket'; problem = null; lastResync = -Infinity
    connection = new AbortController()
    const attempt = connection
    const g = generation
    publish()
    if (!current() || generation !== g) return false
    try {
      if (motion) {
        motionReady ??= qualifyMotionContext(motion).then(context => { motionContext = context }, () => { motionContext = null })
        await motionReady
        if (!current() || generation !== g || !visible || attempt.signal.aborted) return false
      }
      await transport.connect(resources, attempt.signal)
      return current() && connection === attempt && !attempt.signal.aborted && baseline !== null && visible
    }
    catch { if (current() && generation === g) connectionFailure(); return false }
  }
  return {
    read: () => view,
    subscribe(listener: () => void) { if (ended || disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    start() {
      if (started || ended || disposed) return Promise.resolve(false)
      started = true
      if (!options.session.isCurrent(scope) || !scope.userId || scope.eventId !== resources.eventId || captured.signal.aborted
        || !['idle', 'closed'].includes(transport.state)) { terminate('authority'); return Promise.resolve(false) }
      const changed = () => terminate('authority')
      captured.signal.addEventListener('abort', changed, { once: true }); removeScope = () => captured.signal.removeEventListener('abort', changed)
      removeTransport = transport.subscribe(onEvent)
      if (!visible) { resume = true; phase = 'suspended'; publish(); return Promise.resolve(false) }
      return connect()
    },
    /** Explicit user retry only; no automatic takeover or network retry loop. */
    reconnect() { return problem === 'map-version' ? Promise.resolve(false) : connect() },
    setVisible(value: boolean) {
      if (disposed || ended || visible === value) return
      visible = value
      if (!value) {
        resume = current() && !['closed', 'disposed', 'idle'].includes(transport.state)
        ++generation; clear(); withdraw(); phase = 'suspended'; publish()
      } else if (resume && current()) { resume = false; void connect() }
      else { phase = transport.state; publish() }
    },
    /** Coalesce axes and at most one new interaction; never accept coordinates or domain commands. */
    setIntent(value: Pick<Input, 'axisX' | 'axisY' | 'buttons'>) {
      if (!current() || !visible || !baseline || !['live', 'switching', 'resync'].includes(transport.state)) return false
      try { validate('Input', { ...value, protocolVersion: policy.protocol_version, type: 'input', streamId: baseline.streamId, roomEpoch: baseline.roomEpoch, seq: 1 }) }
      catch { return false }
      intent = { axisX: value.axisX, axisY: value.axisY, interact: active() && (intent.interact || value.buttons.includes('interact')) }
      scheduleInput()
      return true
    },
    /** true means request sent; only a replacement baseline proves target-room admission. */
    async switchRoom(roomId: string): Promise<boolean> {
      if (!active() || phase !== 'live') return false
      const g = generation
      try { await transport.roomSwitch(roomId, (options.requestId ?? (() => crypto.randomUUID()))()); return current() && generation === g }
      catch { if (current() && generation === g) { problem = 'room-unavailable'; publish() }; return false }
    },
    /** Called by the eventual existing renderer's frame loop; no second RAF/timer is installed. */
    sample() { if (active()) publish(); return view },
    dispose() {
      if (disposed) return
      if (ended) { disposed = true; phase = 'disposed'; problem = null; view = snapshot(); return }
      terminate('authority', true)
    },
  }
}
export type WorldReplication = ReturnType<typeof createWorldReplication>
