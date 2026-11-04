// FE37-only native qualification. Existing evidence runners are not overwritten.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const root = process.cwd(), frontend = resolve(root, 'frontend')
const temporary = process.env.MCR_OPS_SOURCE_TMP, output = process.env.MCR_OPS_OUTPUT
assert(temporary?.startsWith('/private/tmp/mcr-operations-current-') && output?.startsWith(resolve(root, 'docs/mcr/evidence/operations-fe37-fe38-current/runs/')))
const require = createRequire(resolve(frontend, 'package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
const { chromium, expect } = require('@playwright/test')
const report = { scope: 'Synthetic FE37 components with canonical Auth/privacy and explicit fixture role/evidence ports. No production admin route, role producer, real moderation, FE38 or live-service acceptance.', cases: [], captures: [], errors: [], released: false }
let browser, server, port
async function close() { await browser?.close(); await server?.close() }
process.once('SIGTERM', () => { void close().finally(() => process.exit(143)) })
try {
  server = await createServer({ root: frontend, configFile: false, cacheDir: resolve(temporary, 'vite'), logLevel: 'error', plugins: [react(), {
    name: 'owned-fe37-only-fixture', configureServer(instance) {
      instance.middlewares.use('/__ops-current', (_request, response, next) => {
        const html = '<!doctype html><html lang="en"><head><meta name="viewport" content="width=device-width, initial-scale=1"><title>Synthetic FE37 qualification</title></head><body><div id="root"></div><script type="module" src="/src/features/admin/reports/__tests__/current/preview.tsx"></script></body></html>'
        void instance.transformIndexHtml('/__ops-current', html).then(rendered => {
          response.setHeader('Content-Type', 'text/html; charset=utf-8'); response.end(rendered)
        }, next)
      })
    },
  }], server: { host: '127.0.0.1', port: 0 } })
  await server.listen(); port = server.httpServer.address().port
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version(); report.port = port
  for (const width of [360, 390, 430]) for (const lane of ['reports', 'members', 'content']) for (const fault of ['startup', 'disposed', '']) {
    const page = await browser.newPage({ viewport: { width, height: 844 } })
    page.on('pageerror', error => report.errors.push({ width, lane, fault, error: error.message }))
    await page.addInitScript(() => {
      const active = new Set(), create = URL.createObjectURL.bind(URL), revoke = URL.revokeObjectURL.bind(URL)
      URL.createObjectURL = blob => { const url = create(blob); active.add(url); return url }
      URL.revokeObjectURL = url => { active.delete(url); revoke(url) }
      window.__FE37_URLS__ = () => active.size
    })
    const name = `${lane}-${width}-${fault || 'pending'}`
    await page.goto(`http://127.0.0.1:${port}/__ops-current?lane=${lane}&fault=${fault}`)
    await expect(page.getByText('SYNTHETIC FE37 FIXTURE ONLY.', { exact: false })).toBeVisible()
    if (fault) {
      await expect(page.getByRole('heading', { name: 'Moderator access unavailable' })).toBeVisible()
      assert.equal(await page.evaluate(() => window.__OPS_CURRENT__.requests()), 0)
      assert.equal(await page.evaluate(() => window.__OPS_CURRENT__.snapshot().access), false)
    } else {
      const review = page.getByRole('button', { name: /^Review 000/ }).first()
      await expect(review).toBeVisible(); await review.click()
      await page.getByLabel('Action', { exact: true }).selectOption(lane === 'reports' ? 'RESOLVED' : lane === 'members' ? 'SUSPEND' : 'HIDE')
      await page.getByLabel('Reason for this action').fill('SYNTHETIC factual review reason')
      if (lane === 'reports') {
        assert.equal(await page.getByRole('img').count(), 0)
        await page.getByRole('button', { name: 'Open private evidence 1' }).click()
        const image = page.getByRole('img', { name: 'Private report evidence' })
        await expect(image).toHaveAttribute('src', /^blob:/)
        await expect.poll(() => image.evaluate(node => node.naturalWidth)).toBeGreaterThan(0)
      }
      await page.getByRole('button', { name: 'Review action', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Confirm action', exact: true })).toBeFocused()
      await page.getByRole('button', { name: 'Cancel confirmation' }).click()
      assert.equal(await page.evaluate(() => window.__OPS_CURRENT__.writes()), 0)
      await page.getByRole('button', { name: 'Review action', exact: true }).click()
    }
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    assert.equal((await page.locator('body').innerText()).includes('PRIVATE producer diagnostic'), false)
    if (width === 390) {
      const file = `${name}.png`; await page.screenshot({ path: resolve(output, file), fullPage: true })
      report.captures.push({ file, sha256: createHash('sha256').update(await readFile(resolve(output, file))).digest('hex') })
    }
    if (!fault) {
      await page.getByRole('button', { name: 'Confirm action', exact: true }).click()
      await page.waitForFunction(() => window.__OPS_CURRENT__.snapshot().phase === 'pending')
      assert.equal(await page.evaluate(() => window.__OPS_CURRENT__.writes()), 1)
      await page.getByRole('button', { name: 'Check original command' }).click()
      await page.waitForFunction(() => window.__OPS_CURRENT__.snapshot().phase === 'failed')
      assert.equal(await page.evaluate(() => window.__OPS_CURRENT__.writes()), 1)
      await page.evaluate(() => window.__OPS_CURRENT__.fail())
      await expect(page.getByRole('heading', { name: 'Moderator access unavailable' })).toBeVisible()
      const state = await page.evaluate(() => window.__OPS_CURRENT__.snapshot())
      assert.deepEqual(state.rows, []); assert.equal(state.detail, null); assert.equal(state.reason, ''); assert.equal(state.receipt, null)
      assert.equal(state.evidenceUrl, null); assert.equal(state.confirm, false)
    }
    assert.equal(await page.evaluate(() => window.__FE37_URLS__()), 0)
    await page.evaluate(() => window.__OPS_CURRENT__.dispose()); await page.close()
    report.cases.push({ name, status: 'PASS', writes: fault ? 0 : 1, privateUrlsAfterWithdrawal: 0 })
  }
  assert.deepEqual(report.errors, []); report.status = 'SYNTHETIC_FE37_PASS'
} finally {
  await close(); report.released = true
  await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n')
}
