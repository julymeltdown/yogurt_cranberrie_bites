import type { Page } from '@playwright/test'
import type { Snapshot, WorldPlayer, ServerMessage, ClientMessage, Input } from '../../../src/world/network/WorldTransport'

export type Decoder = (payload: unknown, direction: 'client' | 'server') => ClientMessage | ServerMessage

/** Passive C05 observations only. Never route, send frames, patch WebSocket, or retain tickets. */
export class WorldObserver {
  constructor(private readonly decode: Decoder) {}
  active = 0; peak = 0; opens = 0; snapshots = 0; invalidFrames = 0; nonzeroInputs = 0
  baseline: Snapshot | null = null
  players = new Map<string, WorldPlayer>()
  streamSeq = -1
  lastInput: Pick<Input, 'seq' | 'axisX' | 'axisY'> | null = null
  private currentSocket: object | null = null
  attach(page: Page, origin: string) {
    const expected = new URL('/world-ws', origin); expected.protocol = expected.protocol === 'https:' ? 'wss:' : 'ws:'
    page.on('websocket', socket => {
      const url = new URL(socket.url())
      if (url.origin !== expected.origin || url.pathname !== expected.pathname) return
      const identity = {}; this.currentSocket = identity; this.active++; this.opens++; this.peak = Math.max(this.peak, this.active)
      socket.on('framereceived', ({ payload }) => { if (this.currentSocket === identity) this.receive(payload) })
      socket.on('framesent', ({ payload }) => {
        if (this.currentSocket !== identity) return
        // Decode authentication transiently, never keep or report its ticket/body.
        try { const message = this.decode(payload, 'client'); if (message.type === 'input') { this.lastInput = { seq: message.seq, axisX: message.axisX, axisY: message.axisY }; if (message.axisX || message.axisY) this.nonzeroInputs++ } } catch { this.invalidFrames++ }
      })
      socket.on('close', () => { this.active--; if (this.currentSocket === identity) { this.currentSocket = null; this.baseline = null; this.players.clear(); this.streamSeq = -1 } })
    })
  }
  receive(payload: string | Buffer) {
    let message: ServerMessage
    try { message = this.decode(payload, 'server') as ServerMessage } catch { this.invalidFrames++; return }
    if (message.type === 'snapshot') {
      this.baseline = message; this.streamSeq = 0; this.snapshots++; this.players = new Map(message.players.map(p => [p.userId, p])); return
    }
    if (message.type === 'revoke' || message.type === 'resyncRequired') { this.players.clear(); this.baseline = null; this.streamSeq = -1; return }
    if (message.type !== 'delta') return
    if (!this.baseline || message.streamId !== this.baseline.streamId || message.roomEpoch !== this.baseline.roomEpoch || message.streamSeq !== this.streamSeq + 1) {
      this.players.clear(); this.baseline = null; this.streamSeq = -1; this.invalidFrames++; return
    }
    this.streamSeq = message.streamSeq
    message.removes.forEach(id => this.players.delete(id)); message.upserts.forEach(p => this.players.set(p.userId, p))
  }
  clear() { this.players.clear(); this.baseline = null; this.lastInput = null }
}
