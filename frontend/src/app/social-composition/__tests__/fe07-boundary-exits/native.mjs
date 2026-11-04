import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, statfs, writeFile } from 'node:fs/promises'
import { createServer as createNetServer } from 'node:net'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

const here = import.meta.dirname, frontend = resolve(here, '../../../../..'), repo = resolve(frontend, '..')
const run = process.env.BOUNDARY_RUN || 'corrected'
assert(/^[a-z0-9-]+$/.test(run))
const out = resolve(here, 'evidence', run)
const disk = await statfs(repo); assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, '8GiB disk floor')
await mkdir(out, { recursive: true })
const pins = {}
for (const file of ['src/app/social-composition/owner.ts', 'src/app/social-composition/navigation.tsx', 'src/app/social-composition/provider.tsx', 'src/app/routes.ts', 'src/app/router.tsx', 'src/app/WorldShell.tsx', 'src/app/AdminBoundary.tsx', 'src/app/social-composition/__tests__/fe07-boundary-exits/preview.tsx', 'src/app/social-composition/__tests__/fe07-boundary-exits/native.mjs', 'pnpm-lock.yaml']) {
  pins[file] = createHash('sha256').update(await readFile(resolve(frontend, file))).digest('hex')
}
const report = { candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(), pins, browser: null,
  qualification: 'Actual createAppRouter/browser history, canonical Auth, StrictMode, default production ports and real Phaser. Synthetic HTTP/C05/identity. Not live backend, physical devices, real server admin permission, movement/admission or whole FE07 acceptance.', cases: [] }
const reservation = createNetServer()
await new Promise(done => reservation.listen(0, '127.0.0.1', done))
const port = reservation.address().port
await new Promise(done => reservation.close(done))
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: `/tmp/mcr-fe07-boundary-${process.pid}`, logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
let browser
const state = page => page.evaluate(() => window.__FE07_BOUNDARIES__.state())
const navigate = (page, path) => page.evaluate(path => window.__FE07_BOUNDARIES__.navigate(path), path)
async function world(page) {
  await expect.poll(async () => (await state(page)).games).toBe(1)
  await expect.poll(async () => (await state(page)).phase).toBe('live')
  await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
}
try {
  await server.listen()
  const origin = server.resolvedUrls.local[0].replace(/\/$/, '')
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const width of process.env.BOUNDARY_WIDTH ? [Number(process.env.BOUNDARY_WIDTH)] : [360, 390, 430]) {
    for (const scenario of ['clean', 'restored-profile']) {
      const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, serviceWorkers: 'block' })
      const page = await context.newPage(), errors = [], result = { width, scenario, status: 'FAIL' }
      page.setDefaultTimeout(10000)
      page.on('pageerror', error => errors.push(error.message))
      try {
        await page.goto(origin + '/src/app/social-composition/__tests__/fe07-boundary-exits/preview.html?entry=/world&strict=1')
        await page.waitForFunction(() => window.__FE07_BOUNDARIES__)
        await world(page)
        if (scenario === 'restored-profile') {
          await navigate(page, '/me/profile')
          const bio = page.getByLabel('Bio (optional)')
          await expect(bio).toBeEnabled()
          const initial = await bio.inputValue()
          await bio.fill('Unsaved boundary fixture draft')
          await navigate(page, '/world'); await world(page)
          await navigate(page, '/me/profile')
          await expect(bio).toHaveValue('Unsaved boundary fixture draft')
          await bio.fill(initial)
          await navigate(page, '/world'); await world(page)
        }
        result.before = await state(page)
        assert.equal(result.before.games, 1); assert.equal(result.before.activeSockets, 1)
        await navigate(page, '/admin/reports?token=secret')
        await expect(page.getByRole('button', { name: 'Continue navigation', exact: true })).toBeDisabled()
        assert.equal((await state(page)).path, '/world')
        await page.getByRole('button', { name: 'Stay with current action', exact: true }).click()
        await navigate(page, '/legal/privacy')
        await expect.poll(async () => (await state(page)).path, { timeout: 3000 }).toBe('/legal/privacy')
        await expect.poll(async () => (await state(page)).games).toBe(0)
        await expect.poll(async () => (await state(page)).activeSockets).toBe(0)
        await expect(page.locator('.world-shell')).toHaveCount(0)
        result.publicExit = await state(page)
        await page.goBack(); await world(page)
        result.returned = await state(page)
        assert.equal(result.returned.knownGames, 2)
        assert.equal(result.returned.peakGames, 1); assert.equal(result.returned.peakSockets, 1)
        await navigate(page, '/admin/reports')
        await expect(page.getByRole('heading', { name: 'Access denied', exact: true })).toBeVisible()
        await expect.poll(async () => (await state(page)).games).toBe(0)
        await expect.poll(async () => (await state(page)).activeSockets).toBe(0)
        await expect(page.locator('.world-shell')).toHaveCount(0)
        result.adminExit = await state(page)
        assert.equal(result.adminExit.requests.filter(r => r.method !== 'GET' && !r.path.endsWith('/login') && !r.path.endsWith('/world-tickets')).length, 0, 'No implicit save, replay or mutation')
        assert.deepEqual(errors, [])
        result.status = 'PASS'
      } catch (error) { result.failure = error.message; result.observed = await state(page).catch(() => null); result.errors = errors }
      await writeFile(resolve(out, `${width}-${scenario}.aria.txt`), await page.locator('body').ariaSnapshot())
      await page.screenshot({ path: resolve(out, `${width}-${scenario}.png`), fullPage: true })
      await page.evaluate(() => window.__FE07_BOUNDARIES__?.dispose())
      result.disposed = await state(page)
      report.cases.push(result)
      console.log(`${result.status} ${width} ${scenario}${result.failure ? ': ' + result.failure.split('\n')[0] : ''}`)
      await context.close()
    }
  }
} finally {
  await browser?.close(); await server.close()
  for (const [file, pin] of Object.entries(pins)) assert.equal(createHash('sha256').update(await readFile(resolve(frontend, file))).digest('hex'), pin, `Input changed: ${file}`)
  await writeFile(resolve(out, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
if (report.cases.some(c => c.status !== 'PASS')) process.exitCode = 1
