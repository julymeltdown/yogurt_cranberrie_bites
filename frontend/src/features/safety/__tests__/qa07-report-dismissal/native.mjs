import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, statfs, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { createServer as createNetServer } from 'node:net'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
import { createServer } from 'vite'

const here = import.meta.dirname, frontend = resolve(here, '../../../../..'), root = resolve(frontend, '..')
const run = process.env.DISMISSAL_RUN || 'corrected'
assert(/^[a-z0-9-]+$/.test(run))
const output = resolve(here, 'evidence', run)
const disk = await statfs(root)
assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, '8GiB disk floor')
await mkdir(output, { recursive: true })
const pins = {}
for (const file of ['src/features/safety/controller.ts', 'src/features/safety/panels.tsx', 'src/features/safety/__tests__/preview.tsx', 'src/features/safety/__tests__/qa07-report-dismissal/native.mjs', 'pnpm-lock.yaml']) {
  pins[file] = createHash('sha256').update(await readFile(resolve(frontend, file))).digest('hex')
}
const report = {
  candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), pins,
  classification: 'Native macOS Chrome; actual retained FE36 controller and product panels; synthetic session/authorization, no live backend or physical-device acceptance',
  cases: [], browser: null,
}
const reservation = createNetServer()
await new Promise(done => reservation.listen(0, '127.0.0.1', done))
const port = reservation.address().port
await new Promise(done => reservation.close(done))
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(tmpdir(), `mcr-fe36-dismissal-${process.pid}`), logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
let browser
const button = (page, name) => page.getByRole('button', { name, exact: true })
async function tabTo(page, target) {
  await expect(target).toBeVisible()
  assert(await target.isEnabled(), `Keyboard target disabled: ${await target.textContent()}`)
  for (let i = 0; i < 40; i++) {
    if (await target.evaluate(el => el === document.activeElement)) return
    await page.keyboard.press('Tab')
  }
  throw Error('Target unreachable through Tab')
}
async function activate(page, target) { await tabTo(page, target); await page.keyboard.press('Enter') }
try {
  await server.listen()
  const origin = server.resolvedUrls.local[0].replace(/\/$/, '')
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const width of process.env.DISMISSAL_WIDTH ? [Number(process.env.DISMISSAL_WIDTH)] : [360, 390, 430]) {
    for (const scenario of ['escape', 'close-button', 'policy-revoked', 'identity-changed']) {
      const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', serviceWorkers: 'block' })
      const page = await context.newPage(), errors = [], requests = []
      const result = { width, scenario, status: 'FAIL', requests }
      page.on('pageerror', e => errors.push(e.message))
      page.on('request', r => { if (new URL(r.url()).pathname.startsWith('/api/')) requests.push({ method: r.method(), path: new URL(r.url()).pathname }) })
      try {
        await page.goto(origin + '/src/features/safety/__tests__/preview.html')
        const opener = button(page, 'Open report'), block = button(page, 'Block user')
        await activate(page, opener)
        await expect(button(page, 'Close report')).toBeFocused()
        const draft = page.getByRole('textbox', { name: /What happened/ })
        await tabTo(page, draft)
        await page.keyboard.type('QA07 keyboard report draft')
        await expect(draft).toHaveValue('QA07 keyboard report draft')
        if (scenario === 'policy-revoked') await page.evaluate(() => window.__FE36__.revoke())
        if (scenario === 'identity-changed') await page.evaluate(() => window.__FE36__.changeIdentity())
        if (scenario === 'close-button') await activate(page, button(page, 'Close report'))
        else await page.keyboard.press('Escape')
        await expect(page.getByRole('dialog')).toHaveCount(0)
        await expect(opener).toBeFocused()
        result.afterDismiss = { blockEnabled: await block.isEnabled(), status: await page.getByRole('status').allTextContents() }
        if (scenario.endsWith('revoked') || scenario.endsWith('changed')) {
          await expect(block).toBeDisabled()
          await activate(page, opener)
          await expect(page.getByRole('textbox', { name: /What happened/ })).toHaveCount(0)
          await expect(page.getByText(/Target:/)).toHaveCount(0)
          await page.keyboard.press('Escape')
          await expect(block).toBeDisabled()
        } else {
          await activate(page, block)
          await expect(button(page, 'Cancel block')).toBeFocused()
          await page.keyboard.press('Shift+Tab')
          await expect(button(page, 'Confirm block')).toBeFocused()
          await page.keyboard.press('Escape')
          await expect(block).toBeFocused()
          await activate(page, opener)
          await expect(page.getByRole('textbox', { name: /What happened/ })).toHaveValue('')
          await page.keyboard.press('Escape')
          await expect(opener).toBeFocused()
          await activate(page, block)
          await activate(page, button(page, 'Cancel block'))
          await expect(block).toBeFocused()
          await expect(block).toBeEnabled()
        }
        assert.deepEqual(requests, [], 'No report/block mutation or API read during dismissal')
        assert.deepEqual(errors, [], 'No uncaught browser errors')
        result.status = 'PASS'
      } catch (error) { result.failure = error.message; result.errors = errors }
      await writeFile(resolve(output, `${width}-${scenario}.aria.txt`), await page.locator('body').ariaSnapshot())
      await page.screenshot({ path: resolve(output, `${width}-${scenario}.png`), fullPage: true })
      report.cases.push(result)
      console.log(`${result.status} ${width} ${scenario}${result.failure ? `: ${result.failure.split('\n')[0]}` : ''}`)
      await context.close()
    }
  }
} finally {
  await browser?.close()
  await server.close()
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
if (report.cases.some(c => c.status !== 'PASS')) process.exitCode = 1
