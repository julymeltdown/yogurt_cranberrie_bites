import { createHmac, randomBytes } from 'node:crypto'

const uuid = '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}'
const routes = [
  ['POST', /^\/api\/auth\/login$/, 'login'],
  ['POST', /^\/api\/auth\/refresh$/, 'refresh'],
  ['DELETE', /^\/api\/auth\/account$/, 'deletionRequest'],
  ['GET', /^\/api\/auth\/account\/deletion$/, 'deletionRead'],
  ['GET', new RegExp(`^/api/concert/events/${uuid}/reservations/${uuid}$`), 'reservationRead'],
  ['POST', new RegExp(`^/api/concert/events/${uuid}/reservations/${uuid}/claim$`), 'pickupClaim'],
  ['GET', new RegExp(`^/api/concert/events/${uuid}/commands$`), 'commandStatus'],
  ['GET', new RegExp(`^/api/concert/events/${uuid}/admin/closure/${uuid}$`), 'closureRead'],
  ['POST', new RegExp(`^/api/concert/events/${uuid}/admin/closure$`), 'closureCommand'],
  ['GET', new RegExp(`^/api/concert/events/${uuid}/me/notifications(?:/unread-count)?$`), 'inboxRead'],
]

/** A closed route classification. No URL, UUID, query, body, header or response text is retained. */
export function classifyRequest(url, method, origin) {
  try {
    const parsed = new URL(url)
    if (parsed.origin !== origin) return null
    return routes.find(([verb, pattern]) => verb === method && pattern.test(parsed.pathname))?.[2] ?? null
  } catch { return null }
}

/** Attach before login. Observes actual browser traffic; never intercepts or mutates it.
 * HMAC salt stays private to this observer lifetime; equal command identities can be
 * compared within its report without publishing the actual key or reusable hashes.
 */
export function observeClient(page, { origin, maxEvents = 2000 }) {
  const parsed = new URL(origin)
  if (parsed.origin !== origin || !Number.isInteger(maxEvents) || maxEvents < 1 || maxEvents > 5000) throw new Error('QA09_OBSERVER_CONFIG')
  const salt = randomBytes(32), events = [], attached = new Map(), requests = new WeakMap()
  let sequence = 0, active = 0, peak = 0, overflow = false, detached = false
  function append(event) {
    if (detached) return
    if (events.length === maxEvents) { overflow = true; return }
    events.push(Object.freeze({ sequence: ++sequence, at: new Date().toISOString(), ...event }))
  }
  const request = req => {
    const operation = classifyRequest(req.url(), req.method(), origin)
    if (!operation) return
    // Read only the single public command-identity header, never allHeaders/body/token.
    const query = operation === 'commandStatus' ? new URL(req.url()).searchParams : null
    const key = query ? query.get('idempotency_key') : req.headers()['idempotency-key']
    const identity = typeof key === 'string' && new RegExp(`^${uuid}$`).test(key)
      ? createHmac('sha256', salt).update(key).digest('hex') : null
    const subjects = { claimConcertReservation: 'pickupClaim', closeConcertEvent: 'closureCommand' }
    const subject = query?.get('operation_id')
    const metadata = Object.freeze({ operation, method: req.method(), commandIdentity: identity,
      subjectOperation: subject && Object.hasOwn(subjects, subject) ? subjects[subject] : null })
    requests.set(req, metadata)
    append({ kind: 'request', ...metadata })
  }
  const response = res => {
    const metadata = requests.get(res.request())
    if (metadata) append({ kind: 'response', ...metadata, status: res.status() })
  }
  const failed = req => {
    const metadata = requests.get(req)
    if (metadata) append({ kind: 'requestFailed', ...metadata })
  }
  const socket = ws => {
    let parsed
    try { parsed = new URL(ws.url()) } catch { return }
    const expected = new URL(origin); expected.protocol = expected.protocol === 'https:' ? 'wss:' : 'ws:'
    if (parsed.origin !== expected.origin || parsed.pathname !== '/world-ws') return
    active++; peak = Math.max(peak, active)
    append({ kind: 'socketOpen', active })
    const closed = () => { active--; append({ kind: 'socketClose', active }); attached.delete(ws) }
    ws.on('close', closed); attached.set(ws, closed)
    // Payloads are intentionally not read: the existing C05 owner qualifies wire convergence.
  }
  const pageError = () => append({ kind: 'pageError' })
  page.on('request', request); page.on('response', response); page.on('requestfailed', failed)
  page.on('websocket', socket); page.on('pageerror', pageError)
  return {
    snapshot() { return { events: [...events], activeSockets: active, peakSockets: peak, overflow, detached } },
    detach() {
      if (detached) return
      detached = true
      page.off('request', request); page.off('response', response); page.off('requestfailed', failed)
      page.off('websocket', socket); page.off('pageerror', pageError)
      for (const [ws, closed] of attached) ws.off('close', closed)
      attached.clear(); salt.fill(0)
    },
  }
}
