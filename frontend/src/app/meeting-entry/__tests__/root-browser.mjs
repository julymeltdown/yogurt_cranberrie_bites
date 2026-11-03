import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { relative, resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

const frontend = resolve(import.meta.dirname, '../../../..'), root = resolve(frontend, '..')
const evidence = resolve(import.meta.dirname, 'evidence/root-browser')
await mkdir(evidence, { recursive: true })
const report = { result: 'FAIL', scope: 'Actual default createAppRoutes/WorldShell with canonical Auth and instrumented fake world. release=null; no live server, authority, venue, real Phaser/socket, device or IN02 acceptance.',
  baseCommit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes: {}, widths: [], errors: [] }
async function hash(file) { report.sourceHashes[relative(root, file)] = createHash('sha256').update(await readFile(file)).digest('hex') }
async function collect(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.name === 'evidence') continue
    const file = resolve(dir, entry.name)
    if (entry.isDirectory()) await collect(file)
    else if (/\.(ts|tsx|mjs|css|html)$/.test(file)) await hash(file)
  }
}
await collect(resolve(import.meta.dirname, '..'))
for (const path of ['src/app/WorldShell.tsx', 'src/app/router.tsx', 'src/app/ports.ts', 'src/app/shell.css', 'src/shared/styles/base.css',
  'src/app/__tests__/fixtures/ports.ts', 'src/features/gift-discovery/__tests__/fixtures/data.ts']) await hash(resolve(frontend, path))
const server = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port: 0 } })
let browser
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  const port = server.httpServer.address().port
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => report.errors.push(e.message))
    const unavailable = page.getByText('Real-world meeting guidance is unavailable until current place, audience, and expiry information is verified.')
    for (const panel of ['check-in', 'beacon']) {
      await page.goto(`http://127.0.0.1:${port}/src/app/meeting-entry/__tests__/root-preview.html?panel=${panel}`)
      await expect(unavailable).toBeVisible()
      await expect(page.getByText('Still taking shape')).toHaveCount(0)
      await expect(page.getByRole('button', { name: 'Share check-in' })).toHaveCount(0)
      const start = await page.evaluate(() => window.__MEETING_ROOT__.state())
      assert.equal(start.world.games, 1); assert.equal(start.world.connections, 1)
      const host = await page.locator('[data-world-host]').elementHandle()
      for (const path of ['/world', '/world/check-in', '/meet/beacons/00000000-0000-4000-8000-000000000015']) {
        await page.evaluate(path => window.__MEETING_ROOT__.navigate(path), path)
        if (path !== '/world') await expect(unavailable).toBeVisible()
        assert(await host.evaluate(el => el === document.querySelector('[data-world-host]')))
        assert.equal((await page.evaluate(() => window.__MEETING_ROOT__.state())).world.mounts, start.world.mounts)
      }
      await page.getByRole('button', { name: 'Back to previous panel' }).focus(); await page.keyboard.press('Enter')
      await expect(page.getByRole('heading', { name: 'Meet', exact: true })).toBeVisible()
      assert.equal((await page.evaluate(() => window.__MEETING_ROOT__.state())).path, '/meet')
      await page.getByRole('button', { name: 'Back to previous panel' }).click()
      assert.equal((await page.evaluate(() => window.__MEETING_ROOT__.state())).path, '/world')
      await page.evaluate(() => window.__MEETING_ROOT__.navigate('/world/check-in'))
      await expect(unavailable).toBeVisible()
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
      if (width === 390 && panel === 'beacon') await page.screenshot({ path: resolve(evidence, 'default-check-in-390.png'), fullPage: true })
      for (const action of ['policy', 'identity']) {
        const before = (await page.evaluate(() => window.__MEETING_ROOT__.state())).world.disposals
        await page.evaluate(action => window.__MEETING_ROOT__[action](), action)
        await expect(unavailable).toBeVisible()
        await expect.poll(async () => (await page.evaluate(() => window.__MEETING_ROOT__.state())).world.games).toBe(1)
        const current = await page.evaluate(() => window.__MEETING_ROOT__.state())
        assert(current.world.disposals > before); assert.equal(current.world.peakGames, 1); assert.equal(current.world.peakConnections, 1)
        assert.equal(current.meetingRequests, 0); assert.equal(current.writes, 0)
      }
      await page.evaluate(() => window.__MEETING_ROOT__.navigate('/meet/beacons/not-a-uuid'))
      await expect(page.getByText('This link is unavailable')).toBeVisible()
      await page.evaluate(() => window.__MEETING_ROOT__.dispose())
      const end = await page.evaluate(() => window.__MEETING_ROOT__.state())
      assert.equal(end.world.games, 0); assert.equal(end.world.connections, 0); assert.equal(end.meetingRequests, 0); assert.equal(end.writes, 0)
    }
    report.widths.push({ width, directEntries: ['W07', 'M06'], result: 'PASS', strictMode: true, navigationWorldHostStable: true, peakGames: 1, peakConnections: 1, meetingRequests: 0, writes: 0, cleanup: 'PASS' })
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally { await browser?.close(); await server.close(); report.cleanup = 'PASS'; await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report))
