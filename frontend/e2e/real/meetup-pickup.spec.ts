import { readFile, writeFile, chmod } from 'node:fs/promises'
import { randomUUID } from 'node:crypto'
import { test, expect, type Page, type BrowserContext, type Response, type Route, type APIResponse, type Locator } from '@playwright/test'
import { WorldObserver } from './world-gate/observer'
import { loadConfig, Prerequisite, GateFailure, requirePrerequisite, verify, record, uuid, sha256, receiptResource, inventory, compareInventory,
  rallySignature, validateBeacon, validateReservation, validateJournal, initialChecks, result, failure,
  domainValidator, loadJourneyWorldDecoder, serverResponseClock, rejectedPickupCodeReceipt, type Config, type Binding, type Step, type Inventory } from './meetup-pickup-helpers'

test.use({ trace: 'off', screenshot: 'off', video: 'off' })
test.setTimeout(240_000)

test('IN02 real owner and recipient complete meeting, reservation, recovery and one-time pickup', async ({ playwright }, info) => {
  const checks = initialChecks(), contexts: BrowserContext[] = [], observers: WorldObserver[] = []
  const report = { kind: 'mcr-in02-meetup-pickup-v1', status: 'NOT_RUN' as 'PASS' | 'FAIL' | 'NOT_RUN', checks,
    harnessBaseline: 'ef9422c4d83f8555ee5b48ef693d37fd5bf87e0c', sourceWorkBase: 'adec636683a37d9799e409f718a9aa15e60f00bd', declaredRelease: '', prerequisiteHash: '', cohortReceiptHash: '', sourceHashes: {} as Record<string, string>,
    observations: { independentContexts: 0, browserUiMutations: 0, realProtocolProbes: 0, suppressedRealReservationResponses: 0,
      reloadReservationPosts: 0, originalStatusReads: 0, actualMovementObserved: false, partyMemberCount: 0,
      before: null as Inventory | null, reserved: null as Inventory | null, claimed: null as Inventory | null,
      invalidCodeFailureDurable: false, duplicateClaimReceiptStable: false, cachedPrivateGuidanceWithdrawn: false, socketsClosed: false },
    scope: 'Real configured deployment only. Two independent real logins, real HTTP/C05 observations. One real reservation response is suppressed after its service effect. No fabricated server response, venue authority, account, trace, screenshot, video or credential export.',
    limits: 'Two roles and one dedicated unit. Same-recipient new-key and same-key probes verify uniqueness/conservation; five independent contenders, device acceptance and broad load qualification remain separate.' }
  let browser: Awaited<ReturnType<typeof playwright.chromium.launch>> | undefined
  let c: Config | undefined, pages: [Page, Page] | undefined, observedServerTime: ReturnType<typeof serverResponseClock> | undefined
  let decode: (name: string, value: unknown) => unknown = () => { throw new GateFailure('CONTRACT_VALIDATOR_UNAVAILABLE') }
  let before: Inventory, reserved: Inventory, original: Binding | undefined, reservationId = '', beaconId = '', applicationId = ''
  let beaconReadHeaders: Record<string, string> | undefined
  let continueFlow = true
  const base = () => `/api/concert/events/${c!.world.eventId}`
  const time = () => { verify(observedServerTime, 'SERVER_REQUEST_CLOCK_INTERVAL_REQUIRED'); const now = observedServerTime(performance.now()); verify(now < c!.qualificationExpiresAt, 'QUALIFICATION_EXPIRED_DURING_RUN'); return now }
  const step = async (name: Step, work: () => Promise<void>, independent = false) => {
    if (!continueFlow && !independent) return
    try { await work(); checks[name] = { status: 'PASS' } }
    catch (error) { checks[name] = failure(error); continueFlow = false }
  }
  async function present(locator: Locator, code: string, enabled = false) {
    try { await expect(locator).toBeVisible({ timeout: 10000 }); if (enabled) await expect(locator).toBeEnabled({ timeout: 10000 }) }
    catch { throw new GateFailure(code) }
  }
  const matches = (r: Response, method: string, path: string) => { const url = new URL(r.url()); return url.origin === c!.world.origin && url.pathname === path && r.request().method() === method }
  function noStore(r: Response | APIResponse) { verify((r.headers()['cache-control'] ?? '').toLowerCase().includes('no-store'), 'PRIVATE_RESPONSE_NO_STORE_REQUIRED') }
  async function json(r: Response | APIResponse, schema?: string) {
    verify(r.status() === 200, 'REAL_HTTP_SUCCESS_REQUIRED'); noStore(r)
    let data: unknown
    try { data = await r.json() } catch { throw new GateFailure('INVALID_JSON_RESPONSE') }
    if (schema) { try { return decode(schema, data) } catch { throw new GateFailure('C04_RESPONSE_CONTRACT') } }
    return data
  }
  async function response(page: Page, method: string, path: string, action: () => Promise<unknown>) {
    const waiting = page.waitForResponse(r => matches(r, method, path), { timeout: 15000 })
    // Attach a rejection handler before dispatch, including when the control action itself fails.
    const guarded = waiting.catch(() => { throw new GateFailure('EXPECTED_REAL_SERVICE_RESPONSE_MISSING') })
    const actionResult = action()
    const [r] = await Promise.all([guarded, actionResult]); return r
  }
  async function open(page: Page, path: string) {
    verify(path.startsWith('/') && !path.startsWith('//'), 'UNSAFE_NAVIGATION')
    if (new URL(page.url()).pathname === path) return
    const links = (target: string) => page.locator(`a[href="${target}"]`).filter({ visible: true }).first()
    async function click(target: string) {
      const link = links(target); await present(link, 'SAME_SHELL_PRODUCT_LINK_MISSING')
      const world = await page.locator('[data-world-host]').elementHandle(); verify(world, 'CANONICAL_WORLD_HOST_REQUIRED')
      try {
        await link.click(); await expect(page).toHaveURL(c!.world.origin + target)
        verify(await world.evaluate(node => node.isConnected), 'PANEL_NAVIGATION_REPLACED_WORLD_HOST')
      } finally { await world.dispose() }
    }
    // The shell owns one memory-only Auth identity and one persistent World.
    // Protected-panel page.goto/reload would discard both owners.
    if (await links(path).isVisible()) { await click(path); return }
    if (path === '/world' || path === '/meet' || path === '/gifts') { await click(path); return }
    if (path === '/world/check-in') {
      await open(page, '/world')
      const tools = page.getByRole('button', { name: 'World tools', exact: true })
      if (await tools.getAttribute('aria-expanded') !== 'true') await tools.click()
      await click(path); return
    }
    if (path.startsWith('/meet/parties/')) {
      await open(page, '/meet')
      // Read the current decoded MY PLAN, including membership just approved in
      // the other context; route re-entry alone does not restart its controller.
      await page.getByRole('button', { name: 'MY PLAN', exact: true }).click()
      await response(page, 'GET', `${base()}/me/parties`, () => page.getByRole('button', { name: 'Refresh MY PLAN', exact: true }).click())
      await page.getByRole('button', { name: 'Parties', exact: true }).click()
      await response(page, 'GET', `${base()}/parties`, () => page.getByRole('button', { name: 'Refresh parties', exact: true }).click())
      const detail = `/meet/parties/${c!.partyId}`
      verify(path === detail || path === `${detail}/manage` || path === `${detail}/rally`, 'UNASSIGNED_PARTY_NAVIGATION')
      await click(detail)
      if (path !== detail) await click(path)
      return
    }
    if (path.startsWith('/meet/beacons/')) {
      verify(path === `/meet/beacons/${beaconId}` && uuid(beaconId), 'UNASSIGNED_BEACON_NAVIGATION')
      await open(page, `/meet/parties/${c!.partyId}/rally`)
      const rally = await readRally(page); verify(rally.beacon_id === beaconId, 'CURRENT_RALLY_BEACON_BINDING')
      await click(path); return
    }
    if (path.startsWith('/gifts/')) {
      verify(path === `/gifts/${c!.giveawayId}/manage` || path === `/gifts/${c!.giveawayId}/reservation`, 'UNASSIGNED_GIFT_NAVIGATION')
      await open(page, '/gifts'); await click(path); return
    }
    throw new GateFailure('UNSUPPORTED_SAME_SHELL_NAVIGATION')
  }
  async function readOnOpen(page: Page, route: string, api: string, schema: string) {
    if (new URL(page.url()).pathname === route) {
      const read = page.getByRole('button', { name: route.endsWith('/manage') && route.startsWith('/meet/') ? 'Read current party' : 'Read current reservation', exact: true })
      if (await read.isVisible()) return json(await response(page, 'GET', api, () => read.click()), schema)
      // Owner inventory has no always-available refresh button. Re-enter via
      // actual list links so its existing mount reads management again.
      await open(page, route.startsWith('/gifts/') ? '/gifts' : '/meet')
    }
    return json(await response(page, 'GET', api, () => open(page, route)), schema)
  }
  async function login(page: Page, actor: number) {
    const button = page.getByRole('button', { name: 'Sign in with email', exact: true })
    const entry = page.getByRole('link', { name: 'Sign in', exact: true }).first()
    await expect.poll(async () => await button.isVisible() || await entry.isVisible(), { timeout: 15000 }).toBe(true)
    if (!await button.isVisible()) await entry.click()
    await present(button, 'REAL_EMAIL_AUTH_UNAVAILABLE')
    const account = c!.world.accounts[actor]!
    await page.getByLabel('Email', { exact: true }).fill(account.email); await page.getByLabel('Password', { exact: true }).fill(account.password)
    const bootstrap = page.waitForResponse(r => matches(r, 'GET', `${base()}/bootstrap`), { timeout: 20000 })
    const requestStarted = performance.now()
    const [signedIn, boot] = await Promise.all([response(page, 'POST', '/api/auth/login', () => button.click()), bootstrap])
    const responseReceived = performance.now()
    requirePrerequisite(signedIn.ok(), 'PROVISIONED_REAL_LOGIN_REJECTED')
    const identity: unknown = await signedIn.json(); verify(record(identity) && identity.userId === account.userId, 'INDEPENDENT_LOGIN_IDENTITY')
    const b: unknown = await boot.json()
    verify(boot.status() === 200 && record(b) && record(b.event) && b.event.event_id === c!.world.eventId && record(b.participant)
      && b.participant.user_id === account.userId && b.participant.event_id === c!.world.eventId, 'BOOTSTRAP_EVENT_ACTOR_CLOCK_BINDING')
    noStore(boot); observedServerTime = serverResponseClock(b.server_time, requestStarted, responseReceived)
    await present(page.locator('[data-world-host] canvas'), 'IN02_REAL_WORLD_NOT_AVAILABLE')
  }
  async function command(page: Page, actor: number, method: string, path: string, operation: string, schema: string, action: () => Promise<unknown>) {
    const r = await response(page, method, path, action), key = r.request().headers()['idempotency-key']
    verify(uuid(key), 'COMMAND_IDEMPOTENCY_KEY_REQUIRED'); report.observations.browserUiMutations++
    const binding: Binding = { actorId: c!.world.accounts[actor]!.userId, eventId: c!.world.eventId, operation, key }
    const sent: unknown = method === 'DELETE' ? null : r.request().postDataJSON()
    if (r.status() !== 202) return { data: await json(r, schema), binding, resource: null, sent }
    noStore(r)
    const resource = await settle(page, binding, await r.json())
    return { data: null, binding, resource, sent }
  }
  async function settle(page: Page, binding: Binding, value: unknown) {
    decode('Domain_common_CommandReceipt', value)
    for (let i = 0; i < 8; i++) {
      const resource = receiptResource(value, binding, time())
      if (resource) return resource
      const button = page.getByRole('button', { name: /^(Check original command|Check original command status|Check command status)$/ })
      await expect(button).toBeEnabled()
      const status = await response(page, 'GET', `${base()}/commands`, () => button.click())
      const url = new URL(status.url()); verify(url.searchParams.get('operation_id') === binding.operation && url.searchParams.get('idempotency_key') === binding.key, 'ORIGINAL_STATUS_QUERY_BINDING')
      value = await json(status, 'Domain_common_CommandReceipt')
      if (record(value) && value.state === 'PENDING') await page.waitForTimeout(250)
    }
    throw new GateFailure('COMMAND_SETTLEMENT_TIMEOUT')
  }
  async function management() {
    return readOnOpen(pages![0], `/gifts/${c!.giveawayId}/manage`, `${base()}/giveaways/${c!.giveawayId}/management`, 'Domain_giveaway_GiveawayManagement')
  }
  async function stock() { return inventory(await management(), c!.world.eventId, c!.giveawayId, c!.world.accounts[0].userId) }
  async function readRally(page: Page) {
    const button = page.getByRole('button', { name: 'Read current rally', exact: true })
    await present(button, 'QUALIFIED_PARTY_MEETING_AUTHORITY_UNAVAILABLE', true)
    const v = await json(await response(page, 'GET', `${base()}/parties/${c!.partyId}/rally`, () => button.click()), 'Domain_party_Rally')
    rallySignature(v, c!.world.eventId, c!.partyId); return v as Record<string, unknown>
  }
  // A bounded protocol probe forwards only the captured real same-origin protected request.
  // It never exports headers, plaintext code or a request/response body to evidence.
  async function proxy(route: Route, method: string, path: string, postData?: unknown, changedKey?: string) {
    verify(new URL(route.request().url()).origin === c!.world.origin, 'PROBE_ORIGIN_BINDING')
    const headers = { ...await route.request().allHeaders() }
    delete headers['content-length']
    if (method === 'GET') { delete headers['idempotency-key']; delete headers['content-type'] }
    else if (changedKey) headers['idempotency-key'] = changedKey
    return route.fetch({ url: c!.world.origin + path, method, headers, postData: method === 'GET' ? undefined : postData,
      timeout: 12000, maxRedirects: 0, maxRetries: 0 })
  }
  async function resolveProxy(route: Route, r: APIResponse, binding: Binding, schema: string, resourcePath: (id: string) => string, resourceSchema = schema) {
    if (r.status() !== 202) return json(r, schema)
    noStore(r)
    let value = await r.json(); decode('Domain_common_CommandReceipt', value)
    for (let i = 0; i < 8; i++) {
      const resource = receiptResource(value, binding, time())
      if (resource) return json(await proxy(route, 'GET', resourcePath(resource.id)), resourceSchema)
      await new Promise(resolve => setTimeout(resolve, 250))
      value = await json(await proxy(route, 'GET', `${base()}/commands?operation_id=${binding.operation}&idempotency_key=${binding.key}`), 'Domain_common_CommandReceipt')
    }
    throw new GateFailure('PROBED_COMMAND_SETTLEMENT_TIMEOUT')
  }
  try {
    for (const path of ['./meetup-pickup.spec.ts', './meetup-pickup-helpers.ts', './world-gate/config.ts', './world-gate/observer.ts', './world-gate/decoder.ts',
      '../../../contracts/domain/v1/operations.json', '../../../contracts/domain/v1/policy.json', '../../../contracts/domain/v1/party.schema.json',
      '../../../contracts/domain/v1/beacon.schema.json', '../../../contracts/domain/v1/giveaway.schema.json', '../../../contracts/generated/mcr-domain.openapi.json',
      '../../src/app/router.tsx', '../../src/app/CommunityFeatures.tsx', '../../src/app/community-entry/routes.tsx', '../../src/app/world-presentation/WorldTools.tsx',
      '../../src/features/party-discovery/ui/PartyDiscovery.tsx', '../../src/features/party-management/ui/PartyManagement.tsx',
      '../../src/features/party-authoring/model/contract.ts', '../../src/features/meet-beacon/model/contract.ts']) report.sourceHashes[path] = sha256(await readFile(new URL(path, import.meta.url)))
    await step('configuration', async () => { c = await loadConfig(); report.declaredRelease = c.world.releaseRevision; report.prerequisiteHash = c.prerequisiteHash; report.cohortReceiptHash = c.world.serviceReceipt.sha256; decode = await domainValidator() })
    await step('servicesAndIndependentAccounts', async () => {
      try { browser = await playwright.chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }) }
      catch { throw new Prerequisite('REAL_BROWSER_EXECUTABLE_UNAVAILABLE') }
      const worldDecode = await loadJourneyWorldDecoder()
      for (let i = 0; i < 2; i++) {
        const context = await browser.newContext({ viewport: { width: 390, height: 844 }, serviceWorkers: 'block', acceptDownloads: false }); contexts.push(context); report.observations.independentContexts = contexts.length
        const page = await context.newPage(), observer = new WorldObserver(worldDecode); observers.push(observer); observer.attach(page, c!.world.origin)
        try { const r = await page.goto(c!.world.origin + '/login?returnTo=%2Fworld', { timeout: 15000 }); requirePrerequisite(r?.ok(), 'REAL_ORIGIN_UNAVAILABLE') }
        catch (e) { if (e instanceof Prerequisite) throw e; throw new Prerequisite('REAL_ORIGIN_UNAVAILABLE') }
        await login(page, i)
      }
      verify(contexts[0] !== contexts[1], 'INDEPENDENT_BROWSER_CONTEXTS_REQUIRED'); report.observations.independentContexts = 2
      pages = [contexts[0]!.pages()[0]!, contexts[1]!.pages()[0]!]
      await expect.poll(() => observers.every((o, i) => o.baseline?.selfUserId === c!.world.accounts[i]!.userId && o.baseline.eventId === c!.world.eventId
        && c!.world.accounts.every(a => o.players.has(a.userId))) && observers[0]!.baseline?.roomId === observers[1]!.baseline?.roomId, { timeout: 15000 }).toBe(true)
    })
    await step('rootAndResourcePrerequisites', async () => {
      await open(pages![0], `/meet/parties/${c!.partyId}/manage`)
      await present(pages![0].getByRole('heading', { name: 'Applications', exact: true }), 'COMMUNITY_MANAGEMENT_ROOT_NOT_COMPOSED')
      const p = await readOnOpen(pages![0], `/meet/parties/${c!.partyId}/manage`, `${base()}/parties/${c!.partyId}`, 'Domain_party_PartyPublic')
      requirePrerequisite(record(p) && record(p.owner) && p.owner.user_id === c!.world.accounts[0].userId && p.party_id === c!.partyId && p.event_id === c!.world.eventId
        && p.state === 'OPEN' && p.member_count === 1 && p.capacity === 2 && p.public_place_id === c!.place.id
        && Date.parse(String(p.meeting_starts_at)) <= time() && Date.parse(String(p.meeting_ends_at)) > time() + 120000, 'DEDICATED_OPEN_PARTY_REQUIRED')
      before = await stock(); report.observations.before = before
      requirePrerequisite(before.total === 1 && before.available === 1 && before.reserved === 0 && before.claimed === 0, 'DEDICATED_SINGLE_UNIT_GIVEAWAY_REQUIRED')
      const gift = await readOnOpen(pages![1], `/gifts/${c!.giveawayId}/reservation`, `${base()}/giveaways/${c!.giveawayId}`, 'Domain_giveaway_GiveawayPublic')
      await present(pages![1].getByRole('button', { name: 'Confirm reservation', exact: true }), 'RESERVATION_ROOT_NOT_COMPOSED', true)
      requirePrerequisite(record(gift) && gift.state === 'OPEN' && gift.public_place_id === c!.place.id && Date.parse(String(gift.pickup_starts_at)) <= time()
        && Date.parse(String(gift.pickup_ends_at)) > time() + 120000, 'CURRENT_READY_PICKUP_WINDOW_REQUIRED')
      // Actual FE20 public-place/audience qualification is tested before any party/gift mutation.
      await open(pages![0], '/world/check-in')
      const place = pages![0].getByLabel('Reviewed place', { exact: true }), audience = pages![0].getByLabel('Who can see this?', { exact: true })
      await present(place, 'QUALIFIED_PHYSICAL_PLACE_PORT_UNAVAILABLE', true)
      await present(audience, 'QUALIFIED_PARTY_AUDIENCE_UNAVAILABLE', true)
      requirePrerequisite(await place.locator('option').evaluateAll((items, id) => items.some(item => (item as HTMLOptionElement).value === id), c!.place.id), 'QUALIFIED_PHYSICAL_PLACE_OPTION_UNAVAILABLE')
      requirePrerequisite(await audience.locator('option').evaluateAll((items, id) => items.some(item => (item as HTMLOptionElement).value === id), `party:${c!.partyId}`), 'QUALIFIED_PARTY_AUDIENCE_OPTION_UNAVAILABLE')
      await place.selectOption(c!.place.id); await audience.selectOption(`party:${c!.partyId}`)
      await expect(place).toHaveValue(c!.place.id); await expect(audience).toHaveValue(`party:${c!.partyId}`)
    })
    await step('partyApproval', async () => {
      const applyPath = `${base()}/parties/${c!.partyId}/applications`
      await open(pages![1], `/meet/parties/${c!.partyId}`)
      const apply = pages![1].getByRole('button', { name: 'Apply to party', exact: true }); await present(apply, 'PARTY_APPLICATION_UNAVAILABLE', true)
      const application = await command(pages![1], 1, 'POST', applyPath, 'applyConcertParty', 'Domain_party_Application', () => apply.click())
      if (application.data) { verify(record(application.data) && application.data.party_id === c!.partyId && record(application.data.applicant)
        && application.data.applicant.user_id === c!.world.accounts[1].userId && application.data.state === 'PENDING' && uuid(application.data.application_id), 'PARTY_APPLICATION_BINDING'); applicationId = application.data.application_id }
      else applicationId = application.resource!.id
      const apps = await readOnOpen(pages![0], `/meet/parties/${c!.partyId}/manage`, applyPath, 'Domain_party_ApplicationList')
      verify(record(apps) && Array.isArray(apps.items) && apps.next_cursor === null, 'COMPLETE_APPLICATION_PAGE_REQUIRED')
      const item = apps.items.find(a => record(a) && a.application_id === applicationId)
      verify(record(item) && item.state === 'PENDING' && record(item.applicant) && item.applicant.user_id === c!.world.accounts[1].userId, 'OWNER_SEES_REAL_PENDING_APPLICATION')
      const row = pages![0].getByRole('region', { name: 'Party applications' }).getByRole('listitem').filter({ has: pages![0].getByRole('button', { name: /^Approve / }) })
      verify(await row.count() === 1, 'DEDICATED_APPLICATION_ROW_REQUIRED')
      const decision = await command(pages![0], 0, 'POST', `${base()}/party-applications/${applicationId}/decision`, 'decideConcertApplication', 'Domain_party_Application', () => row.getByRole('button', { name: /^Approve / }).click())
      if (decision.data) verify(record(decision.data) && decision.data.application_id === applicationId && decision.data.state === 'APPROVED', 'APPROVAL_RESULT')
      const members = await readOnOpen(pages![0], `/meet/parties/${c!.partyId}/manage`, `${base()}/parties/${c!.partyId}/members`, 'Domain_party_MemberList')
      verify(record(members) && Array.isArray(members.items) && members.next_cursor === null && members.items.length === 2
        && members.items.some(m => record(m) && m.user_id === c!.world.accounts[1].userId && m.party_id === c!.partyId && m.event_id === c!.world.eventId && m.state === 'ACTIVE'), 'APPROVAL_CREATED_ACTIVE_MEMBERSHIP')
      report.observations.partyMemberCount = members.items.length
    })
    await step('beaconAudienceRevisionExpiry', async () => {
      await open(pages![0], '/world/check-in')
      await pages![0].getByLabel('Reviewed place', { exact: true }).selectOption(c!.place.id)
      await pages![0].getByLabel('Who can see this?', { exact: true }).selectOption(`party:${c!.partyId}`)
      await pages![0].getByLabel('How to find me', { exact: true }).fill(c!.meeting.instructions)
      await pages![0].getByLabel('Identification', { exact: true }).fill(c!.meeting.identification)
      await pages![0].getByLabel('Visible for (seconds)', { exact: true }).fill(String(c!.meeting.ttlSeconds))
      await pages![0].getByRole('checkbox', { name: 'I explicitly confirm this real-world description and its audience.' }).check()
      const made = await command(pages![0], 0, 'POST', `${base()}/beacons`, 'createConcertBeacon', 'Domain_beacon_MeetBeacon', () => pages![0].getByRole('button', { name: 'Share check-in', exact: true }).click())
      decode('Domain_beacon_CreateBeacon', made.sent); verify(record(made.sent) && made.sent.physical_presence_acknowledged === true, 'EXPLICIT_BEACON_ACKNOWLEDGMENT_REQUIRED')
      if (made.data) beaconId = validateBeacon(made.data, c!, time()).id; else beaconId = made.resource!.id
      await pages![1].route(c!.world.origin + `${base()}/beacons/${beaconId}`, async route => {
        verify(route.request().method() === 'GET', 'BEACON_READ_CAPTURE_METHOD')
        beaconReadHeaders = await route.request().allHeaders()
        await route.continue()
      }, { times: 1 })
      const viewed = await readOnOpen(pages![1], `/meet/beacons/${beaconId}`, `${base()}/beacons/${beaconId}`, 'Domain_beacon_MeetBeacon')
      validateBeacon(viewed, c!, time()); await expect(pages![1].getByText(c!.meeting.instructions, { exact: true })).toBeVisible()
      await expect(pages![1].getByText('Approved party members only', { exact: true })).toBeVisible()
    })
    await step('virtualMovementNoArrival', async () => {
      await open(pages![0], `/meet/parties/${c!.partyId}/rally`)
      const beforeRally = await readRally(pages![0]); requirePrerequisite(beforeRally.beacon_id === beaconId, 'QUALIFIED_PARTY_BEACON_LINK_REQUIRED')
      const signature = rallySignature(beforeRally, c!.world.eventId, c!.partyId)
      await open(pages![1], '/world')
      const own = observers[1]!, peer = observers[0]!, actor = c!.world.accounts[1].userId
      await expect.poll(() => !!own.players.get(actor) && !!peer.players.get(actor), { timeout: 15000 }).toBe(true)
      const beforePosition = { ...peer.players.get(actor)!.position }, inputs = own.nonzeroInputs
      const stick = pages![1].getByRole('button', { name: 'Movement joystick. Arrow keys or W A S D when focused', exact: true }); await stick.focus()
      for (const key of ['ArrowRight', 'ArrowDown', 'ArrowLeft', 'ArrowUp']) {
        await pages![1].keyboard.down(key)
        try { await pages![1].waitForTimeout(250) } finally { await pages![1].keyboard.up(key) }
        if (peer.players.get(actor)?.position.x !== beforePosition.x || peer.players.get(actor)?.position.y !== beforePosition.y) break
      }
      await expect.poll(() => own.nonzeroInputs > inputs && !!peer.players.get(actor) && (peer.players.get(actor)!.position.x !== beforePosition.x || peer.players.get(actor)!.position.y !== beforePosition.y)).toBe(true)
      verify(rallySignature(await readRally(pages![0]), c!.world.eventId, c!.partyId) === signature, 'VIRTUAL_MOVEMENT_CHANGED_PHYSICAL_RALLY')
      report.observations.actualMovementObserved = true
    })
    await step('explicitPhysicalRally', async () => {
      await open(pages![1], `/meet/parties/${c!.partyId}/rally`); const beforeRally = await readRally(pages![1])
      const here = pages![1].getByRole('button', { name: /^(HERE|I am here)$/ }); await expect(here).toBeDisabled()
      await pages![1].getByRole('checkbox', { name: /I am (explicitly reporting my physical meeting status|reporting my own real-world status)/ }).check()
      const changed = await command(pages![1], 1, 'PUT', `${base()}/parties/${c!.partyId}/rally/me`, 'setConcertRally', 'Domain_party_RallyEntry', () => here.click())
      decode('Domain_party_SetRally', changed.sent); verify(record(changed.sent) && changed.sent.physical_presence_acknowledged === true
        && changed.sent.state === 'HERE' && changed.sent.meeting_revision === beforeRally.meeting_revision, 'EXPLICIT_RALLY_ACKNOWLEDGMENT_REQUIRED')
      if (changed.data) verify(record(changed.data) && changed.data.user_id === c!.world.accounts[1].userId && changed.data.state === 'HERE'
        && changed.data.meeting_revision === beforeRally.meeting_revision, 'EXPLICIT_SELF_RALLY_RESULT')
      const current = await readRally(pages![0]); verify(current.meeting_revision === beforeRally.meeting_revision && Array.isArray(current.entries)
        && current.entries.some(e => record(e) && e.user_id === c!.world.accounts[1].userId && e.state === 'HERE' && e.meeting_revision === current.meeting_revision), 'PEER_OBSERVED_EXPLICIT_ARRIVAL')
    })
    await step('atomicReservation', async () => {
      await open(pages![1], `/gifts/${c!.giveawayId}/reservation`)
      const target = `${base()}/giveaways/${c!.giveawayId}/reservations`, url = c!.world.origin + target
      let proxyFailure: unknown, finished = false
      await pages![1].route(url, async route => {
        try {
          const req = route.request(); verify(req.method() === 'POST', 'RESERVE_PROBE_METHOD'); const key = req.headers()['idempotency-key']; verify(uuid(key), 'RESERVE_ORIGINAL_KEY')
          original = { eventId: c!.world.eventId, actorId: c!.world.accounts[1].userId, operation: 'reserveConcertGiveaway', key }
          const body: unknown = req.postDataJSON(); decode('Domain_giveaway_Reserve', body)
          // The authenticated recipient must not obtain the owner's inventory or
          // private reservation page. An upstream Auth/dependency error is not proof.
          const ownerOnly = await proxy(route, 'GET', `${base()}/giveaways/${c!.giveawayId}/management`)
          report.observations.realProtocolProbes++; verify(ownerOnly.status() === 404, 'RECIPIENT_OWNER_MANAGEMENT_NOT_DENIED'); noStore(ownerOnly)
          const denied: unknown = await ownerOnly.json()
          verify(record(denied) && Object.keys(denied).every(key => ['code', 'details', 'message', 'traceId', 'timestamp'].includes(key)) && typeof denied.code === 'string' && typeof denied.message === 'string'
            && record(denied.details) && Object.keys(denied.details).join(',') === 'reason' && denied.details.reason === 'RESOURCE_UNAVAILABLE', 'OWNER_MANAGEMENT_PRIVATE_RESPONSE_LEAK')
          const actual = await proxy(route, 'POST', target, body)
          const value = await resolveProxy(route, actual, original, 'Domain_giveaway_Reservation', id => `${base()}/reservations/${id}`)
          const r = validateReservation(value, c!, undefined); reservationId = r.id; verify(r.state === 'READY' && r.expiresAt > time(), 'READY_RESERVATION_REQUIRED')
          report.observations.browserUiMutations++
          const replay = await proxy(route, 'POST', target, body); report.observations.realProtocolProbes++
          const repeated = validateReservation(await resolveProxy(route, replay, original, 'Domain_giveaway_Reservation', id => `${base()}/reservations/${id}`), c!, reservationId)
          verify(repeated.revision === r.revision, 'SAME_KEY_RESERVATION_REPLAY_CHANGED_RESOURCE')
          // Same-key replay preserves the original body above. A new-key
          // uniqueness probe must use the actual current root revision, because
          // the producer checks CAS before checking active reservations.
          const current = await json(await proxy(route, 'GET', `${base()}/giveaways/${c!.giveawayId}`), 'Domain_giveaway_GiveawayPublic')
          report.observations.realProtocolProbes++
          verify(record(current) && current.event_id === c!.world.eventId && current.giveaway_id === c!.giveawayId && current.state === 'OPEN'
            && current.public_place_id === c!.place.id && Date.parse(String(current.pickup_starts_at)) <= time() && Date.parse(String(current.pickup_ends_at)) > time(), 'DUPLICATE_PROBE_CURRENT_GIVEAWAY_REQUIRED')
          const duplicateBody = { giveaway_revision: current.revision }; decode('Domain_giveaway_Reserve', duplicateBody)
          const duplicate = await proxy(route, 'POST', target, duplicateBody, randomUUID()); report.observations.realProtocolProbes++
          verify(duplicate.status() === 409, 'DUPLICATE_RESERVATION_NOT_REJECTED'); noStore(duplicate); const rejected: unknown = await duplicate.json()
          verify(record(rejected) && record(rejected.details) && rejected.details.reason === 'DUPLICATE_ACTIVE_RESERVATION', 'DUPLICATE_ACTIVE_RESERVATION_NOT_PROVEN')
          report.observations.suppressedRealReservationResponses++
        } catch (e) { proxyFailure = e }
        finally { finished = true; await route.abort('failed').catch(() => {}) }
      }, { times: 1 })
      await pages![1].getByRole('button', { name: 'Confirm reservation', exact: true }).click()
      await expect.poll(() => finished, { timeout: 45000 }).toBe(true); if (proxyFailure) throw proxyFailure
      await expect(pages![1].getByText(/Outcome unknown/)).toBeVisible()
      verify(!!original && !!reservationId, 'REAL_RESERVATION_EFFECT_REQUIRED')
      reserved = await stock(); compareInventory(before, reserved, 'reserve'); report.observations.reserved = reserved
    })
    await step('reloadOriginalKeyRecovery', async () => {
      const page = pages![1], binding = original!
      const journalKey = `mcr:pickup-intent:v1:${binding.actorId}:${binding.eventId}`
      const raw = await page.evaluate(key => sessionStorage.getItem(key), journalKey); verify(raw !== null, 'RELOAD_RECOVERY_JOURNAL_MISSING')
      let value: unknown; try { value = JSON.parse(raw) } catch { throw new GateFailure('RECOVERY_JOURNAL_INVALID') }
      validateJournal(value, binding, c!.giveawayId)
      let posts = 0; const watch = (r: import('@playwright/test').Request) => { if (r.method() === 'POST' && new URL(r.url()).pathname === `${base()}/giveaways/${c!.giveawayId}/reservations`) posts++ }
      page.on('request', watch)
      try {
        await page.reload(); await login(page, 1)
        await open(page, `/gifts/${c!.giveawayId}/reservation`)
        await present(page.getByRole('button', { name: 'Check original command', exact: true }), 'RELOAD_STATUS_RECOVERY_UI_UNAVAILABLE', true)
        const reread = page.waitForResponse(r => matches(r, 'GET', `${base()}/reservations/${reservationId}`)).catch(() => null)
        const status = await response(page, 'GET', `${base()}/commands`, () => page.getByRole('button', { name: 'Check original command', exact: true }).click())
        const query = new URL(status.url()); verify(query.searchParams.get('operation_id') === binding.operation && query.searchParams.get('idempotency_key') === binding.key, 'RELOAD_STATUS_USED_DIFFERENT_KEY')
        const receipt = receiptResource(await json(status, 'Domain_common_CommandReceipt'), binding, time()); verify(receipt?.id === reservationId, 'RELOAD_RECEIPT_RESOURCE_MISMATCH')
        const resourceResponse = await reread; verify(resourceResponse, 'RELOAD_RESOURCE_READ_MISSING')
        const observed = validateReservation(await json(resourceResponse, 'Domain_giveaway_Reservation'), c!, reservationId); verify(observed.revision >= receipt.revision && observed.state === 'READY', 'RELOAD_AUTHORIZED_RESOURCE_RECONCILIATION')
        report.observations.originalStatusReads++; await expect(page.getByRole('heading', { name: 'Reservation status', exact: true })).toBeVisible()
        verify(await page.evaluate(key => sessionStorage.getItem(key), journalKey) === null, 'RESOLVED_JOURNAL_NOT_CLEARED')
        verify(posts === 0, 'RELOAD_REPLAYED_RESERVATION'); report.observations.reloadReservationPosts = posts
      } finally { page.off('request', watch) }
      compareInventory(reserved, await stock(), 'duplicate')
    })
    await step('privatePickup', async () => {
      const pickup = await json(await response(pages![1], 'GET', `${base()}/reservations/${reservationId}/pickup`, () => pages![1].getByRole('button', { name: 'Read private pickup details', exact: true }).click()), 'Domain_giveaway_PickupDetails')
      verify(record(pickup) && record(pickup.meeting), 'PRIVATE_PICKUP_RESPONSE'); validateReservation(pickup.reservation, c!, reservationId)
      verify(pickup.meeting.place_id === c!.place.id && pickup.meeting.map_revision === c!.place.mapRevision && pickup.meeting.place_revision === c!.place.placeRevision
        && Date.parse(String(pickup.meeting.expires_at)) > time(), 'PRIVATE_PICKUP_CURRENT_PLACE_EXPIRY')
      await present(pages![1].getByRole('region', { name: 'Authorized pickup details' }), 'QUALIFIED_PICKUP_PLACE_AUTHORITY_UNAVAILABLE')
    })
    await step('oneTimeCollection', async () => {
      const page = pages![1], codePath = `${base()}/reservations/${reservationId}/pickup-code`
      let issued = await command(page, 1, 'POST', codePath, 'issueConcertPickupCode', 'Domain_giveaway_PickupCode', () => page.getByRole('button', { name: 'Request pickup code', exact: true }).click())
      if (issued.data === null) {
        // The completed original issuance cannot recover plaintext. Re-read the real plan,
        // then express one separate rotation intent, never replay the completed key.
        await json(await response(page, 'GET', `${base()}/reservations/${reservationId}/pickup`, () => page.getByRole('button', { name: 'Read private pickup details', exact: true }).click()), 'Domain_giveaway_PickupDetails')
        const previousKey = issued.binding.key
        issued = await command(page, 1, 'POST', codePath, 'issueConcertPickupCode', 'Domain_giveaway_PickupCode', () => page.getByRole('button', { name: 'Request pickup code', exact: true }).click())
        verify(issued.binding.key !== previousKey, 'CODE_ROTATION_REUSED_COMPLETED_KEY')
      }
      requirePrerequisite(issued.data !== null, 'ONE_SHOT_CODE_RESPONSE_REQUIRED_AFTER_EXPLICIT_ROTATION')
      verify(record(issued.data) && issued.data.reservation_id === reservationId && typeof issued.data.confirmation_code === 'string'
        && /^\d{6}$/.test(issued.data.confirmation_code) && Date.parse(String(issued.data.expires_at)) > time(), 'ONE_SHOT_CODE_BINDING_EXPIRY')
      // Read only into transient memory; no assertions/attachments may interpolate this secret.
      let code = await page.getByLabel('Pickup confirmation code').innerText(); verify(/^\d{6}$/.test(code) && code === issued.data.confirmation_code, 'CODE_UI_RESPONSE_BINDING')
      const m = await management(); verify(record(m) && Array.isArray(m.reservations) && m.reservations.length === 1
        && record(m.reservations[0]) && m.reservations[0].reservation_id === reservationId, 'OWNER_EXACT_RESERVATION_ROW')
      await pages![0].getByRole('button', { name: 'Open pickup 1', exact: true }).click()
      await pages![0].getByLabel('Recipient’s six-digit code', { exact: true }).fill(code); code = ''
      const target = `${base()}/reservations/${reservationId}/claim`; let proxyFailure: unknown, finished = false
      let pendingClaim: { binding: Binding; receipt: unknown } | undefined
      await pages![0].route(c!.world.origin + target, async route => {
        try {
          const key = route.request().headers()['idempotency-key']; verify(uuid(key), 'CLAIM_KEY_REQUIRED')
          const binding = { eventId: c!.world.eventId, actorId: c!.world.accounts[0].userId, operation: 'claimConcertReservation', key }
          const body: unknown = route.request().postDataJSON(); decode('Domain_giveaway_ClaimReservation', body)
          verify(record(body), 'CLAIM_BODY_REQUIRED')
          const beforeInvalid = validateReservation(await json(await proxy(route, 'GET', `${base()}/reservations/${reservationId}`), 'Domain_giveaway_Reservation'), c!, reservationId)
          verify(beforeInvalid.state === 'READY' && beforeInvalid.revision === body.expected_revision, 'CURRENT_CODE_READY_RESERVATION_REQUIRED')
          const invalidKey = randomUUID(), invalidBinding = { ...binding, key: invalidKey }
          const wrong = { ...body, confirmation_code: body.confirmation_code === '000000' ? '000001' : '000000' }
          const rejected = await proxy(route, 'POST', target, wrong, invalidKey); report.observations.realProtocolProbes++; noStore(rejected)
          verify(rejected.status() === 403 || rejected.status() === 202, 'INVALID_PICKUP_CODE_NOT_REJECTED')
          if (rejected.status() === 403) {
            const error: unknown = await rejected.json()
            verify(record(error) && record(error.details) && error.details.reason === 'PICKUP_CODE_INVALID', 'INVALID_PICKUP_CODE_REASON')
          }
          // An invalid code spends an admission attempt and has a durable FAILED
          // BE15 invocation. It must not consume inventory, code or reservation.
          let failed = rejected.status() === 202 ? await rejected.json() : await json(await proxy(route, 'GET', `${base()}/commands?operation_id=${binding.operation}&idempotency_key=${invalidKey}`), 'Domain_common_CommandReceipt')
          decode('Domain_common_CommandReceipt', failed)
          for (let i = 0; !rejectedPickupCodeReceipt(failed, invalidBinding, time()); i++) {
            verify(i < 8, 'INVALID_CODE_SETTLEMENT_TIMEOUT'); await new Promise(resolve => setTimeout(resolve, 250))
            failed = await json(await proxy(route, 'GET', `${base()}/commands?operation_id=${binding.operation}&idempotency_key=${invalidKey}`), 'Domain_common_CommandReceipt')
          }
          const afterInvalid = validateReservation(await json(await proxy(route, 'GET', `${base()}/reservations/${reservationId}`), 'Domain_giveaway_Reservation'), c!, reservationId)
          verify(afterInvalid.state === beforeInvalid.state && afterInvalid.revision === beforeInvalid.revision && afterInvalid.receiptId === null, 'INVALID_CODE_CHANGED_RESERVATION')
          compareInventory(reserved, inventory(await json(await proxy(route, 'GET', `${base()}/giveaways/${c!.giveawayId}/management`), 'Domain_giveaway_GiveawayManagement'), c!.world.eventId, c!.giveawayId, c!.world.accounts[0].userId), 'duplicate')
          report.observations.invalidCodeFailureDurable = true
          const first = await proxy(route, 'POST', target, body)
          if (first.status() === 202) pendingClaim = { binding, receipt: await first.json() }
          const confirmed = await resolveProxy(route, first, binding, 'Domain_giveaway_PickupReceipt', id => `${base()}/reservations/${id}`, 'Domain_giveaway_Reservation')
          let claimedAt = record(confirmed) && typeof confirmed.claimed_at === 'string' ? confirmed.claimed_at : undefined
          const initial = validateReservation(await json(await proxy(route, 'GET', `${base()}/reservations/${reservationId}`), 'Domain_giveaway_Reservation'), c!, reservationId)
          verify(initial.state === 'CLAIMED' && uuid(initial.receiptId), 'CLAIM_NOT_DURABLY_CONFIRMED')
          for (const nextKey of [key, randomUUID()]) {
            const replay = await proxy(route, 'POST', target, body, nextKey); report.observations.realProtocolProbes++
            const data = await resolveProxy(route, replay, { ...binding, key: nextKey }, 'Domain_giveaway_PickupReceipt', id => `${base()}/reservations/${id}`, 'Domain_giveaway_Reservation')
            if (record(data) && 'receipt_id' in data) {
              verify(data.receipt_id === initial.receiptId && data.reservation_id === reservationId && data.giveaway_id === c!.giveawayId
                && data.state === 'CLAIMED' && data.revision === initial.revision && typeof data.claimed_at === 'string'
                && (claimedAt === undefined || data.claimed_at === claimedAt), 'DUPLICATE_PICKUP_RECEIPT_CHANGED')
              claimedAt = data.claimed_at
            }
            const current = validateReservation(await json(await proxy(route, 'GET', `${base()}/reservations/${reservationId}`), 'Domain_giveaway_Reservation'), c!, reservationId)
            verify(current.state === 'CLAIMED' && current.receiptId === initial.receiptId && current.revision === initial.revision, 'DUPLICATE_CLAIM_CHANGED_TERMINAL_RESOURCE')
          }
          report.observations.duplicateClaimReceiptStable = true; report.observations.browserUiMutations++
          // Forward the untouched real response, never fabricate a body or fulfill with a fixture.
          await route.fulfill({ response: first })
        } catch (e) { proxyFailure = e; await route.abort('failed').catch(() => {}) }
        finally { finished = true }
      }, { times: 1 })
      await pages![0].getByRole('button', { name: 'Confirm one-time collection', exact: true }).click()
      await expect.poll(() => finished, { timeout: 45000 }).toBe(true); if (proxyFailure) throw proxyFailure
      if (pendingClaim) { const settled = await settle(pages![0], pendingClaim.binding, pendingClaim.receipt); verify(settled.id === reservationId, 'CLAIM_RECOVERY_RESOURCE_BINDING') }
      await expect(pages![0].getByText('The server confirmed one-time collection.', { exact: true })).toBeVisible()
      const claimed = await stock(); compareInventory(reserved, claimed, 'claim'); report.observations.claimed = claimed
      await pages![1].getByRole('button', { name: 'Read current reservation', exact: true }).click()
      await expect(pages![1].getByRole('heading', { name: 'Confirmation code', exact: true })).toHaveCount(0)
      await expect(pages![1].getByRole('region', { name: 'Authorized pickup details' })).toHaveCount(0)
    })
    await step('privacyWithdrawal', async () => {
      await pages![1].route(c!.world.origin + `${base()}/beacons/${beaconId}`, async route => {
        verify(route.request().method() === 'GET', 'BEACON_READ_CAPTURE_METHOD')
        beaconReadHeaders = await route.request().allHeaders(); await route.continue()
      }, { times: 1 })
      await readOnOpen(pages![1], `/meet/beacons/${beaconId}`, `${base()}/beacons/${beaconId}`, 'Domain_beacon_MeetBeacon')
      await expect(pages![1].getByText(c!.meeting.instructions, { exact: true })).toBeVisible()
      await open(pages![0], `/meet/parties/${c!.partyId}/manage`)
      const remove = pages![0].getByRole('button', { name: `Remove member ${c!.world.accounts[1].userId}`, exact: true }); await expect(remove).toBeEnabled(); await remove.click()
      await command(pages![0], 0, 'DELETE', `${base()}/parties/${c!.partyId}/members/${c!.world.accounts[1].userId}`, 'removeConcertMember', 'Domain_party_Member', () => pages![0].getByRole('button', { name: 'Confirm removal', exact: true }).click())
      // Do not navigate/reload the recipient: cached private guidance must withdraw on actual policy invalidation.
      await expect(pages![1].getByText(c!.meeting.instructions, { exact: true })).toHaveCount(0, { timeout: 12000 })
      report.observations.cachedPrivateGuidanceWithdrawn = true
      // Reuse only headers captured from this recipient's actual authenticated
      // beacon read. Reload would discard the in-memory identity and test
      // anonymous access instead of revoked membership.
      verify(beaconReadHeaders && typeof beaconReadHeaders.authorization === 'string', 'AUTHENTICATED_BEACON_READ_REQUIRED')
      const denied = await contexts[1]!.request.get(c!.world.origin + `${base()}/beacons/${beaconId}`, {
        headers: beaconReadHeaders, timeout: 12000, maxRedirects: 0, maxRetries: 0 })
      report.observations.realProtocolProbes++
      verify(denied.status() === 404, 'REMOVED_MEMBER_BEACON_ACCESS_NOT_REVOKED'); noStore(denied)
      const body: unknown = await denied.json(); verify(record(body) && record(body.details) && body.details.reason === 'RESOURCE_UNAVAILABLE', 'PRIVATE_RESOURCE_EXISTENCE_LEAK')
    })
  } finally {
    beaconReadHeaders = undefined
    await step('teardown', async () => {
      requirePrerequisite(contexts.length > 0, 'NO_REAL_CONTEXTS_ALLOCATED')
      const closed = await Promise.allSettled(contexts.map(context => context.close()))
      verify(closed.every(r => r.status === 'fulfilled'), 'BROWSER_CONTEXT_CLEANUP_FAILED')
      await expect.poll(() => observers.every(o => o.active === 0)).toBe(true)
      report.observations.socketsClosed = true
    }, true)
    await browser?.close().catch(() => { checks.teardown = { status: 'FAIL', code: 'BROWSER_CLEANUP_FAILED' } })
    observers.forEach(observer => observer.clear())
    report.status = result(checks)
    const path = info.outputPath('in02-result.json')
    await writeFile(path, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 }); await chmod(path, 0o600)
    await info.attach('in02-result', { path, contentType: 'application/json' })
  }
  // Stable classifications only: never print Playwright errors containing credentials/DOM/code text.
  if (report.status !== 'PASS') throw new Error(`IN02 ${report.status}: ${Object.entries(checks).filter(([, v]) => v.status !== 'PASS').map(([key, value]) => `${key}=${value.code}`).join('; ')}`)
})
