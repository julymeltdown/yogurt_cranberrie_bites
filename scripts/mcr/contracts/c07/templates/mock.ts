// Generated deterministic TEST transport. No live-service or authorization claim.
export class FixtureClock {
  private epoch: number
  private state: number
  constructor(now = '2030-01-01T00:00:00.000Z', seed = 424242) {
    this.epoch = Date.parse(now)
    if (!Number.isSafeInteger(this.epoch) || !Number.isInteger(seed) || seed <= 0 || seed > 0xffffffff) throw new Error('C07 invalid fixture clock/seed')
    this.state = seed >>> 0
  }
  now(): string { return new Date(this.epoch).toISOString() }
  advance(milliseconds: number): void {
    if (!Number.isSafeInteger(milliseconds) || milliseconds < 0 || !Number.isSafeInteger(this.epoch + milliseconds) || !Number.isFinite(new Date(this.epoch + milliseconds).valueOf())) throw new Error('C07 invalid clock advance')
    this.epoch += milliseconds
  }
  private next(): number { let x = this.state; x ^= x << 13; x ^= x >>> 17; x ^= x << 5; this.state = x >>> 0; return this.state }
  id(): string {
    const bytes = Array.from({ length: 16 }, () => this.next() & 255)
    bytes[6] = (bytes[6]! & 15) | 64; bytes[8] = (bytes[8]! & 63) | 128
    const hex = bytes.map(x => x.toString(16).padStart(2, '0')).join('')
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
  }
}
export interface MockStep {
  method: string
  path: string
  status?: number
  mediaType?: string | null
  body?: unknown
  headers?: Record<string, string>
  failure?: 'network'
  advanceMs?: number
}
export function createMock(steps: readonly MockStep[], clock = new FixtureClock()) {
  let cursor = 0
  const requests: { method: string; path: string; at: string; id: string }[] = []
  const fetch: typeof globalThis.fetch = async (input, init) => {
    if (init?.signal?.aborted) throw init.signal.reason
    const url = new URL(input instanceof Request ? input.url : String(input))
    const step = steps[cursor++]
    if (!step || step.method !== (init?.method ?? 'GET') || step.path !== url.pathname + url.search) throw new Error('C07 unexpected mock request')
    requests.push({ method: step.method, path: step.path, at: clock.now(), id: clock.id() })
    clock.advance(step.advanceMs ?? 0)
    if (step.failure === 'network') throw new Error('C07 deterministic network failure')
    const headers = new Headers(step.headers)
    if (step.mediaType) headers.set('Content-Type', step.mediaType)
    const body = step.mediaType === 'application/json' ? JSON.stringify(step.body) : step.body as BodyInit | undefined
    return new Response(body ?? null, { status: step.status ?? 200, headers })
  }
  return { fetch, requests, clock, assertComplete() { if (cursor !== steps.length) throw new Error('C07 unconsumed mock scenario') } }
}
