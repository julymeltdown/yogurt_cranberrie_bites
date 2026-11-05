import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { policyFixtureRelease } from './fixtures.ts'

const frontend = resolve(import.meta.dirname, '../../../..'), repo = resolve(frontend, '..')
const output = process.env.MCR_PRE_WORLD_POLICY_OUTPUT
assert(output?.startsWith(resolve(repo, 'docs/mcr/evidence/pre-world-service-policy/') + '/'))
await mkdir(output, { recursive: false })
const input = { revision: 'test-only', reviewedBy: 'test-only', movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(process.env.TMPDIR, 'vite'),
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)),
    // TEST ONLY deterministic recognized catalog pair through the actual Root parser.
    // This does not claim a real publisher manifest/catalog or live service-policy authority.
    'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify(policyFixtureRelease) },
  server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false }, logLevel: 'warn' })
let browser, context, page, stage = 'start'
const report = { result: 'FAIL', cases: [], errors: [],
  limits: 'One390 viewport, serial contexts, actual main/canonical UI Auth/admission/HTTP/C03/Phaser with controlled C05 and selected-art fixtures. TEST ONLY known catalog pair; no actual publisher/catalog, native service HTTP, multiuser, device, load or full visual-reference acceptance.' }
const state = () => page.evaluate(() => window.__PRE_WORLD_POLICY__.state())
const unavailable = () => page.getByRole('heading', { name: 'Event availability could not be confirmed', exact: true })
async function open(scenario) {
  await context?.close()
  context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true })
  await context.addInitScript(value => { window.__MCR_POLICY_CASE__ = value }, scenario)
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/pre-world-policy/__tests__/native.html`)
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
}
async function capture(name) {
  const snapshot = await state()
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  for (const read of snapshot.reads) { assert.equal(read.method, 'GET'); assert(read.authorized); assert.equal(read.cache, 'no-store') }
  await page.screenshot({ path: resolve(output, name + '-390.png'), fullPage: true })
  report.cases.push({ name, snapshot })
  return snapshot
}
async function noWorld() {
  const snapshot = await state()
  assert.equal(snapshot.retained.canvases, 0); assert.equal(snapshot.retained.sockets, 0)
  return snapshot
}
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  stage = 'trusted maintenance expiry and manual version retry'
  await open({ kind: 'MAINTENANCE', ttl: 1_500 })
  await expect(page.getByRole('heading', { name: 'World entry is paused', exact: true })).toBeVisible()
  assert.equal((await noWorld()).bootstrapDispatches, 0)
  await capture('maintenance')
  await expect(unavailable()).toBeVisible()
  assert.equal((await state()).reads.length, 1)
  await page.evaluate(() => window.__PRE_WORLD_POLICY__.mode({ kind: 'CLIENT_VERSION_REQUIRED' }))
  await page.getByRole('button', { name: 'Check again', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'An app update is required', exact: true })).toBeVisible()
  assert.equal((await noWorld()).bootstrapDispatches, 0)
  await capture('minimum-client')
  const reload = page.getByRole('button', { name: 'Reload app', exact: true })
  await reload.focus(); await reload.press('Enter')
  // Full reload drops the in-memory actor and preserves the current destination.
  await expect(page.getByRole('heading', { name: 'Sign in to continue', exact: true })).toBeVisible()
  assert.equal(new URL(page.url()).pathname, '/meet/people')
  assert.equal(await page.locator('canvas').count(), 0)

  stage = 'ordinary503 unavailable then fresh NORMAL World consumption'
  await open({ kind: 'HTTP503' }); await expect(unavailable()).toBeVisible()
  assert.equal((await noWorld()).bootstrapDispatches, 0)
  assert.equal(await page.getByRole('heading', { name: 'World entry is paused', exact: true }).count(), 0)
  await capture('ordinary-503')
  await page.evaluate(() => window.__PRE_WORLD_POLICY__.mode({ kind: 'NORMAL', ttl: 3_000 }))
  await page.getByRole('button', { name: 'Check again', exact: true }).click()
  await expect(page.locator('[data-mcr-shell]')).toBeVisible()
  // The retained mobile destination opens a full panel and pauses covered World.
  await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
  await expect.poll(async () => (await state()).retained.canvases).toBe(1)
  await expect.poll(async () => (await state()).retained.activeSockets).toBe(1)
  const before = await state()
  await new Promise(resolveDelay => setTimeout(resolveDelay, 3_100))
  await page.evaluate(() => window.dispatchEvent(new Event('focus')))
  const after = await capture('consumed-normal')
  assert.equal(after.retained.sameHost, true); assert.equal(after.retained.sameCanvas, true)
  assert.equal(after.retained.peakCanvases, 1); assert.equal(after.retained.peakSockets, 1)
  assert.equal(after.retained.sockets, before.retained.sockets); assert.equal(after.bootstrapDispatches, before.bootstrapDispatches)
  assert.equal(after.reads.length, before.reads.length)

  stage = 'stale-on-arrival policy cannot dispatch bootstrap'
  await open({ kind: 'NORMAL', ttl: 50, delay: 200 }); await expect(unavailable()).toBeVisible()
  assert.equal((await noWorld()).bootstrapDispatches, 0)
  await capture('late-policy')

  stage = 'public event404 keeps the existing safe destination category'
  await open({ kind: 'HTTP404' })
  await expect(page.getByRole('heading', { name: 'Event not found', exact: true })).toBeVisible()
  assert.equal((await noWorld()).bootstrapDispatches, 0)
  await capture('event-not-found')

  stage = 'policy expiry cancels in-flight C03 without a late World'
  await open({ kind: 'NORMAL', ttl: 250, bootstrapDelay: 700 }); await expect(unavailable()).toBeVisible()
  await new Promise(resolveDelay => setTimeout(resolveDelay, 800))
  assert.equal((await noWorld()).bootstrapDispatches, 1)
  await capture('expired-bootstrap')

  for (const [status, title] of [[401, 'Sign in to continue'], [403, 'Access denied']]) {
    stage = `fresh policy preserves actual C03 HTTP${status}`
    await open({ kind: 'NORMAL', bootstrapStatus: status })
    await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible()
    assert.equal((await noWorld()).bootstrapDispatches, 1)
    await capture(`bootstrap-${status}`)
  }
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.stage = stage; report.failure = String(error)
  report.failureState = await state().catch(() => null)
  await page?.screenshot({ path: resolve(output, 'failure-390.png'), fullPage: true }).catch(() => {})
} finally {
  await context?.close(); await browser?.close(); await server.close()
  report.closed = true
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
assert.equal(report.result, 'PASS', JSON.stringify(report))
