import type { Scheduler, WorldTransportOptions } from './WorldTransport'
import { TransportEngine, type SocketPort, type SocketEvent } from './transport/engine'
/** Explicit development/test import only. No synthetic product roster or automatic success. */
export class MockSocket implements SocketPort {
  readyState = 0
  bufferedAmount = 0
  protocol = ''
  readonly sent: string[] = []
  readonly listeners = new Map<string, Set<(e: SocketEvent) => void>>()
  closeCount = 0
  addEventListener(type: string, listener: (e: SocketEvent) => void) {
    if (!this.listeners.has(type)) this.listeners.set(type,new Set())
    this.listeners.get(type)!.add(listener)
  }
  removeEventListener(type: string, listener: (e: SocketEvent) => void) { this.listeners.get(type)?.delete(listener) }
  emit(type: string, data?: unknown, code?: number) { for (const fn of Array.from(this.listeners.get(type) ?? [])) fn({ data, ...(code === undefined ? {} : { code }) }) }
  open() { this.readyState = 1; this.emit('open') }
  receive(data: unknown) { this.emit('message',data) }
  send(data: string) { if (this.readyState !== 1) throw Error('Mock socket closed'); this.sent.push(data) }
  close() { this.readyState = 3; this.closeCount++ }
  fail() { this.emit('error') }
  serverClose(code = 1000) { this.readyState = 3; this.emit('close',undefined,code) }
}
/** Controllable deterministic scheduler. Callbacks scheduled for the same instant keep insertion order. */
export class MockWorldClock implements Scheduler {
  private time: number
  private next = 0
  private jobs = new Map<number, { at: number; callback(): void }>()
  constructor(now = 0) { this.time = now }
  now() { return this.time }
  get pending() { return this.jobs.size }
  set(delay: number, callback: () => void) { const id = ++this.next; this.jobs.set(id,{ at: this.time + delay, callback }); return id }
  clear(handle: unknown) { this.jobs.delete(handle as number) }
  advance(ms: number) {
    if (ms < 0 || !Number.isFinite(ms)) throw Error('Invalid clock advance')
    const target = this.time + ms
    for (;;) {
      const next = [...this.jobs].sort((a,b) => a[1].at - b[1].at || a[0] - b[0]).find(([,job]) => job.at <= target)
      if (!next) break
      this.jobs.delete(next[0]); this.time = next[1].at; next[1].callback()
    }
    this.time = target
  }
}
export class MockWorldTransport extends TransportEngine {
  readonly sockets: MockSocket[]
  constructor(options: WorldTransportOptions) {
    const sockets: MockSocket[] = []
    super(options, () => { const socket = new MockSocket(); sockets.push(socket); return socket })
    this.sockets = sockets
  }
}
