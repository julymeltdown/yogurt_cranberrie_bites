import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { preview, build } from 'vite'
import { chromium, expect } from '@playwright/test'
const directory = fileURLToPath(new URL('.', import.meta.url)), root = fileURLToPath(new URL('../../../../../../', import.meta.url))
const hashes = {}
for (const path of ['src/features/people/contract.ts', 'src/features/people/controller.ts', 'src/features/people/People.tsx', 'src/shared/validation/text.ts']) hashes[path] = createHash('sha256').update(await readFile(root+path)).digest('hex')
await build({ root, configLoader: 'runner', cacheDir: '/private/tmp/mcr-fe29-unicode-build-cache', build: { outDir: '/private/tmp/mcr-fe29-unicode-production', emptyOutDir: true, rollupOptions: { input: directory+'preview.html' } } })
const dev = await preview({ root, configLoader: 'runner', logLevel: 'error', build: { outDir: '/private/tmp/mcr-fe29-unicode-production' }, preview: { host: '127.0.0.1', port: 0, strictPort: false } })
const report = { result: 'FAIL', scope: 'Actual people/profile/controller views, synthetic HTTP and existing unavailable-art state; no live/root/realOSIME/acceptance', hashes, build: 'PASS', browser: '', widths: [], errors: [] }
let browser
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 1000 } })
    page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(dev.resolvedUrls.local[0]+'src/features/people/__tests__/evidence/unicode-length/preview.html')
    const state = page.locator('#native-state'), input = page.getByLabel('Find introductions', { exact: true })
    await expect(state).toContainText('"phase":"ready"')
    await expect(page.getByRole('link', { name: '😀'.repeat(40), exact: true })).toBeVisible()
    await expect(page.getByText('😀'.repeat(1000), { exact: true })).toBeVisible()
    assert.equal(await input.getAttribute('maxlength'), null)
    for (const count of [51, 100]) {
      const text = '😀'.repeat(count)
      await input.fill(text); await expect(input).toHaveValue(text); await expect(input).toHaveAttribute('aria-invalid', 'false')
    }
    await page.getByRole('button', { name: 'Search', exact: true }).click()
    await expect(state).toContainText('"requests":2')
    await expect(state).toContainText('"query":"'+'😀'.repeat(100)+'"')
    await page.screenshot({ path: directory+`valid-${width}.png`, fullPage: true })
    await input.fill('😀'.repeat(101)); await page.getByRole('button', { name: 'Search', exact: true }).click()
    await expect(input).toHaveValue('😀'.repeat(101)); await expect(input).toHaveAttribute('aria-invalid', 'true')
    await expect(page.getByRole('alert')).toContainText('101 / 100 Unicode code points after trimming.')
    await expect(state).toContainText('"requests":2')
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth); assert.equal(overflow, false)
    await page.screenshot({ path: directory+`over-limit-${width}.png`, fullPage: true })
    await page.getByRole('link', { name: '😀'.repeat(40), exact: true }).click()
    await expect(page.getByRole('heading', { name: '😀'.repeat(40), exact: true })).toBeVisible()
    await expect(page.getByText('😀'.repeat(160), { exact: true })).toBeVisible()
    report.widths.push({ width, searchAccepted: [51, 100], rejectedRetained: 101, requestsAfterInvalid: 2, discoveryText: 1000, displayName: 40, directProfileBio: 160, overflow })
    await page.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} finally { await browser?.close(); await new Promise(resolve => dev.httpServer.close(resolve)); await writeFile(directory+'report.json', JSON.stringify(report, null, 2)+'\n') }
