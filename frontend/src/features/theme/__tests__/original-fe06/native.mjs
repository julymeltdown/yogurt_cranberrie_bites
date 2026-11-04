import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'

const frontend = resolve(import.meta.dirname, '../../../../..'), repo = resolve(frontend, '..')
const evidence = process.env.MCR_FE06_OUTPUT
assert(evidence?.startsWith(resolve(repo, 'docs/mcr/evidence/original-fe06/') + '/'))
await mkdir(evidence, { recursive: false })
const git = (...args) => execFileSync('git', args, { cwd: repo }).toString().trim()
const input = { revision: 'test-only', reviewedBy: 'test-only', movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(process.env.TMPDIR, 'vite'), define: {
  'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
  'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)),
}, server: { host: '127.0.0.1', port: 0, hmr: false }, logLevel: 'warn' })
let browser, context, page, releaseParade = () => {}, stage = 'start'
const report = { result: 'FAIL', head: git('rev-parse', 'HEAD'), diffSha256: createHash('sha256').update(git('diff', 'HEAD')).digest('hex'),
  qualification: 'ONE390 actual main/default routes, canonical UI sign-in/Auth/HTTP and incumbent World/Phaser. Read-only retained sharing-safety controlled HTTP/C05 fixture and wholeData immutable selected-source art. Checks only new committed-palette projection and necessary contrast/lifetime integration. Existing persistence/rollback/geometry/avatar/native-width receipts reused. No live backend, provider, device, multiuser or full reference acceptance.', errors: [], states: [] }
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce' })
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  const state = () => page.evaluate(() => window.__SHARING_SAFETY__.state())
  const host = page.locator('[data-mcr-shell]')
  const palette = () => host.evaluate(shell => {
    const style = getComputedStyle(shell)
    return { theme: shell.getAttribute('data-mcr-theme'), ink: style.getPropertyValue('--mcr-ink').trim(), paper: style.getPropertyValue('--mcr-paper').trim(), scheme: style.colorScheme }
  })
  const shot = async name => { await page.screenshot({ path: resolve(evidence, `${name}-390.png`), fullPage: true }); report.states.push({ name, ...await palette() }) }
  const era = name => page.getByRole('button', { name: name[0] + name.slice(1).toLowerCase(), exact: true })
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/features/theme/__tests__/original-fe06/native.html`)
  stage = 'canonical UI sign-in'
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await expect(host).toBeVisible()
  await page.getByRole('link', { name: 'World', exact: true }).click()
  await expect(host).toHaveAttribute('data-mcr-theme', 'REVENGE')
  await expect.poll(async () => (await state()).canvases).toBe(1)
  assert.deepEqual(await palette(), { theme: 'REVENGE', ink: '#24191d', paper: '#fff5ef', scheme: 'light' })
  const before = await state()
  assert(before.requests.some(r => r.path === '/api/auth/enrollment/admission'))
  await shot('revenge')
  stage = 'pending palette retained'
  const gate = new Promise(resolve => { releaseParade = resolve })
  await page.route('**/world/themes/parade.png', async route => { await gate; await route.continue() })
  await era('PARADE').click()
  await expect(page.getByRole('status').filter({ hasText: 'Loading Parade' })).toBeVisible()
  await expect(host).toHaveAttribute('data-mcr-theme', 'REVENGE')
  await expect(era('REVENGE')).toHaveAttribute('aria-pressed', 'true')
  releaseParade()
  await expect(host).toHaveAttribute('data-mcr-theme', 'PARADE')
  assert.deepEqual(await palette(), { theme: 'PARADE', ink: '#f4edda', paper: '#1b1b20', scheme: 'dark' })
  await shot('parade')
  stage = 'failed artwork retains committed palette'
  await page.route('**/world/themes/danger.png', route => route.fulfill({ status: 503, body: 'TEST ONLY unavailable texture' }))
  await era('DANGER').click()
  await expect(page.getByRole('status').filter({ hasText: 'Parade is selected. The era could not be loaded or saved.' })).toBeVisible()
  await expect(host).toHaveAttribute('data-mcr-theme', 'PARADE')
  await expect(era('PARADE')).toHaveAttribute('aria-pressed', 'true')
  await shot('failed-danger-retains-parade')
  stage = 'retry commits DANGER'
  await page.unroute('**/world/themes/danger.png')
  await era('DANGER').click(); await expect(host).toHaveAttribute('data-mcr-theme', 'DANGER')
  assert.deepEqual(await palette(), { theme: 'DANGER', ink: '#152d3b', paper: '#f5fbf5', scheme: 'light' })
  await shot('danger')
  stage = 'warm panel exceptions and motion'
  await page.getByRole('button', { name: 'World tools', exact: true }).click()
  await page.getByRole('navigation', { name: 'World tools', exact: true }).getByRole('link', { name: 'Help', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'HOW TO PLAY', exact: true })).toBeVisible()
  const reduce = page.getByRole('button', { name: 'Reduce panel', exact: true })
  if (await reduce.isVisible()) await reduce.click()
  await era('PARADE').first().click(); await expect(host).toHaveAttribute('data-mcr-theme', 'PARADE')
  const panel = await page.locator('.shell-panel-body').evaluate(el => {
    const style = getComputedStyle(el)
    return { ink: style.getPropertyValue('--mcr-ink').trim(), paper: style.getPropertyValue('--mcr-paper').trim(), scheme: style.colorScheme, background: style.backgroundColor, color: style.color, transition: style.transitionDuration }
  })
  assert.equal(panel.ink, '#201d1d'); assert.equal(panel.paper, '#e9e1d4'); assert.equal(panel.scheme, 'light')
  assert.equal(panel.background, 'rgb(233, 225, 212)'); assert.equal(panel.transition, '0s')
  report.warmPanel = panel
  await shot('parade-warm-panel')
  const after = await state()
  assert.equal(after.sameHost, true); assert.equal(after.sameCanvas, true)
  assert.equal(after.peakCanvases, 1); assert.equal(after.peakSockets, 1); assert.equal(after.sockets, before.sockets)
  assert.equal(after.requests.filter(r => /world-tickets$/.test(r.path)).length, before.requests.filter(r => /world-tickets$/.test(r.path)).length)
  assert.deepEqual(after.requests.filter(r => r.method !== 'GET').map(r => [r.path, r.method]), before.requests.filter(r => r.method !== 'GET').map(r => [r.path, r.method]))
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  assert.deepEqual(report.errors, [])
  report.before = before; report.after = after; report.result = 'PASS'
} catch (error) { report.stage = stage; report.failure = String(error); if (page) await page.screenshot({ path: resolve(evidence, 'failure-390.png'), fullPage: true }).catch(() => {})
} finally {
  releaseParade(); await context?.close(); await browser?.close(); await server.close()
  report.closed = true
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
assert.equal(report.result, 'PASS', JSON.stringify(report))
