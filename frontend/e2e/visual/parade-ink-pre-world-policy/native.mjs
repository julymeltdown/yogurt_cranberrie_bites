import assert from 'node:assert/strict'
import { mkdir, rename, writeFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { policyFixtureRelease } from '../../../src/app/pre-world-policy/__tests__/fixtures.ts'

const repo = resolve(import.meta.dirname, '../../../..'), frontend = resolve(repo, 'frontend')
const output = process.env.MCR_PI12_PREWORLD_OUTPUT
assert(output?.startsWith(resolve(repo, 'docs/mcr/evidence/parade-ink-pre-world-policy/') + '/'))
await mkdir(output, { recursive: false })
const port = 5383
const server = await createServer({ root: frontend, configFile: resolve(import.meta.dirname, 'vite.config.ts'), configLoader: 'runner', cacheDir: resolve(process.env.TMPDIR, 'vite'),
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
    'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify(policyFixtureRelease) },
  server: { host: '127.0.0.1', port, strictPort: true, hmr: false }, logLevel: 'warn' })
const unavailable = 'Event availability could not be confirmed'
const scenarios = [
  ['policy-pending', { kind: 'NORMAL', delay: 1_500, bootstrapStatus: 403 }, 'Checking event availability'],
  ['bootstrap-pending', { kind: 'NORMAL', bootstrapDelay: 1_500, bootstrapStatus: 403 }, 'Opening your event'],
  ['maintenance', { kind: 'MAINTENANCE' }, 'World entry is paused'],
  ['minimum-version', { kind: 'CLIENT_VERSION_REQUIRED' }, 'An app update is required'],
  ['unavailable', { kind: 'HTTP503' }, unavailable],
  ['not-found', { kind: 'HTTP404' }, 'Event not found'],
  ['session-boundary', { kind: 'NORMAL', bootstrapStatus: 401 }, 'Sign in to continue'],
  ['denied', { kind: 'NORMAL', bootstrapStatus: 403 }, 'Access denied'],
  ['gone', { kind: 'NORMAL', bootstrapStatus: 410 }, 'This event is no longer available'],
  ['throttled', { kind: 'NORMAL', bootstrapStatus: 429 }, 'Please wait before trying again'],
]
const report = { result: 'FAIL', captures: [], completedCases: [], errors: [], interactions: [], startupDiagnostics: [], privatePort: port,
  foundation: 'Exact readonly final PI01 source7f9d1095 eighteen postimages, owner-qualified/published canonical651159ec; this leaf controlled validation is separate from combinedmaster/live/device acceptance',
  limits: 'PI12 pre-World S02/S03 subset only; actual main/default Auth/ports and controlled HTTP/C03/C05 fixtures. No live server/catalog/device, in-World overlays, general route-error or wholePI12 acceptance.' }
const retainedPNGCount = Number(process.env.MCR_PI12_PREWORLD_RETAINED_PNG_COUNT)
const retainedPNGBytes = Number(process.env.MCR_PI12_PREWORLD_RETAINED_PNG_BYTES)
const familyPNGCap = Number(process.env.MCR_PI12_PREWORLD_PNG_CAP)
assert(Number.isInteger(retainedPNGCount) && Number.isInteger(retainedPNGBytes) && [40, 48].includes(familyPNGCap))
let browser, context, page, stage = 'start', pngCount = 0, pngBytes = 0
async function checkpoint() {
  const temporary = resolve(output, 'report.partial.json.tmp')
  await writeFile(temporary, JSON.stringify({ ...report, stage, pngCount, pngBytes, checkpointAtUTC: new Date().toISOString() }, null, 2) + '\n')
  await rename(temporary, resolve(output, 'report.partial.json'))
}
async function savePNG(file) {
  const bytes = await page.screenshot({ fullPage: true })
  assert(retainedPNGCount + pngCount + 1 <= familyPNGCap && retainedPNGBytes + pngBytes + bytes.byteLength <= 16 * 1024**2, 'Finite family PNG upper')
  await writeFile(resolve(output, file), bytes)
  pngCount++; pngBytes += bytes.byteLength
}
const state = () => page.evaluate(() => window.__PRE_WORLD_POLICY__.state())
async function keyboardFocus(control) {
  for (let step = 0; step < 8; step++) {
    await page.keyboard.press('Tab')
    if (await control.evaluate(el => document.activeElement === el)) return
  }
  assert.fail('The existing action must be reachable through native Tab navigation')
}
async function open(viewport, scenario) {
  await context?.close()
  context = await browser.newContext({ viewport, isMobile: true, hasTouch: true, reducedMotion: 'reduce' })
  await context.addInitScript(value => { window.__MCR_POLICY_CASE__ = value }, scenario)
  page = await context.newPage()
  page.on('pageerror', error => report.errors.push(error.message))
  page.on('requestfailed', request => {
    if (report.startupDiagnostics.length < 20) report.startupDiagnostics.push({ path: new URL(request.url()).pathname.slice(0, 180), failure: request.failure()?.errorText })
  })
  page.on('console', message => {
    if (message.type() === 'error' && report.startupDiagnostics.length < 20) report.startupDiagnostics.push({ console: message.text().slice(0, 512) })
  })
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/e2e/visual/parade-ink-pre-world-policy/native.html`)
  // The fixture awaits nested dynamic imports; document load can precede them.
  await page.waitForFunction(() => Boolean(window.__PRE_WORLD_POLICY__), null, { timeout: 15_000 })
  await expect(page.getByRole('button', { name: 'Sign in with email', exact: true })).toBeVisible()
  await page.evaluate(async () => { await document.fonts.ready })
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--pi-contract').trim()), '"pi01-photo2-v1"', 'Actual default entry must consume PI01; no injected replacement styles')
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
}
async function capture(name, viewport, title) {
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible()
  const frame = page.locator('.pi-pre-world')
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  const snapshot = await state()
  assert.equal(snapshot.retained.canvases, 0); assert.equal(snapshot.retained.sockets, 0)
  for (const request of snapshot.reads) { assert.equal(request.method, 'GET'); assert.equal(request.authorized, true); assert.equal(request.cache, 'no-store') }
  let regions
  if (name === 'session-boundary') {
    // Canonical Auth revokes scope on C03 401. AccessNotice belongs to owner83;
    // this is boundary compatibility evidence, not this leaf's visual acceptance.
    await expect(frame).toHaveCount(0)
    const signIn = page.getByRole('link', { name: 'Sign in', exact: true })
    await expect(signIn).toBeVisible()
    const href = await signIn.getAttribute('href')
    const target = new URL(href, page.url())
    assert.equal(target.pathname, '/login'); assert.equal(target.searchParams.get('returnTo'), '/meet/people')
    assert.equal(new URL(page.url()).pathname, '/meet/people')
    regions = { externalBoundary: 'Unchanged canonical AccessNotice; generic presentation owner83', preservedSignInHref: href }
    report.interactions.push({ viewport, sessionBoundary: 'Canonical Auth revokes scope; sign-in preserves destination; no premature World' })
  } else {
  await expect(frame).toHaveAttribute('aria-labelledby', /.+/)
  regions = await frame.evaluate(root => {
    const measured = selector => {
      const el = root.querySelector(selector), style = getComputedStyle(el), box = el.getBoundingClientRect()
      return { box: { x: box.x, y: box.y, width: box.width, height: box.height }, color: style.color, background: style.backgroundColor,
        font: style.fontFamily, size: style.fontSize, leading: style.lineHeight, border: style.borderTopWidth, padding: style.padding }
    }
    return { frame: measured('.pi-pre-world__window'), title: measured('.pi-pre-world__title'), heading: measured('h1'),
      body: measured('.pi-pre-world__body'), actions: measured('.pi-pre-world__actions'),
      controls: [...root.querySelectorAll('a,button')].map(el => { const box = el.getBoundingClientRect(); return { name: el.textContent, width: box.width, height: box.height } }) }
  })
  assert.equal(regions.frame.border, '2px'); assert.equal(regions.title.background, 'rgb(23, 23, 23)')
  assert.equal(regions.title.color, 'rgb(255, 255, 255)'); assert(regions.title.box.height >= 48)
  assert.equal(regions.heading.size, '26px'); assert.equal(regions.heading.leading, '32px')
  assert.match(regions.heading.font, /Parade Ink Display/)
  for (const control of regions.controls) { assert(control.width >= 44, control.name); assert(control.height >= 44, control.name) }
  }
  const capturedAtUTC = new Date().toISOString(), file = `${name}-${viewport.width}x${viewport.height}.png`
  await savePNG(file)
  const capturedAtKST = new Intl.DateTimeFormat('sv-SE', { timeZone: 'Asia/Seoul', dateStyle: 'short', timeStyle: 'long' }).format(new Date(capturedAtUTC))
  report.captures.push({ name, viewport, title, file, capturedAtUTC, capturedAtKST, regions, snapshot })
  await checkpoint()
}
try {
  await server.listen()
  const crashpad = resolve(process.env.MCR_PI12_PREWORLD_TEMP, 'crashpad')
  await mkdir(crashpad)
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    env: { ...process.env, BREAKPAD_DUMP_LOCATION: crashpad } })
  report.crashpadNamespace = crashpad
  report.crashpadProcesses = execFileSync('/bin/ps', ['-axo', 'pid=,ppid=,pgid=,lstart=,command='], { encoding: 'utf8', maxBuffer: 4 * 1024**2 }).split('\n')
    .filter(line => line.includes('/Helpers/chrome_crashpad_handler') && line.includes(crashpad))
  assert(report.crashpadProcesses.length > 0, 'Actual Chrome crash handler must use the charged private namespace')
  await checkpoint()
  for (const viewport of [{ width: 360, height: 844 }, { width: 390, height: 844 }, { width: 430, height: 844 }, { width: 390, height: 380 }]) {
    const cases = viewport.height === 380 ? scenarios.filter(([name]) => ['minimum-version', 'throttled'].includes(name)) : scenarios
    for (const [name, scenario, title] of cases) {
      stage = `${name}/${viewport.width}x${viewport.height}`
      await open(viewport, scenario); await capture(name, viewport, title)
      if (name === 'minimum-version') {
        const reload = page.getByRole('button', { name: 'Reload app', exact: true })
        await keyboardFocus(reload); await expect(reload).toBeFocused()
        assert.notEqual(await reload.evaluate(el => getComputedStyle(el).outlineStyle), 'none')
        if (viewport.width === 390) await capture('minimum-version-keyboard-focus', viewport, title)
        await reload.press('Enter')
        await expect(page.getByRole('heading', { name: 'Sign in to continue', exact: true })).toBeVisible()
        assert.equal(new URL(page.url()).pathname, '/meet/people')
        assert.equal(await page.locator('canvas').count(), 0)
        report.interactions.push({ viewport, keyboardReload: 'preserved destination, canonical anonymous gate, no World' })
      }
      if (name === 'unavailable') {
        const before = await state()
        await page.evaluate(() => window.__PRE_WORLD_POLICY__.mode({ kind: 'MAINTENANCE' }))
        const retry = page.getByRole('button', { name: 'Check again', exact: true })
        await keyboardFocus(retry); await retry.press('Enter')
        await expect(page.getByRole('heading', { name: 'World entry is paused', exact: true })).toBeVisible()
        const after = await state()
        assert.equal(after.reads.length, before.reads.length + 1); assert.equal(after.bootstrapDispatches, 0)
        report.interactions.push({ viewport, manualKeyboardRetry: 'exactly one new policy read, no bootstrap' })
      }
      if (name === 'throttled' && viewport.height === 380) {
        const retry = page.getByRole('button', { name: 'Try again', exact: true })
        await keyboardFocus(retry); await expect(retry).toBeInViewport()
        report.interactions.push({ viewport, shortHeightAction: 'keyboard focus and scroll reach actual retry' })
      }
      report.completedCases.push({ name, viewport, completedAtUTC: new Date().toISOString() })
      await checkpoint()
    }
    if (viewport.height === 844) {
      stage = `expiry/${viewport.width}`
      await open(viewport, { kind: 'MAINTENANCE', ttl: 750 })
      await expect(page.getByRole('heading', { name: 'World entry is paused', exact: true })).toBeVisible()
      const before = await state()
      await expect(page.getByRole('heading', { name: unavailable, exact: true })).toBeVisible()
      assert.equal((await state()).reads.length, before.reads.length)
      await capture('expired-policy', viewport, unavailable)
      report.completedCases.push({ name: 'expired-policy', viewport, completedAtUTC: new Date().toISOString() })
      await checkpoint()
    }
  }
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.stage = stage; report.failure = String(error); report.failureState = page ? await state().catch(() => null) : null
  if (page) await savePNG('failure.png').catch(() => {})
} finally {
  await context?.close(); await browser?.close(); await server.close()
  report.closed = true; report.pngCount = pngCount; report.pngBytes = pngBytes; report.familyPNGCount = retainedPNGCount + pngCount; report.familyPNGBytes = retainedPNGBytes + pngBytes; report.serverClosed = !server.httpServer?.listening
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
assert.equal(report.result, 'PASS', JSON.stringify({ ...report, captures: report.captures.map(({ name, viewport }) => ({ name, viewport })) }))
