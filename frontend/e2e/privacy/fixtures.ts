import { QueryClient, QueryObserver, MutationObserver, dehydrate } from '@tanstack/react-query'
import { createElement } from 'react'
import { SharingRequest } from '../../src/features/social-sharing/ui/SocialSharing'
import { mountPrivateView, mountedUI } from './mounted-ui'
import { createMemorySession } from '../../src/shared/api/http'
import { createQueryPrivacy, publicOnlyDehydrateOptions, type PolicyChange } from '../../src/shared/query/privacy'
import { publicQueryKey } from '../../src/shared/query/keys'
import { createFixtureSessionController } from './auth-fixture'
import { createSocialSharingController } from '../../src/features/social-sharing/model/controller'
import { publicIntroSchema } from '../../src/entities/intro/model/contract'
import { beaconSchema } from '../../src/features/meet-beacon/model/contract'
import { parse as feedValue } from '../../src/features/event-feed/contract'
import { decodeParty } from '../../src/features/party-discovery/model/contract'
import { giveawayDecoder } from '../../src/features/gift-discovery/model/contract'
import { reportSchema } from '../../src/features/safety/contract'
import approved from '../../../contracts/fixtures/domain/approved-social-request.json'
import intro from '../../../contracts/fixtures/domain/public-intro.json'
import post from '../../../contracts/fixtures/domain/public-post.json'
import party from '../../../contracts/fixtures/domain/party-full.json'
import giveaway from '../../../contracts/fixtures/domain/giveaway-management.json'
import beacon from '../../../contracts/fixtures/domain/private-beacon.json'
import { roles, operations, markers, check, assertAbsent, deferred, until, type Cell, type Role } from './oracle'
import { evidenceCells } from './evidence-fixtures'
import { recipientCells } from './recipient-fixtures'
import { beaconCells } from './beacon-fixtures'

const eventId = approved.event_id, owner = approved.requester_id, member = approved.recipient_id
const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const epoch = Date.parse('2026-11-07T09:00:00Z')
const participant = (role: Role) => role === 'owner' || role === 'member'
const actor = (role: Role) => role === 'owner' ? owner : role === 'member' ? member : id(700 + roles.indexOf(role))
const noStore = (body: unknown, status = 200) => Response.json(body, { status, headers: { 'Cache-Control': 'no-store' } })
const denied = () => noStore({ code: 'NOT_FOUND', message: 'Resource unavailable.', details: { reason: 'RESOURCE_UNAVAILABLE' } }, 404)

/** Roles and their denial choices here are synthetic scenarios, NOT inferred backend policy. */
async function socialCell(role: Role): Promise<Cell[]> {
  const calls: string[] = [], userId = actor(role)
  let mode: 'normal' | 'poison' | 'delayed' = 'normal'
  let storedRequest = { ...approved }
  const late = deferred<Response>()
  const secret = { request_id: approved.request_id, platform: 'INSTAGRAM', owner_id: role === 'owner' ? member : owner,
    handle_version: role === 'owner' ? 3 : 2, handle: markers[0], grant_revision: 1, expires_at: approved.expires_at }
  const fetch: typeof globalThis.fetch = async input => {
    const request = input as Request, path = new URL(request.url).pathname
    calls.push(request.method + ':' + path.split('/').at(-1))
    if (!participant(role)) return denied()
    if (path.endsWith('/reveal')) {
      check(request.cache === 'no-store', 'sensitive read no-store')
      check(request.credentials === 'same-origin', 'sensitive read same-origin')
      if (mode === 'delayed') return late.promise
      return noStore(secret)
    }
    const requestBody = mode === 'poison' ? { ...storedRequest, handle: markers[0] } : storedRequest
    if (path.endsWith('/social-requests')) return noStore({ items: [requestBody], next_cursor: null })
    if (path.endsWith('/revoke')) {
      check(request.method === 'POST' && !!request.headers.get('Idempotency-Key'), 'canonical mutation intent')
      storedRequest = { ...approved, state: 'REVOKED', revision: 3 }
      return noStore(storedRequest)
    }
    return noStore(requestBody)
  }
  const auth = createFixtureSessionController({ eventId, fetch })
  auth.session.replace(0, { userId, eventId, accessToken: 'qa04-fixture-only-token' })
  const controller = createSocialSharingController({ auth, sensitiveRead: auth.requestSensitive,
    access: { read: () => ({ available: true, readOnly: false }), subscribe: () => () => undefined }, now: () => epoch })
  controller.start()
  const view = mountPrivateView('handle', role, createElement(SharingRequest, { controller, retry: () => { void controller.loadRequest(approved.request_id) } }))
  try {
    if (role === 'owner') controller.setDirection('OUTGOING')
    await controller.loadRequests()
    check(controller.read().requests.phase === (participant(role) ? 'ready' : 'error'), 'list role scenario')
    assertAbsent(controller.read())
    await controller.loadRequest(approved.request_id)
    check(controller.read().detail.phase === (participant(role) ? 'ready' : 'error'), 'detail role scenario')
    check(!calls.some(call => call.endsWith(':reveal')), 'no reveal before explicit action')
    view.absent('before explicit reveal')
    if (participant(role)) {
      view.click('Reveal Instagram')
      await until(() => controller.read().secret.phase === 'ready')
      check(controller.read().secret.data?.handle === markers[0], 'authorized fixture secret visible')
      view.visible(markers[0])
      auth.privacy.changePolicy(auth.privacy.capture(), 'consent-revoked')
      assertAbsent(controller.read())
      view.absent('after consent revoked')
      // A malicious 200 containing a raw handle must fail the real strict client decoder.
      mode = 'poison'
      await controller.loadRequests(); await controller.loadRequest(approved.request_id)
      check(controller.read().requests.phase === 'error' && controller.read().detail.phase === 'error', 'poisoned list/detail rejected')
      assertAbsent(controller.read())
      auth.privacy.changePolicy(auth.privacy.capture(), 'revalidated')
      mode = 'normal'; await controller.loadRequest(approved.request_id)
      mode = 'delayed'
      const pending = controller.revealHandle('INSTAGRAM')
      await until(() => calls.filter(call => call.endsWith(':reveal')).length === 2)
      auth.privacy.changePolicy(auth.privacy.capture(), 'block')
      late.resolve(noStore(secret))
      check(await pending === false, 'late reveal fenced after block')
      assertAbsent(controller.read())
      view.absent('after blocked late response')
      mode = 'normal'; await controller.loadRequest(approved.request_id)
      check(await controller.revokeRequest(), 'canonical fixture revoke')
      assertAbsent(controller.read())
    } else {
      check(await controller.revealHandle('INSTAGRAM') === false, 'denied reveal remains unavailable')
      check(await controller.revokeRequest() === false, 'no mutation from denied detail')
      check(!calls.some(call => call.startsWith('POST:') || call.endsWith(':reveal')), 'no unauthorized action dispatch')
      assertAbsent(controller.read())
    }
    view.absent('final revoked or denied state'); view.finish()
    return (['list', 'detail', 'mutation'] as const).map(operation => ({ role, operation, status: 'PASS', provenance: 'SYNTHETIC_FIXTURE',
      evidence: participant(role) ? 'Actual social controller; approved/revoke/strict poison decoder/late reveal fences. Synthetic principal and HTTP only.' : 'Actual controller consumes synthetic generic404; no private payload or unauthorized mutation/reveal dispatch. Admin has no social bypass.' }))
  } finally { view.dispose(); controller.dispose(); auth.dispose() }
}

async function cacheCell(role: Role): Promise<Cell> {
  const session = createMemorySession(), client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  session.replace(0, { userId: actor(role), eventId, accessToken: 'qa04-cache-only-token' })
  const privacy = createQueryPrivacy({ session, client }), scope = privacy.capture()
  const payload = Object.fromEntries(['handle', 'claim', 'evidence', 'beacon', 'recipient'].map((key, i) => [key, markers[i]]))
  // Deliberate tainted prior state: proves scrubbing, not the right to fetch these data.
  const options = privacy.options({ resource: 'participant', read: () => payload })
  const observer = new QueryObserver(client, options), off = observer.subscribe(() => undefined)
  const lateRead = deferred<typeof payload>(), lateMutation = deferred<typeof payload>()
  const pendingRead = privacy.run(scope, () => lateRead.promise).then(() => false, () => true)
  let draft: unknown = payload, subscriptions = 1, success = 0
  const mutation = new MutationObserver(client, { mutationFn: () => privacy.run(scope, () => lateMutation.promise), onSuccess: () => { success++ } })
  const stopMutation = mutation.subscribe(() => undefined)
  const cleanup = privacy.registerCleanup(() => { draft = null; subscriptions = 0; mutation.reset() })
  const pendingMutation = mutation.mutate(undefined).then(() => false, () => true)
  try {
    await until(() => observer.getCurrentResult().data !== undefined)
    client.setQueryData(['unclassified-private-view'], payload)
    client.setQueryData(publicQueryKey(), { providers: [] })
    const publicState = dehydrate(client, publicOnlyDehydrateOptions)
    assertAbsent(publicState)
    check(publicState.queries.length === 1 && publicState.mutations.length === 0, 'public-only dehydration')
    const reasons: Partial<Record<Role, PolicyChange>> = { owner: 'consent-revoked', member: 'handle-replaced', stranger: 'consent-revoked', blocked: 'block', suspended: 'suspended', expired: 'consent-expired' }
    if (role === 'admin') session.replace(session.read().generation, { userId: id(999), eventId, accessToken: 'qa04-next-identity-token' })
    else privacy.changePolicy(scope, reasons[role]!)
    check(observer.getCurrentResult().data === undefined, 'active query observer synchronously scrubbed')
    check(draft === null && subscriptions === 0, 'draft and subscription cleanup')
    assertAbsent(client.getQueryCache().getAll().map(query => query.state))
    assertAbsent(mutation.getCurrentResult())
    check(client.getMutationCache().getAll().length === 0, 'mutation cache cleared')
    check(!privacy.isCurrent(scope), 'old lifetime invalid')
    lateRead.resolve(payload); lateMutation.resolve(payload)
    check(await pendingRead && await pendingMutation, 'late reads/mutations rejected')
    await Promise.resolve()
    check(success === 0, 'late success callback suppressed')
    assertAbsent([observer.getCurrentResult(), mutation.getCurrentResult(), dehydrate(client, publicOnlyDehydrateOptions)])
    return { role, operation: 'cache', status: 'PASS', provenance: 'SYNTHETIC_FIXTURE', evidence: 'Actual SessionPort/PrivacyController/Query+Mutation observers, five-domain taint scrubbing, late completion fences and public dehydration. Revocation invoked locally, not WS.' }
  } finally { cleanup(); off(); observer.destroy(); stopMutation(); privacy.dispose(); client.clear() }
}

function projections() {
  const publicGiveaway = giveaway.giveaway
  const scope = { userId: owner, eventId, generation: 1 }
  const partyPath = `/api/concert/events/${eventId}/parties/${party.party_id}`
  const giftPath = `/api/concert/events/${eventId}/giveaways/${publicGiveaway.giveaway_id}`
  const cases: [string, unknown, (value: unknown) => unknown][] = [
    ['intro', intro, value => publicIntroSchema.parse(value)],
    ['feed', post, value => feedValue('Domain_feed_PostPublic', value)],
    ['party', party, value => decodeParty(party.party_id, partyPath)(value, { scope, path: partyPath, status: 200 })],
    ['giveaway', publicGiveaway, value => giveawayDecoder(scope, publicGiveaway.giveaway_id)(value, { scope, path: giftPath, status: 200 })],
    ['beacon', beacon, value => beaconSchema.parse(value)],
    ['report', { report_id: id(803), event_id: eventId, target_type: 'USER', target_id: member, state: 'OPEN', revision: 1, created_at: approved.created_at }, value => reportSchema.parse(value)],
  ]
  return cases.map(([name, input, decode]) => {
    const baseline = decode(input)
    assertAbsent(baseline)
    // Reject added private fields; never strip evidence silently to make a response appear safe.
    const fields = ['handle', 'claim', 'evidence', 'beacon_secret', 'recipient_secret']
    for (let i = 0; i < fields.length; i++) {
      let rejected = false
      try { decode({ ...input as object, [fields[i]!]: markers[i] }) } catch { rejected = true }
      check(rejected, `${name} rejects private field ${fields[i]}`)
    }
    return { name, status: 'PASS', classification: name === 'beacon' ? 'AUTHORIZED_SHAPE_ONLY_NOT_PUBLIC_AUDIENCE_PROOF' : 'PUBLIC_PROJECTION_SHAPE_ONLY', injectedPrivateFields: fields.length }
  })
}

export async function runFixtures() {
  const cells: Cell[] = []
  for (const role of roles) {
    cells.push(...await socialCell(role), await cacheCell(role), { role, operation: 'WS', status: 'UNSUPPORTED', provenance: 'SYNTHETIC_FIXTURE', evidence: 'No frozen social privacy/revocation WS message or registered public transport handoff. C05 world revoke is not a social grant event.' })
  }
  check(cells.length === roles.length * operations.length, 'matrix complete without dropped cells')
  check(new Set(cells.map(cell => cell.role + ':' + cell.operation)).size === cells.length, 'unique matrix cells')
  const domains = ['handle', 'claim', 'evidence', 'beacon', 'recipient']
  const evidence = (await Promise.all(roles.map(role => evidenceCells(role)))).flat()
  const recipient = (await Promise.all(roles.map(role => recipientCells(role)))).flat()
  const beacon = (await Promise.all(roles.map(role => beaconCells(role)))).flat()
  const featureCells = { evidence, recipient, beacon }
  const domainMatrix = domains.flatMap(domain => cells.map(cell => ({ domain, role: cell.role, operation: cell.operation,
    status: domain in featureCells && featureCells[domain as keyof typeof featureCells].some(result => result.role === cell.role && result.operation === cell.operation) ? 'PASS' : domain === 'handle' || cell.operation === 'cache' ? cell.status : 'UNSUPPORTED', provenance: 'SYNTHETIC_FIXTURE',
    evidence: domain in featureCells ? featureCells[domain as keyof typeof featureCells].find(result => result.role === cell.role && result.operation === cell.operation)?.evidence ?? (domain === 'beacon' && cell.operation === 'list' ? 'UNSUPPORTED: no frozen beacon list operation; direct beacon detail is exercised through the current public interface.' : cell.evidence) : domain === 'handle' || cell.operation === 'cache' ? cell.evidence : 'Public DTO leak-rejection checks are separate; raw private claim role/operation access has no frozen frontend interface in this increment. Actual backend policy/runtime inputs required; no result fabricated.' })))
  return { task: 'QA04', result: 'MOCK_READY', originalTask: 'IN_PROGRESS', qualification: 'SYNTHETIC_CLIENT_FIXTURE_ONLY', cells, domainMatrix, mountedUI, projections: projections(),
    realService: 'NOT_RUN', remaining: ['Actual seeded principals and registered public services', 'HTTP/WS role authorization, real revocation delivery and cached product UI qualification'],
    nonClaims: ['No QA04 live acceptance', 'No IN02/IN03/QA07/FE09 production acceptance', 'No server role-policy proof', 'No actual WS/cache service execution'] }
}
