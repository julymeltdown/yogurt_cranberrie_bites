import type { SessionPort, SessionScope } from '../../shared/api/http'
import type { Input, Resync, ServerMessage, WorldTicketRequest, WorldTicketResponse } from './transport/wire-types'
export type * from './transport/wire-types'
export { WorldTransportError } from './transport/contract'
export interface WorldResources {
  readonly eventId: string
  readonly roomId: string
  readonly mapId: string
  readonly mapRevision: number
  readonly avatarManifestId: string
  readonly avatarManifestVersion: number
}
/** Adapter uses C05 issueWorldTicket POST, NO_STORE, NO_AUTOMATIC_REPLAY and canonical FE01 session. */
export type AcquireWorldTicket = (request: { readonly scope: SessionScope; readonly body: WorldTicketRequest; readonly signal: AbortSignal }) => Promise<WorldTicketResponse>
export type WorldState = 'idle' | 'ticket' | 'opening' | 'authenticating' | 'live' | 'resync' | 'switching' | 'closed' | 'disposed'
export type WorldEvent = { readonly type: 'state'; readonly state: WorldState } | { readonly type: 'frame'; readonly frame: ServerMessage }
  | { readonly type: 'resync-needed'; readonly reason: 'GAP' | 'EPOCH_CHANGED' }
  | { readonly type: 'error'; readonly kind: string; readonly code: number }
export interface WorldTransport {
  readonly state: WorldState
  subscribe(listener: (event: WorldEvent) => void): () => void
  connect(resources: WorldResources, signal?: AbortSignal): Promise<void>
  sendInput(input: Pick<Input, 'seq' | 'axisX' | 'axisY' | 'buttons'>): void
  resync(requestId: string, reason: Resync['reason']): void
  roomSwitch(roomId: string, requestId: string): Promise<void>
  clientPong(heartbeatId: string): void
  disconnect(): void
  dispose(): void
}
export interface Scheduler {
  now(): number
  set(delayMs: number, callback: () => void): unknown
  clear(handle: unknown): void
}
export interface WorldTransportOptions {
  /** Explicit compatible C05 request. Omission preserves the legacy Authenticate byte shape. */
  readonly motionProfileVersion?: '1.0.0'
  readonly session: SessionPort
  readonly acquireTicket: AcquireWorldTicket
  /** Exact browser origin, with no path/query/credentials. WSS for HTTPS; WS only loopback HTTP. */
  readonly origin: string
  readonly scheduler?: Scheduler
}
