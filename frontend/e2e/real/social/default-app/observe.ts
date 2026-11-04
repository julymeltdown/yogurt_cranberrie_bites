import type { Page, Request, Response, WebSocket } from '@playwright/test'
import { createHash } from 'node:crypto'
import { createAudit, policyMetadata, publicPath } from './audit.mjs'

type Source = { revision: string; mainSha256: string }
export function observeDefaultApp(page: Page, expected: { origin: string; eventId: string; userId: string; worldURL: string; source: Source }) {
  const audit = createAudit(), pending = new Set<Promise<void>>(), sockets = new Set<WebSocket>()
  const endpoint = new URL(expected.worldURL)
  let liveWorld = 0, peakWorld = 0, worldOpened = 0, otherOpened = 0, matchedBaselines = 0, invalidations = 0
  let mainLoads = 0, loginRequests = 0, bootstrapBound = 0, oldEntryRequests = 0, inspectionsFailed = false
  let policyReady = false, protectedReadsBeforeBaseline = 0
  let consentInvalidations = 0, blockInvalidations = 0
  const violations: string[] = [], requests = new WeakMap<Request, number>()
  let feedIds: string[] = []
  const at = () => Date.now()
  const request = (value: Request) => {
    const path = new URL(value.url()).pathname
    requests.set(value, at())
    if (path.startsWith('/e2e/real/social/')) oldEntryRequests++
    if (path === '/api/auth/login' || path === '/auth/login') loginRequests++
    if (value.method() === 'GET' && path.startsWith(`/api/concert/events/${expected.eventId}/`)
      && /\/(?:feed|posts|intros|social-requests|me\/(?:social-handles|social-requests|notifications))(?:\/|$)/.test(path)
      && !policyReady) protectedReadsBeforeBaseline++
  }
  async function inspect(value: Response) {
    const url = new URL(value.url()), path = url.pathname, original = value.request()
    if (url.origin !== expected.origin) { if (path.startsWith('/api/')) violations.push('foreign-http-origin'); return }
    if (path.startsWith('/api/')) audit.add({ kind: 'http', method: original.method(), path: publicPath(value.url()), status: value.status(), requestedAt: requests.get(original) ?? null, receivedAt: at() })
    if (path === `/api/concert/events/${expected.eventId}/feed` && value.status() === 200) {
      const body = await value.json() as { items?: { post_id: string; event_id: string; state: string }[] }
      if (!Array.isArray(body.items) || !body.items.every(row => row.event_id === expected.eventId && row.state === 'PUBLISHED' && typeof row.post_id === 'string')) {
        violations.push('feed-context-mismatch'); return
      }
      const next = body.items.map(row => row.post_id)
      feedIds = url.searchParams.has('cursor') ? [...feedIds, ...next] : next
    }
    if (path === '/src/main.tsx') {
      mainLoads++
      const headers = await value.allHeaders()
      if (value.status() !== 200 || headers['x-mcr-source-revision'] !== expected.source.revision
        || headers['x-mcr-source-sha256'] !== expected.source.mainSha256) violations.push('entry-source-mismatch')
      // Public transformed module bytes, not an authenticated body or C03 hash.
      const bytes = await value.body()
      audit.add({ kind: 'main-served', sha256: createHash('sha256').update(bytes).digest('hex'), sourceSha256: expected.source.mainSha256, receivedAt: at() })
    }
    if (path === `/api/concert/events/${expected.eventId}/bootstrap` && value.status() === 200) {
      const body = await value.json() as { participant?: { user_id?: string; event_id?: string }; event?: { event_id?: string } }
      const matches = body.participant?.user_id === expected.userId && body.participant.event_id === expected.eventId && body.event?.event_id === expected.eventId
      if (matches) bootstrapBound++; else violations.push('bootstrap-identity-mismatch')
      audit.add({ kind: 'bootstrap-binding', matches, receivedAt: at() })
    }
  }
  const response = (value: Response) => {
    const job = inspect(value).catch(() => { inspectionsFailed = true }).finally(() => { pending.delete(job) })
    pending.add(job)
  }
  const socket = (value: WebSocket) => {
    const url = new URL(value.url()), world = url.origin === endpoint.origin && url.pathname === endpoint.pathname
    let stream: string | null = null
    if (world) { sockets.add(value); liveWorld++; worldOpened++; peakWorld = Math.max(peakWorld, liveWorld) }
    else otherOpened++ // Includes development tooling; never counts as a World policy stream.
    if (!world) return
    audit.add({ kind: 'world-open', receivedAt: at() })
    value.on('framereceived', ({ payload }) => {
      const safe = policyMetadata(typeof payload === 'string' ? payload : '', expected.eventId, stream, expected.userId)
      if (!safe) return
      if (safe.type === 'snapshot') { stream = safe.streamId; policyReady = false }
      else if (safe.matches && safe.type === 'policy.baseline') { matchedBaselines++; policyReady = true }
      else if (safe.matches && safe.type === 'policy.invalidate') {
        invalidations++; policyReady = false
        if ('reason' in safe && safe.reason === 'CONSENT_CHANGED') consentInvalidations++
        if ('reason' in safe && safe.reason === 'BLOCK_CHANGED') blockInvalidations++
      }
      else policyReady = false
      // The stream tuple is compared in RAM. Its identifier and all frame bodies are omitted.
      audit.add({ kind: safe.type, matches: safe.matches, ...('revision' in safe ? { revision: safe.revision } : {}), receivedAt: at() })
    })
    value.on('close', () => { if (sockets.delete(value)) liveWorld--; policyReady = false; audit.add({ kind: 'world-close', receivedAt: at() }) })
  }
  const pageError = () => { violations.push('application-page-error') }
  page.on('request', request); page.on('response', response); page.on('websocket', socket); page.on('pageerror', pageError)
  return {
    feedIds: () => [...feedIds], // Private in-memory UI row lookup, never included in the public audit.
    read() { return { ...audit.read(), mainLoads, loginRequests, bootstrapBound, oldEntryRequests,
      liveWorld, peakWorld, worldOpened, otherOpened, matchedBaselines, invalidations, consentInvalidations, blockInvalidations,
      policyReady, protectedReadsBeforeBaseline, inspectionsFailed, violations: [...violations] } },
    async drain() { await Promise.allSettled(pending) },
    dispose() { page.off('request', request); page.off('response', response); page.off('websocket', socket); page.off('pageerror', pageError) },
  }
}
