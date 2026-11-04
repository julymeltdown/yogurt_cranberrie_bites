import type { SessionScope } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import operations from '../../../../../contracts/realtime/v1/operations.json'
import type { WorldEvent, WorldResources, WorldState, WorldTransport, WorldTransportOptions, Scheduler } from '../WorldTransport'
import type { ServerMessage, Snapshot, Delta, MotionProfile, SelfMotion, Input, Resync, SwitchRoom } from './wire-types'
import { decode, decodeTicket, ensure, policy, validate, WorldTransportError } from './contract'
export interface SocketEvent { readonly data?: unknown; readonly code?: number }
export interface SocketPort {
  readonly readyState: number
  readonly bufferedAmount: number
  readonly protocol: string
  addEventListener(type: string, listener: (event: SocketEvent) => void): void
  removeEventListener(type: string, listener: (event: SocketEvent) => void): void
  send(data: string): void
  close(code?: number, reason?: string): void
}
export type SocketFactory = (url: string) => SocketPort
const defaultScheduler: Scheduler = {
  now: () => Date.now(), set: (delay, callback) => setTimeout(callback, delay),
  clear: handle => clearTimeout(handle as ReturnType<typeof setTimeout>),
}
const keys = ['eventId', 'roomId', 'mapId', 'mapRevision', 'avatarManifestId', 'avatarManifestVersion'] as const
const body = (r: WorldResources) => ({ room_id: r.roomId, map_id: r.mapId, map_revision: r.mapRevision,
  avatar_manifest_id: r.avatarManifestId, avatar_manifest_version: r.avatarManifestVersion })
function endpoint(origin: string): string {
  const u = new URL(origin)
  if (u.origin !== origin || u.username || u.password || u.search || u.hash ||
    !(u.protocol === 'https:' || (u.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(u.hostname)))) throw new WorldTransportError('invalid-context')
  return `${u.protocol === 'https:' ? 'wss:' : 'ws:'}//${u.host}/world-ws`
}
const ticketOperation = operations.find(operation => operation.operationId === 'issueWorldTicket')!
const ordinaryTicketReasons = new Set(['PERMISSION_DENIED', 'RESOURCE_UNAVAILABLE', 'RATE_LIMITED', 'INTERNAL_ERROR', 'DEPENDENCY_UNAVAILABLE', 'DEADLINE_EXCEEDED'])
class TicketAcquisitionFailure extends WorldTransportError {
  readonly preserveOldRoom: boolean
  constructor(error: unknown) {
    const http = error instanceof TransportError ? error : null
    super(http?.kind === 'timeout' ? 'timeout' : 'unavailable', http?.kind === 'timeout' ? 4408 : 4500)
    const reasons = ticketOperation.errors as Record<string, readonly string[]>
    this.preserveOldRoom = !!http && (http.kind === 'network' || http.kind === 'timeout' || (http.kind === 'http'
      && !!http.server?.reason && ordinaryTicketReasons.has(http.server.reason)
      && !!reasons[String(http.status)]?.includes(http.server.reason)))
  }
}
function abortable<T>(work: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const cancel = () => reject(new WorldTransportError('cancelled'))
    work.then(resolve, reject).finally(() => signal.removeEventListener('abort', cancel))
    if (signal.aborted) { cancel(); return }
    signal.addEventListener('abort', cancel, { once: true })
  })
}
/** Transport protocol bookkeeping only. No prediction, interpolation, automatic resync/reconnect or rendering. */
export class TransportEngine implements WorldTransport {
  private status: WorldState = 'idle'
  get state() { return this.status }
  private listeners = new Set<(event: WorldEvent) => void>()
  private generation = 0
  private socket: SocketPort | null = null
  private controller: AbortController | null = null
  private release: (() => void)[] = []
  private timers = new Map<string, unknown>()
  private clock: Scheduler
  private url: string
  private scope: SessionScope | null = null
  private resources: WorldResources | null = null
  private baseline: Omit<Snapshot, 'players'> | null = null
  private roster = new Set<string>()
  private streams = new Set<string>()
  private requests = new Set<string>()
  private pending: { sent: boolean; type: 'resync' | 'switchRoom'; requestId: string; roomId: string } | null = null
  private heartbeat: string | null = null
  private sequence = 0
  private inputSequence = 0
  private ack = 0
  private tick = 0
  private lastResync = -Infinity
  private rateTime = 0
  private allTokens: number = policy.all_client_burst
  private inputTokens: number = policy.input_burst
  private serverOffset = 0
  private bufferedFrames = 0
  private usedTickets = new Set<string>()
  private readonly motionProfileVersion: '1.0.0' | undefined
  private motion: { profile: MotionProfile; anchor: SelfMotion; ack: number } | null = null
  private completion: { resolve(): void; reject(error: WorldTransportError): void } | null = null
  constructor(private readonly options: WorldTransportOptions, private readonly factory: SocketFactory) {
    if (options.motionProfileVersion !== undefined && options.motionProfileVersion !== '1.0.0') throw new WorldTransportError('invalid-context')
    this.motionProfileVersion = options.motionProfileVersion
    this.clock = options.scheduler ?? defaultScheduler
    this.url = endpoint(options.origin)
  }
  subscribe(listener: (event: WorldEvent) => void) {
    if (this.status === 'disposed') return () => {}
    this.listeners.add(listener); return () => { this.listeners.delete(listener) }
  }
  private emit(event: WorldEvent) {
    const g = this.generation
    for (const listener of Array.from(this.listeners)) {
      if (g !== this.generation) break
      try { listener(event) } catch { /* Consumer exceptions cannot retain socket authority. */ }
    }
  }
  private stateTo(state: WorldState) { this.status = state; this.emit({ type: 'state', state }) }
  private clearTimer(name: string) { if (this.timers.has(name)) { this.clock.clear(this.timers.get(name)); this.timers.delete(name) } }
  private timer(name: string, delay: number, callback: () => void) {
    this.clearTimer(name); const g = this.generation
    const handle = this.clock.set(delay, () => {
      if (this.timers.get(name) !== handle || !this.current(g)) return
      this.timers.delete(name); callback()
    })
    this.timers.set(name,handle)
  }
  private current(g: number) { return g === this.generation && !!this.controller && !this.controller.signal.aborted && !!this.scope && this.options.session.isCurrent(this.scope) }
  private stop(error: WorldTransportError | null = null, disposed = false, terminalFrame?: ServerMessage) {
    ++this.generation
    const controller = this.controller, socket = this.socket, completion = this.completion
    this.controller = null; this.socket = null; this.completion = null
    this.release.splice(0).forEach(fn => fn()); controller?.abort()
    for (const handle of this.timers.values()) this.clock.clear(handle)
    this.timers.clear(); this.scope = null; this.resources = null; this.baseline = null; this.motion = null
    this.roster.clear(); this.streams.clear(); this.requests.clear(); this.pending = null; this.heartbeat = null
    this.sequence = this.inputSequence = this.ack = this.tick = this.serverOffset = this.bufferedFrames = 0
    if (socket) { try { socket.close(error && error.code >= 3000 && error.code <= 4999 ? error.code : error ? 4400 : 1000, error ? 'world-transport-error' : 'world-transport-closed') } catch { /* socket already unavailable */ } }
    completion?.reject(error ?? new WorldTransportError(disposed ? 'disposed' : 'cancelled'))
    this.status = disposed ? 'disposed' : 'closed'
    const g = this.generation
    if (terminalFrame) this.emit({ type: 'frame',frame:terminalFrame })
    if (g !== this.generation) return
    if (error) this.emit({ type: 'error', kind: error.kind, code: error.code })
    if (g === this.generation) this.emit({ type: 'state', state: this.status })
    if (disposed) { this.listeners.clear(); this.usedTickets.clear() }
  }
  disconnect() { if (this.status !== 'disposed') this.stop() }
  dispose() { if (this.status !== 'disposed') this.stop(null, true) }
  connect(resources: WorldResources, signal?: AbortSignal): Promise<void> {
    if (this.status === 'disposed') return Promise.reject(new WorldTransportError('disposed'))
    if (!['idle', 'closed'].includes(this.status)) return Promise.reject(new WorldTransportError('invalid-context'))
    const session = this.options.session.read()
    if (!session.userId || session.eventId !== resources.eventId || session.signal.aborted || signal?.aborted) return Promise.reject(new WorldTransportError('invalid-context'))
    try { validate('WorldTicketRequest', body(resources)); validate('Authenticate', { ...resources, protocolVersion: policy.protocol_version, type: 'authenticate', ticket: 'A'.repeat(43),
      ...(this.motionProfileVersion ? { motionProfileVersion: this.motionProfileVersion } : {}) }) }
    catch { return Promise.reject(new WorldTransportError('invalid-context')) }
    const g = ++this.generation
    this.scope = { generation: session.generation, userId: session.userId, eventId: session.eventId }
    this.resources = { ...resources }; this.controller = new AbortController()
    this.rateTime = this.clock.now(); this.allTokens = policy.all_client_burst; this.inputTokens = policy.input_burst; this.lastResync = -Infinity
    const cancel = () => { if (this.generation === g) this.stop(new WorldTransportError('cancelled')) }
    for (const s of [session.signal, signal]) if (s) { s.addEventListener('abort', cancel, { once: true }); this.release.push(() => s.removeEventListener('abort', cancel)) }
    const result = new Promise<void>((resolve,reject) => { this.completion = { resolve, reject } })
    this.timer('handshake', policy.authenticate_deadline_ms, () => this.stop(new WorldTransportError('timeout', 4408)))
    this.stateTo('ticket')
    if (this.current(g)) void this.open(g, resources)
    return result
  }
  private async ticket(resources: WorldResources, g: number, acquisitionSignal?: AbortSignal) {
    ensure(this.scope && this.controller)
    const signal = acquisitionSignal ?? this.controller.signal, started = this.clock.now()
    let raw: unknown
    try { raw = await abortable(this.options.acquireTicket({ scope: { ...this.scope }, body: body(resources), signal }), signal) }
    catch (error) { throw new TicketAcquisitionFailure(error) }
    if (!this.current(g) || signal.aborted) throw new WorldTransportError('cancelled')
    const ticket = decodeTicket(raw)
    ensure(ticket.event_id === resources.eventId && Object.entries(body(resources)).every(([k,v]) => ticket[k as keyof typeof ticket] === v))
    ensure(ticket.ws_path === '/world-ws')
    const digest = Array.from(new Uint8Array(await abortable(crypto.subtle.digest('SHA-256',new TextEncoder().encode(ticket.ticket)),signal)), b => b.toString(16).padStart(2,'0')).join('')
    if (!this.current(g) || signal.aborted) throw new WorldTransportError('cancelled')
    ensure(!this.usedTickets.has(digest) && this.usedTickets.size < 1024)
    this.usedTickets.add(digest)
    const elapsed = this.clock.now() - started, ttl = Date.parse(ticket.expires_at) - Date.parse(ticket.server_time)
    ensure(elapsed >= 0 && elapsed < ttl)
    return { ticket, expires: this.clock.now() + ttl - elapsed, serverOffset: Date.parse(ticket.server_time) - started }
  }
  private async open(g: number, resources: WorldResources) {
    try {
      let acquired: Awaited<ReturnType<TransportEngine['ticket']>> | null = await this.ticket(resources,g)
      if (!this.current(g)) return
      this.serverOffset = acquired.serverOffset
      const socket = this.factory(this.url)
      if (!this.current(g)) { socket.close(1000,'world-transport-closed'); return }
      this.socket = socket
      const listen = (type: string, fn: (e: SocketEvent) => void) => {
        const guarded = (e: SocketEvent) => { if (this.current(g) && this.socket === socket) fn(e) }
        socket.addEventListener(type, guarded); this.release.push(() => socket.removeEventListener(type, guarded))
      }
      listen('open', () => {
        try {
          ensure(this.status === 'opening' && socket.protocol === '' && acquired && this.clock.now() < acquired.expires)
          const frame = { protocolVersion: policy.protocol_version, type: 'authenticate', ...resources, ticket: acquired.ticket.ticket,
            ...(this.motionProfileVersion ? { motionProfileVersion: this.motionProfileVersion } : {}) }
          acquired = null
          this.send(frame); this.stateTo('authenticating')
        } catch (error) { acquired = null; if (this.current(g)) this.stop(error instanceof WorldTransportError ? error : new WorldTransportError('invalid-frame')) }
      })
      listen('message', e => {
        try { this.receive(decode(e.data, 'server') as ServerMessage) }
        catch (error) { if (this.current(g)) this.stop(error instanceof WorldTransportError ? error : new WorldTransportError('invalid-frame')) }
      })
      listen('close', e => {
        const known = new Set<number>([...Object.values(policy.close_codes),1001,1005,1006,1011])
        this.stop(new WorldTransportError('closed', typeof e.code === 'number' && known.has(e.code) ? e.code : 4400))
      })
      listen('error', () => this.stop(new WorldTransportError('unavailable', 4500)))
      this.release.push(() => { acquired = null })
      this.timer('handshake', policy.authenticate_deadline_ms, () => this.stop(new WorldTransportError('timeout', 4408)))
      this.stateTo('opening')
    } catch (error) { if (this.current(g)) this.stop(error instanceof WorldTransportError ? error : new WorldTransportError('unavailable', 4500)) }
  }
  private send(frame: unknown) {
    ensure(this.controller && this.scope && this.options.session.isCurrent(this.scope) && this.socket?.readyState === 1)
    const data = JSON.stringify(frame)
    decode(data, 'client')
    const bytes = new TextEncoder().encode(data).length
    if (this.socket.bufferedAmount === 0) this.bufferedFrames = 0
    if (this.socket.bufferedAmount + bytes > policy.outgoing_queue_bytes || this.bufferedFrames >= policy.outgoing_queue_frames) {
      // No application replay queue. Conservatively count frames until the native buffer drains.
      const error = new WorldTransportError('backpressure', 4409); this.stop(error); throw error
    }
    const now = this.clock.now(), dt = Math.max(0, now - this.rateTime) / 1000
    this.allTokens = Math.min(policy.all_client_burst, this.allTokens + dt * policy.all_client_messages_per_second)
    this.inputTokens = Math.min(policy.input_burst, this.inputTokens + dt * policy.input_messages_per_second); this.rateTime = now
    const input = (frame as { type: string }).type === 'input'
    if (this.allTokens < 1 || (input && this.inputTokens < 1)) throw new WorldTransportError('rate-limited', 4429)
    this.allTokens--; if (input) this.inputTokens--
    try { this.socket.send(data); this.bufferedFrames++ } catch { const e = new WorldTransportError('unavailable',4500); this.stop(e); throw e }
  }
  private live() { ensure(this.baseline && this.scope && this.options.session.isCurrent(this.scope) && ['live','switching'].includes(this.status)); return this.baseline }
  sendInput(input: Pick<Input, 'seq' | 'axisX' | 'axisY' | 'buttons'>) {
    const b = this.live(); ensure(input.seq > this.inputSequence)
    this.send({ ...input, protocolVersion: policy.protocol_version, type: 'input', streamId: b.streamId, roomEpoch: b.roomEpoch }); this.inputSequence = input.seq
  }
  private requestId(id: string) { ensure(!this.requests.has(id) && this.requests.size < 1024); this.requests.add(id) }
  resync(requestId: string, reason: Resync['reason']) {
    const b = this.baseline; ensure(b && ['live','resync'].includes(this.status) && !this.pending)
    ensure(this.clock.now() - this.lastResync >= policy.resync_min_interval_ms)
    const frame = { protocolVersion: policy.protocol_version, type: 'resync', requestId, reason, streamId: b.streamId, roomEpoch: b.roomEpoch, lastStreamSeq: this.sequence }
    validate('Resync',frame); this.requestId(requestId); this.send(frame)
    this.pending = { sent: true, type: 'resync', requestId, roomId: b.roomId }; this.lastResync = this.clock.now()
    this.timer('control', policy.authenticate_deadline_ms, () => this.stop(new WorldTransportError('timeout',4408)))
    this.stateTo('resync')
  }
  async roomSwitch(roomId: string, requestId: string) {
    const b = this.live(); ensure(!this.pending && roomId !== b.roomId && this.resources)
    const target = { ...this.resources, roomId }, g = this.generation
    validate('WorldTicketRequest',body(target)); validate('SwitchRoom', { ...target, type: 'switchRoom', protocolVersion: policy.protocol_version, streamId: b.streamId, requestId, ticket: 'A'.repeat(43) })
    this.requestId(requestId); this.pending = { sent: false, type: 'switchRoom', requestId, roomId }
    const acquisition = new AbortController(), connectionSignal = this.controller!.signal
    const cancelAcquisition = () => acquisition.abort()
    connectionSignal.addEventListener('abort', cancelAcquisition, { once: true })
    let issuanceTimedOut = false
    this.timer('control', policy.authenticate_deadline_ms, () => { issuanceTimedOut = true; acquisition.abort() })
    this.stateTo('switching')
    try {
      if (!this.current(g)) throw new WorldTransportError('cancelled')
      const acquired = await this.ticket(target,g,acquisition.signal)
      if (!this.current(g)) throw new WorldTransportError('cancelled')
      ensure(this.baseline?.streamId === b.streamId && this.pending?.requestId === requestId)
      const frame: SwitchRoom = { protocolVersion: '1.0.0', type: 'switchRoom', streamId: b.streamId, requestId, ticket: acquired.ticket.ticket, ...target }
      this.pending.sent = true
      this.timer('control', policy.authenticate_deadline_ms, () => this.stop(new WorldTransportError('timeout',4408)))
      this.send(frame)
    } catch (error) {
      const safe = issuanceTimedOut ? new WorldTransportError('timeout',4408)
        : error instanceof WorldTransportError ? error : new WorldTransportError('unavailable',4500)
      if (this.current(g)) {
        const ordinaryIssuanceFailure = issuanceTimedOut || (error instanceof TicketAcquisitionFailure && error.preserveOldRoom)
        const oldRoomValid = this.status === 'switching' && this.pending?.requestId === requestId && !this.pending.sent
          && this.baseline?.streamId === b.streamId && this.baseline.roomEpoch === b.roomEpoch && this.socket?.readyState === 1
          && Date.parse(b.accessExpiresAt) > this.clock.now() + this.serverOffset
        if (ordinaryIssuanceFailure && oldRoomValid) {
          this.pending = null; this.clearTimer('control'); this.stateTo('live')
        } else this.stop(safe)
      }
      throw safe
    } finally {
      connectionSignal.removeEventListener('abort', cancelAcquisition); acquisition.abort()
    }
  }
  clientPong(heartbeatId: string) {
    ensure(this.baseline && this.heartbeat === heartbeatId)
    this.send({ protocolVersion: policy.protocol_version, type: 'pong', heartbeatId }); this.heartbeat = null; this.clearTimer('pong')
  }
  private recover(reason: 'GAP' | 'EPOCH_CHANGED') {
    ensure(!this.pending || this.pending.type === 'resync')
    if (this.status === 'resync') return
    const g = this.generation
    this.timer('control', policy.authenticate_deadline_ms, () => this.stop(new WorldTransportError('timeout',4408)))
    this.stateTo('resync')
    if (this.current(g)) this.emit({ type: 'resync-needed',reason })
  }
  private receive(frame: ServerMessage) {
    const g = this.generation
    if (frame.type === 'snapshot') this.snapshot(frame)
    else {
      const b = this.baseline; ensure(b)
      if (frame.type === 'delta') {
        // Validate the wire first, then discard queued old deltas without touching any baseline counter.
        if (this.status === 'resync') return
        if (frame.streamId !== b.streamId || frame.roomEpoch !== b.roomEpoch) { this.recover('EPOCH_CHANGED'); return }
        if (frame.streamSeq !== this.sequence + 1) { this.recover('GAP'); return }
        ensure(frame.serverTick >= this.tick && frame.ackInputSeq >= this.ack && frame.ackInputSeq <= this.inputSequence)
        this.motionDelta(frame, b.selfUserId)
        const roster = new Set(this.roster)
        for (const id of frame.removes) { ensure(roster.has(id) && id !== this.scope?.userId); roster.delete(id) }
        for (const row of frame.upserts) { this.avatar(row.avatar.config, b); roster.add(row.userId) }
        ensure(roster.size <= policy.room_capacity_max)
        this.roster = roster; this.sequence = frame.streamSeq; this.tick = frame.serverTick; this.ack = frame.ackInputSeq
      } else {
        if ('streamId' in frame) ensure(frame.streamId === b.streamId)
        if (frame.type === 'policy.baseline' || frame.type === 'policy.invalidate') {
          // Metadata borrows this authenticated stream; it never grants World or HTTP authority.
          ensure(frame.eventId === b.eventId)
        } else if (frame.type === 'switchRejected') {
          ensure(this.pending?.sent && this.pending.type === 'switchRoom' && this.pending.requestId === frame.requestId)
          this.pending = null; this.clearTimer('control'); this.stateTo('live')
        } else if (frame.type === 'resyncRequired') {
          if (['MAP_VERSION_MISMATCH','BACKPRESSURE'].includes(frame.reason)) { this.stop(new WorldTransportError('closed',4409),false,frame); return }
          this.recover(frame.reason as 'GAP' | 'EPOCH_CHANGED')
        } else if (frame.type === 'revoke') {
          this.stop(new WorldTransportError('closed',4401),false,frame); return
        } else if (frame.type === 'ping') {
          ensure(!this.heartbeat); this.heartbeat = frame.heartbeatId
          this.timer('pong',policy.heartbeat_timeout_ms, () => this.stop(new WorldTransportError('timeout',4408)))
          this.timer('heartbeat',policy.heartbeat_interval_ms + policy.heartbeat_timeout_ms, () => this.stop(new WorldTransportError('timeout',4408)))
        }
      }
    }
    if (this.current(g)) this.emit({ type: 'frame',frame })
  }
  private avatar(config: { manifest_id: string; manifest_version: number }, context: WorldResources) {
    ensure(config.manifest_id === context.avatarManifestId && config.manifest_version === context.avatarManifestVersion)
  }
  private motionDelta(frame: Delta, selfUserId: string) {
    if (!this.motion) { ensure(!frame.selfMotion); return }
    const next = frame.selfMotion, previous = this.motion.anchor
    ensure(next && next.consumedInputSeq >= previous.consumedInputSeq && next.consumedInputSeq <= this.inputSequence
      && next.droppedTicks >= previous.droppedTicks)
    const actualSteps = frame.serverTick - this.tick - (next.droppedTicks - previous.droppedTicks)
    ensure(actualSteps >= 0)
    if (frame.ackInputSeq === this.motion.ack) {
      ensure(next.inputAppliedTicks >= previous.inputAppliedTicks && next.inputAppliedTicks - previous.inputAppliedTicks <= actualSteps)
      if (next.heldInput) ensure(previous.heldInput && next.heldInput.seq === previous.heldInput.seq
        && next.heldInput.axisX === previous.heldInput.axisX && next.heldInput.axisY === previous.heldInput.axisY
        && next.heldInput.remainingHoldMs <= previous.heldInput.remainingHoldMs)
    } else ensure(next.inputAppliedTicks <= actualSteps)
    if (next.heldInput) ensure(next.heldInput.remainingHoldMs <= this.motion.profile.holdMs)
    const self = frame.upserts.find(player => player.userId === selfUserId)
    if (self) ensure(self.position.x === next.position.x && self.position.y === next.position.y)
    this.motion = { ...this.motion, anchor: structuredClone(next), ack: frame.ackInputSeq }
  }
  private snapshot(frame: Snapshot) {
    ensure(this.scope && this.resources && ['authenticating','live','resync','switching'].includes(this.status))
    ensure(frame.selfUserId === this.scope.userId && frame.ackInputSeq === 0 && !this.streams.has(frame.streamId) && this.streams.size < 1024)
    const old = this.baseline
    let expected = this.resources
    if (!old) ensure(frame.cause === 'INITIAL')
    else {
      ensure(Date.parse(frame.accessExpiresAt) === Date.parse(old.accessExpiresAt))
      expected = old
      if (frame.cause === 'RESYNC' || frame.cause === 'ROOM_SWITCH') {
        ensure(this.pending?.sent && this.pending.type === (frame.cause === 'RESYNC' ? 'resync' : 'switchRoom') && this.pending.requestId === frame.requestId)
        expected = { ...old, roomId: this.pending.roomId }
      } else ensure(frame.cause === 'EPOCH_CHANGED' && !this.pending && frame.roomEpoch !== old.roomEpoch)
      if (frame.roomId === old.roomId && frame.roomEpoch === old.roomEpoch) ensure(frame.serverTick >= this.tick)
    }
    ensure(keys.every(k => frame[k] === expected[k]))
    let motion: typeof this.motion = null
    if (frame.motionProfile || frame.selfMotion) {
      ensure(this.motionProfileVersion === '1.0.0' && frame.motionProfile && frame.selfMotion)
      if (old && this.motion && frame.roomId === old.roomId && frame.roomEpoch === old.roomEpoch) {
        ensure(frame.selfMotion.droppedTicks >= this.motion.anchor.droppedTicks
          && frame.selfMotion.droppedTicks - this.motion.anchor.droppedTicks <= frame.serverTick - this.tick)
      }
      motion = { profile: structuredClone(frame.motionProfile), anchor: structuredClone(frame.selfMotion), ack: 0 }
    }
    for (const row of frame.players) this.avatar(row.avatar.config,frame)
    const remaining = Date.parse(frame.accessExpiresAt) - (this.clock.now() + this.serverOffset)
    ensure(remaining > 0)
    const { players, ...metadata } = frame
    this.baseline = metadata; this.motion = motion; this.resources = Object.fromEntries(keys.map(k => [k,frame[k]])) as unknown as WorldResources
    this.roster = new Set(players.map(p => p.userId)); this.streams.add(frame.streamId)
    this.sequence = this.inputSequence = this.ack = 0; this.tick = frame.serverTick; this.pending = null
    this.clearTimer('handshake'); this.clearTimer('control')
    const accessExpiresAt = frame.accessExpiresAt
    const expire = () => {
      const ms = Date.parse(accessExpiresAt) - (this.clock.now() + this.serverOffset)
      if (ms <= 0) this.stop(new WorldTransportError('timeout',4401)); else this.timer('access', Math.min(ms,2_147_483_647), expire)
    }
    expire()
    if (!old) this.timer('heartbeat',policy.heartbeat_interval_ms + policy.heartbeat_timeout_ms, () => this.stop(new WorldTransportError('timeout',4408)))
    const complete = this.completion; this.completion = null; this.stateTo('live'); complete?.resolve()
  }
}
