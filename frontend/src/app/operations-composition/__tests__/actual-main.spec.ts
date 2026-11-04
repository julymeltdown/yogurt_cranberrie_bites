import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { isAbsolute, resolve } from 'node:path'
import { test, expect, type Page, type BrowserContext } from '@playwright/test'
import { loadContracts, type GateConfig } from '../../../../e2e/real/mural-helpers'
import { domainOperation } from '../../../shared/api/domain'
import { OPERATIONS_ACCESS_ID, operationsAccessPath } from '../../operations-entry/contract'
import { actualInputs, qualifyAccessContracts } from './actual-inputs'

test('Actual canonical Admin discovery uses genuine operator lease and denies the real member', async ({ playwright }) => {
  const out = process.env.ADMIN_OUTPUT
  assert(out && isAbsolute(out), 'ALLOCATED_PRIVATE_OUTPUT_REQUIRED')
  assert(Number(process.env.ADMIN_WORK_END_EPOCH_MS) > Date.now(), 'ACTUAL_ACCEPTED_CLOCK_REQUIRED')
  await mkdir(out, { recursive: true })
  const report = { kind: 'ACTUAL_DEFAULT_ADMIN_DISCOVERY', status: 'NOT_RUN', stage: 'inputs', candidate: '',
    checks: [] as string[], requests: [] as { actor: string; method: string; path: string; status: number; protected: boolean }[],
    captures: [] as string[], limitations: 'Read-only actual producer discovery only. No role publication, service launch, intercepted HTTP, test registry, identity injection or replay of controlled Wall mutation coverage.' }
  let browser: Awaited<ReturnType<typeof playwright.chromium.launch>> | undefined
  let context: BrowserContext | undefined
  let failure: Error | undefined
  try {
    // These checks happen before Chrome. Missing actual inputs are never a PASS.
    const registered = domainOperation(OPERATIONS_ACCESS_ID)
    assert(registered?.method === 'GET' && registered.path === '/api/concert/events/{eventId}/me/operations-access', 'ACTUAL_CANONICAL_REGISTRY_REQUIRED')
    const { config, observe } = await actualInputs()
    report.candidate = config.releaseRevision
    const contracts = qualifyAccessContracts(await loadContracts(config))
    const operation = contracts.operation(OPERATIONS_ACCESS_ID)
    assert(operation.method.toUpperCase() === 'GET' && operation.path === registered.path, 'CURRENT_PUBLIC_OPERATION_REQUIRED')
    const schemaName = (name: string) => 'Domain_' + name.replaceAll('.', '_')
    const accessPath = operationsAccessPath(config.eventId), settingsPath = `/api/concert/events/${config.eventId}/admin/settings`
    report.checks.push('actual_frontend_backend_and_registered_contract_bound')
    browser = await playwright.chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
      args: ['--disable-background-networking', '--disable-component-update'] })
    for (const label of ['operator', 'member'] as const) {
      report.stage = label; await observe()
      context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce' })
      const page = await context.newPage(), account = label === 'operator' ? config.operator : config.members[0]
      const pageErrors: string[] = []
      let processing = Promise.resolve(), invalidResponse = false
      let admitted = false, own: unknown, settings: unknown
      page.on('pageerror', () => pageErrors.push('PAGE_ERROR'))
      page.on('response', response => {
        const request = response.request(), path = new URL(response.url()).pathname
        if (!path.startsWith('/api/')) return
        const action = async () => {
          const headers = await request.allHeaders()
          const protectedRequest = typeof headers.authorization === 'string' && headers.authorization.startsWith('Bearer ')
          report.requests.push({ actor: label, method: request.method(), path, status: response.status(), protected: protectedRequest })
          if (path === '/api/auth/enrollment/admission' && response.status() === 200) {
            const value: unknown = await response.json()
            admitted = !!value && typeof value === 'object' && 'decision' in value && value.decision === 'ADMITTED'
          }
          if (path !== accessPath && path !== settingsPath) return
          assert(admitted && protectedRequest, 'ACTUAL_ADMISSION_AND_PROTECTED_ADMIN_READ_REQUIRED')
          assert(request.method() === 'GET' && new URL(request.url()).search === '' && request.postData() === null, 'BODYLESS_REGISTERED_DISCOVERY_REQUIRED')
          assert(response.headers()['cache-control']?.includes('no-store'), 'PRIVATE_ADMIN_NO_STORE_REQUIRED')
          assert.equal(response.status(), 200, 'ACTUAL_ADMIN_DISCOVERY_RESPONSE_REQUIRED')
          const value: unknown = await response.json()
          if (path === accessPath) own = contracts.decode(schemaName(operation.response), value)
          else settings = contracts.decode(schemaName(contracts.operation('getConcertSettings').response), value)
        }
        processing = processing.then(action).catch(() => { invalidResponse = true })
      })
      await signIn(page, config, account)
      await page.evaluate(() => { history.pushState(null, '', '/admin/event'); dispatchEvent(new PopStateEvent('popstate')) })
      if (label === 'operator') {
        await expect(page.getByRole('heading', { name: 'Event settings', exact: true })).toBeVisible()
        await processing; assert(!invalidResponse, 'GENUINE_ADMIN_RESPONSE_VALIDATION_REQUIRED')
        const value = own as { server_time: string; access: { actor_id: string; event_id: string; role: string; checked_at: string; expires_at: string } | null }
        assert(value?.access && value.access.actor_id === account.userId && value.access.event_id === config.eventId && value.access.role === 'OPERATOR', 'GENUINE_OWN_OPERATOR_LEASE_REQUIRED')
        assert(value.access.checked_at === value.server_time && Date.parse(value.access.expires_at) > Date.parse(value.server_time)
          && Date.parse(value.access.expires_at) - Date.parse(value.server_time) <= 60000, 'CURRENT_BOUNDED_SERVER_LEASE_REQUIRED')
        const joined = settings as { revision: number; event: { event_id: string; revision: number; title: string } }
        assert(joined?.event.event_id === config.eventId && joined.revision === joined.event.revision, 'GENUINE_AUTHORIZED_EVENT_SETTINGS_REQUIRED')
        await expect(page.getByRole('heading', { name: joined.event.title, exact: true })).toBeVisible()
        report.checks.push('operator_actual_lease_and_settings_join_mount_default_admin')
      } else {
        await expect(page.getByRole('heading', { name: 'Administration unavailable', exact: true })).toBeVisible()
        await processing; assert(!invalidResponse, 'GENUINE_ADMIN_RESPONSE_VALIDATION_REQUIRED')
        assert(own && typeof own === 'object' && 'access' in own && own.access === null, 'GENUINE_MEMBER_NULL_ACCESS_REQUIRED')
        assert.equal(settings, undefined, 'MEMBER_MUST_NOT_REQUEST_OPERATOR_SETTINGS')
        await expect(page.getByRole('heading', { name: 'Event settings', exact: true })).toHaveCount(0)
        report.checks.push('member_actual_null_lease_fails_closed_without_private_settings')
      }
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'ADMIN_MOBILE_WIDTH_REQUIRED')
      assert.equal(pageErrors.length, 0, 'NO_PAGE_ERRORS_REQUIRED')
      const capture = `390-actual-${label}.png`
      await page.screenshot({ fullPage: true, path: resolve(out, capture) }); report.captures.push(capture)
      await processing; assert(!invalidResponse, 'GENUINE_ADMIN_RESPONSE_VALIDATION_REQUIRED')
      await context.close(); context = undefined
    }
    report.status = 'PASS'; report.stage = 'complete'
  } catch {
    if (report.stage !== 'inputs') report.status = 'FAIL'
    // Never serialize thrown protocol/network objects or private input values.
    failure = new Error('ACTUAL_ADMIN_DISCOVERY_UNQUALIFIED_AT_' + report.stage.toUpperCase())
  } finally {
    let cleanupFailed = false
    try { await context?.close() } catch { cleanupFailed = true }
    try { await browser?.close() } catch { cleanupFailed = true }
    if (cleanupFailed) { report.status = 'FAIL'; report.checks.push('CLEANUP_FAILED') }
    await writeFile(resolve(out, 'actual-admin.json'), JSON.stringify(report, null, 2) + '\n')
    if (cleanupFailed) failure ??= new Error('ACTUAL_ADMIN_CLEANUP_FAILED')
  }
  if (failure) throw failure
})
async function signIn(page: Page, config: GateConfig, account: GateConfig['operator']) {
  const response = await page.goto(config.origin + '/login')
  assert(response?.ok(), 'ACTUAL_PRODUCT_ORIGIN_REQUIRED')
  await page.getByLabel('Email', { exact: true }).fill(account.email)
  await page.getByLabel('Password', { exact: true }).fill(account.password)
  const login = page.waitForResponse(r => new URL(r.url()).pathname === '/api/auth/login' && r.request().method() === 'POST')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  const authenticated = await login
  assert(authenticated.ok(), 'ACTUAL_CANONICAL_LOGIN_REQUIRED')
  const identity: unknown = await authenticated.json()
  assert(identity && typeof identity === 'object' && 'userId' in identity && identity.userId === account.userId, 'CANONICAL_ACTOR_IDENTITY_REQUIRED')
  await page.waitForURL(config.origin + '/world')
}
