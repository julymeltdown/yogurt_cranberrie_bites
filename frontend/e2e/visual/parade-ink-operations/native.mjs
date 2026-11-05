// Actual main/Auth/router/ports under controlled HTTP. Requires a fresh finite browser ACCEPT.
import assert from 'node:assert/strict'
import * as fixtures from './fixtures.mjs'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, readdir, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { repo, frontend, deps, aliases, readset, requireDependency, pinnedComposition } from './config.mjs'

assert.equal(process.env.PI12_NATIVE_ACCEPTED, '1')
const out = process.env.PI12_NATIVE_OUT
assert(out)
const { createServer } = requireDependency('vite')
const react = requireDependency('@vitejs/plugin-react').default
const { chromium, expect } = requireDependency('@playwright/test')
const wire = JSON.parse(await readFile(resolve(frontend, 'src/app/operations-composition/__tests__/wire.json')))
const report = { status: 'FAIL', qualification: 'TEST_ONLY_CONTROLLED_HTTP_ACTUAL_MAIN',
  source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(),
  canonicalSixPages: [], sourcePinnedMemberContent: [], errors: [], captures: [], measurements: [],
  canonicalMemberContentMount: 'PUBLISHED_SEPARATELY_cf7bb137; own_native_uses_qualified_immutable_postimages', physicalDevices: 'NOT_RUN', realProducer: 'NOT_RUN', twoRealClients: 'NOT_RUN',
  reference: { sha256: '9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27', crop: [24, 60, 395, 787], normalizedWidth: 390,
    limitation: 'Operator pages are unpictured; shared frame/control anatomy only. Outer administration composition remains separately owned.' },
  sourceQualifiedComposition: readset.fe37SourcePassedReadonlyPostimages, focusedStates: [] }
await mkdir(out, { recursive: true })
let server, browser, context, page
const port = 5376
const stage = process.env.PI12_NATIVE_STAGE ?? 'all'
assert(['all', 'canonical', 'moderation'].includes(stage))
const navigate = path => page.evaluate(path => { history.pushState(null, '', path); dispatchEvent(new PopStateEvent('popstate')) }, path)
async function capture(name) {
  await page.evaluate(() => window.scrollTo(0, 0))
  const file = `${name}.png`
  await page.screenshot({ path: resolve(out, file), fullPage: true })
  report.captures.push(file)
  const files = (await readdir(out)).filter(name => name.endsWith('.png'))
  const bytes = (await Promise.all(files.map(name => stat(resolve(out, name))))).reduce((sum, item) => sum + item.size, 0)
  assert(bytes <= 8 * 1024 ** 2, 'All captures and comparisons share the 8MiB cap')
}
async function measure(view) {
  const surface = page.locator(`[data-operations-view="${view}"]`)
  await expect(surface).toBeVisible()
  const result = await surface.evaluate(root => {
    const body = getComputedStyle(root), bounds = root.getBoundingClientRect()
    const controls = [...root.querySelectorAll('button,input,select,textarea')].filter(e => e.getClientRects().length).map(e => {
      const box = e.getBoundingClientRect(), style = getComputedStyle(e)
      return { label: e.getAttribute('aria-label') || e.textContent?.slice(0, 80), width: box.width, height: box.height, font: style.fontFamily, background: style.backgroundColor }
    })
    const heading = root.querySelector('h1'), style = heading && getComputedStyle(heading)
    return { width: bounds.width, font: body.fontFamily, size: body.fontSize, leading: body.lineHeight, background: body.backgroundColor,
      heading: style && { font: style.fontFamily, size: style.fontSize, leading: style.lineHeight }, controls,
      viewportOverflow: document.documentElement.scrollWidth > innerWidth,
      nestedMainCount: document.querySelectorAll('main main').length }
  })
  assert.equal(result.nestedMainCount, 0, `Ready presentation landmarks: ${view}`)
  assert.equal(result.viewportOverflow, false, `Viewport overflow: ${view}`)
  assert(result.font.includes('Parade Ink Text'), `Canonical body font: ${view}`)
  assert.equal(result.size, '18px'); assert.equal(result.leading, '22px')
  assert.equal(result.background, 'rgb(249, 249, 248)')
  for (const box of result.controls) assert(box.width >= 44 && box.height >= 44, JSON.stringify({ view, box }))
  assert.equal(await page.getByText('PRIVATE_PROFILE_BIO', { exact: true }).count(), 0)
  assert.equal(await page.getByText('PRIVATE_BACKEND_DIAGNOSTIC', { exact: true }).count(), 0)
  report.measurements.push({ view, viewport: page.viewportSize(), ...result })
}
async function compareReference(view) {
  const reference = await readFile(resolve(repo, 'docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg'))
  assert.equal(createHash('sha256').update(reference).digest('hex'), report.reference.sha256)
  const bounds = await page.locator(`[data-operations-view="${view}"]`).boundingBox()
  const actual = await page.screenshot({ fullPage: true })
  const images = await page.evaluate(async ({ actual, reference, bounds }) => {
    const load = src => new Promise((done, fail) => { const img = new Image(); img.onload = () => done(img); img.onerror = fail; img.src = src })
    const [a, r] = await Promise.all([load(actual), load(reference)])
    const width = 390, height = Math.round(787 * width / 395)
    const side = document.createElement('canvas'); side.width = 792; side.height = height + 30
    const ctx = side.getContext('2d'); ctx.fillStyle = '#f9f9f8'; ctx.fillRect(0, 0, side.width, side.height)
    ctx.fillStyle = '#171717'; ctx.font = '14px Arial'; ctx.fillText('Photo 2 shared frame', 6, 20); ctx.fillText('Actual unpictured operator surface', 408, 20)
    ctx.drawImage(r, 24, 60, 395, 787, 0, 30, width, height)
    ctx.drawImage(a, bounds.x, bounds.y, bounds.width, Math.min(bounds.height, height * bounds.width / width), 402, 30, width, Math.min(bounds.height * width / bounds.width, height))
    const overlay = document.createElement('canvas'); overlay.width = width; overlay.height = height
    const blend = overlay.getContext('2d'); blend.drawImage(r, 24, 60, 395, 787, 0, 0, width, height); blend.globalAlpha = .5
    blend.drawImage(a, bounds.x, bounds.y, bounds.width, Math.min(bounds.height, height * bounds.width / width), 0, 0, width, Math.min(bounds.height * width / bounds.width, height))
    return { side: side.toDataURL(), overlay: overlay.toDataURL() }
  }, { actual: `data:image/png;base64,${actual.toString('base64')}`, reference: `data:image/jpeg;base64,${reference.toString('base64')}`, bounds })
  for (const [kind, data] of Object.entries(images)) await writeFile(resolve(out, `operator-390-reference-${kind}.png`), Buffer.from(data.split(',')[1], 'base64'))
  report.reference.developerAssessment = 'PENDING_IMAGE_INSPECTION'
}
async function startServer(pinned) {
  if (server) await server.close()
  server = await createServer({ root: frontend, configFile: false, logLevel: 'error',
    cacheDir: resolve(out, `vite-cache-${pinned ? 'fe37' : 'canonical'}`),
    plugins: [...(pinned ? [pinnedComposition()] : []), react()], resolve: { alias: aliases },
    define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(wire.event) },
    server: { host: '127.0.0.1', port, strictPort: true, hmr: false, fs: { allow: [repo, resolve(deps, '..')] } },
    optimizeDeps: { entries: ['index.html'], include: ['react', 'react-dom/client', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-router-dom', '@tanstack/react-query', 'zod', 'ky', 'phaser', 'react-hook-form', '@hookform/resolvers/zod'] } })
  await server.listen()
}
async function start(width, height, pinned) {
  const module = await server.ssrLoadModule(pinned ? '/src/app/operations-composition/__tests__/member-content-production/server.ts' : '/src/app/operations-composition/__tests__/server.ts')
  const controlled = pinned ? module.createMemberContentServer() : module.createControlledServer()
  controlled.state.role = pinned ? 'MODERATOR' : 'OPERATOR'
  context = await browser.newContext({ viewport: { width, height }, isMobile: true, hasTouch: true, deviceScaleFactor: 1, reducedMotion: 'reduce' })
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  await page.route(url => url.pathname.startsWith('/api/'), async route => {
    const r = route.request(), request = new Request(r.url(), { method: r.method(), headers: r.headers(), ...(r.postData() ? { body: r.postData() } : {}) })
    const path = new URL(request.url).pathname
    try {
      let response
      if (!pinned && request.method === 'GET' && request.headers.get('Authorization') && path.endsWith('/admin/places')) response = Response.json({ items: [fixtures.place()], next_cursor: null })
      else if (!pinned && request.method === 'GET' && request.headers.get('Authorization') && path.endsWith('/admin/notices')) response = Response.json({ items: [{ ...fixtures.notice(), event_id: wire.event }], next_cursor: null })
      else if (!pinned && request.method === 'GET' && request.headers.get('Authorization') && path.endsWith('/admin/audit')) response = Response.json({ items: [{ audit_id: fixtures.uid(50), event_id: wire.event, actor_id: wire.actor, operation_id: 'setConcertSettings', target_id: wire.event, outcome: 'SUCCEEDED', receipt_id: fixtures.uid(80), created_at: fixtures.time(-1000) }], next_cursor: null })
      else response = await controlled.handle(request)
      await route.fulfill({ status: response.status, headers: Object.fromEntries(response.headers), body: await response.text() })
    } catch { await route.abort('failed') }
  })
  await page.goto(`http://127.0.0.1:${port}/login`)
  await page.getByLabel('Email', { exact: true }).fill('operator@example.invalid')
  await page.getByLabel('Password', { exact: true }).fill('TEST_ONLY_password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await page.waitForURL('**/world')
  return { controlled, memberIds: module.memberIds }
}
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--disable-background-networking', '--disable-component-update'] })
  report.browser = browser.version()
  if (stage !== 'moderation') {
  await startServer(false)
  for (const width of [360, 390, 430]) {
    const { controlled } = await start(width, 844, false)
    for (const [path, view] of [['/admin', 'control'], ['/admin/event', 'event'], ['/admin/places', 'places'], ['/admin/mural', 'wall'], ['/admin/audit', 'audit'], ['/admin/archive', 'closure']]) {
      await navigate(path)
      if (view === 'control' || view === 'event') await expect(page.getByRole('heading', { name: 'SYNTHETIC operations event', exact: true })).toBeVisible()
      if (view === 'places') await expect(page.getByRole('button', { name: 'Review SYNTHETIC landmark', exact: true })).toBeVisible()
      if (view === 'wall') await expect(page.getByRole('button', { name: 'Review wall action', exact: true })).toBeVisible()
      if (view === 'audit') await expect(page.getByRole('button', { name: 'View audit entry', exact: true })).toBeVisible()
      await measure(view); await page.evaluate(() => document.fonts.ready)
      await capture(`${width}-${view}`); report.canonicalSixPages.push({ path, width, result: 'PASS' })
      if (width === 390 && view === 'control') await compareReference(view)
      if (width === 390 && view === 'audit') {
        await page.getByRole('button', { name: 'View audit entry', exact: true }).click()
        await expect(page.getByRole('heading', { name: 'Audit entry details', exact: true })).toBeFocused()
        await capture('390-audit-details'); report.focusedStates.push('audit-details-focus')
      }
      if (width === 390 && view === 'event') {
        controlled.state.settingsStatus = 503
        await page.getByRole('button', { name: 'Refresh event information', exact: true }).click()
        await expect(page.getByText('The owning service is unavailable. The outcome has not been assumed.', { exact: true })).toBeVisible()
        await capture('390-event-unavailable-read'); report.focusedStates.push('event-unavailable-read')
        controlled.state.settingsStatus = 200
      }
    }
    await context.close(); context = null
  }
  const short = await start(390, 420, false)
  await navigate('/admin/mural'); await measure('wall')
  await page.getByLabel('Reason', { exact: true }).fill('Synthetic operator review with an intentionally long factual explanation. '.repeat(4))
  await page.getByRole('button', { name: 'Review wall action', exact: true }).click()
  await expect(page.getByRole('group', { name: 'Confirm wall action' })).toBeFocused()
  await capture('390x420-wall-confirm')
  await page.getByRole('button', { name: 'Cancel wall action', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Review wall action', exact: true })).toBeVisible()
  short.controlled.state.lost = true
  await page.getByLabel('Reason', { exact: true }).fill('Synthetic uncertain pause')
  await page.getByRole('button', { name: 'Review wall action', exact: true }).click()
  await page.getByRole('button', { name: 'Confirm wall action', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Check original wall command', exact: true })).toBeVisible()
  await measure('wall'); await capture('390x420-wall-unknown'); report.focusedStates.push('wall-unknown-original-command')
  await page.getByRole('button', { name: 'Check original wall command', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Review wall action', exact: true })).toBeVisible()
  short.controlled.state.wall.state = 'FROZEN'; short.controlled.state.wall.revision++
  await page.getByRole('button', { name: 'Refresh wall controls', exact: true }).click()
  await expect(page.getByText('The final wall is frozen. Editing, resume and restore are unavailable.', { exact: true })).toBeVisible()
  await measure('wall'); await capture('390x420-wall-frozen'); report.focusedStates.push('wall-frozen')
  await navigate('/admin/archive'); await measure('closure')
  await page.getByLabel('Existing closure job ID', { exact: true }).fill(wire.job)
  await page.getByRole('button', { name: 'Load existing closure job', exact: true }).click()
  await expect(page.getByLabel('Closure job state')).toContainText('PARTIAL_FAILURE')
  await capture('390x420-closure-partial')
  await context.close(); context = null
  }
  if (stage !== 'canonical') {
  await startServer(true)
  for (const width of [360, 390, 430]) {
    const { controlled, memberIds } = await start(width, 844, true)
    for (const [path, view, action] of [[`/admin/users/${memberIds[0]}`, 'members', 'SUSPEND'], ['/admin/content', 'content', 'HIDE']]) {
      await navigate(path); await expect(page.getByRole('button', { name: /^Review / }).first()).toBeVisible(); await measure(view)
      await page.getByRole('button', { name: /^Review / }).first().click()
      await expect(page.locator('#moderation-record')).toBeFocused()
      await page.getByLabel('Action', { exact: true }).selectOption(action)
      await page.getByLabel('Reason for this action', { exact: true }).fill('Synthetic factual presentation review')
      await page.getByRole('button', { name: 'Review action', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Confirm action', exact: true })).toBeFocused()
      await measure(view); await capture(`${width}-${view}-confirm`)
      await page.getByRole('button', { name: 'Cancel confirmation', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeFocused()
      assert.equal(controlled.requests.filter(r => r.method === 'POST' && new URL(r.url).pathname.includes('/moderation')).length, 0)
      report.sourcePinnedMemberContent.push({ path, width, result: 'PASS', canonical: false })
    }
    if (width === 390) {
      controlled.moderation.mode = 'conflict'
      await page.getByRole('button', { name: 'Review action', exact: true }).click()
      await page.getByRole('button', { name: 'Confirm action', exact: true }).click()
      await expect(page.getByRole('alert')).toContainText('The record changed.')
      await capture('390-content-conflict'); report.focusedStates.push('content-conflict')
      await navigate(`/admin/users/${memberIds[0]}`)
      await expect(page.getByRole('button', { name: /^Review / }).first()).toBeVisible()
      await page.getByRole('button', { name: /^Review / }).first().click()
      await expect(page.locator('#moderation-record')).toBeFocused()
      await page.getByLabel('Action', { exact: true }).selectOption('SUSPEND')
      await page.getByLabel('Reason for this action', { exact: true }).fill('Synthetic factual lost-response review')
      await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeEnabled()
      controlled.moderation.mode = 'lost'
      await page.getByRole('button', { name: 'Review action', exact: true }).click()
      await page.getByRole('button', { name: 'Confirm action', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeVisible()
      await capture('390-member-unknown'); report.focusedStates.push('member-unknown')
      controlled.state.role = null
      await page.getByRole('button', { name: 'Check current access', exact: true }).click()
      await expect(page.locator('[data-operations-view]')).toHaveCount(0)
      await expect(page.getByRole('heading', { name: 'Review tools unavailable', exact: true })).toBeVisible()
      await capture('390-authority-withdrawn'); report.focusedStates.push('authority-withdrawn-protected-root')
    }
    await context.close(); context = null
  }
  }
  assert.equal(report.errors.length, 0, 'Actual main page errors remain unresolved')
  report.status = 'PASS'
} catch (error) {
  report.errors.push(error.stack || String(error)); throw error
} finally {
  await context?.close(); await browser?.close(); await server?.close()
  report.cleanup = { browser: 'CLOSED', vite: 'CLOSED' }
  await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2))
}
