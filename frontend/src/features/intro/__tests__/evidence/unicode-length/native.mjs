import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const directory = fileURLToPath(new URL('.', import.meta.url)), root = fileURLToPath(new URL('../../../../../../', import.meta.url))
const server = await createServer({ root, configLoader: 'runner', cacheDir: '/private/tmp/mcr-fe03-vite-cache', logLevel: 'error', server: { host: '127.0.0.1', port: 0, strictPort: false } })
const report = { result: 'FAIL', scope: 'Actual IntroEditor/controller, synthetic owner HTTP fixture; Google Chrome. No backend, real IME/device, root routing or release acceptance.', browser: '', hashes: {}, checks: [], errors: [] }
const selectedWidths = process.env.MCR_FE03_WIDTHS?.split(',').map(Number)
const widths = selectedWidths ?? [360, 390, 430, 768, 1280, 1440]
assert(widths.every(width => Number.isSafeInteger(width) && width >= 320))
let browser
try {
  for (const path of ['src/features/intro/IntroEditor.tsx', 'src/features/intro/contract.ts', 'src/shared/validation/text.ts']) report.hashes[path] = createHash('sha256').update(await readFile(root+path)).digest('hex')
  if (selectedWidths) {
    const previous = JSON.parse(await readFile(directory+'report.json', 'utf8'))
    assert.equal(previous.result, 'PASS'); assert.deepEqual(previous.hashes, report.hashes)
    report.checks = previous.checks.filter(check => !widths.includes(check.width))
  }
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const width of widths) {
    const page = await browser.newPage({ viewport: { width, height: 1000 } })
    page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(server.resolvedUrls.local[0]+'src/features/intro/__tests__/evidence/unicode-length/preview.html')
    const state = page.locator('#native-state'), textarea = page.getByLabel('Introduction', { exact: true })
    await expect(state).toContainText('"phase":"ready"')
    assert.equal(await textarea.getAttribute('maxlength'), null)
    for (const count of [500, 501, 1000]) {
      await textarea.fill('😀'.repeat(count))
      await expect(textarea).toHaveValue('😀'.repeat(count))
      await expect(textarea).toHaveAttribute('aria-invalid', 'false')
    }
    await page.getByRole('button', { name: 'Save introduction', exact: true }).click()
    await expect(state).toContainText('"savedLength":1000')
    await expect(state).toContainText('"writes":1')
    await page.screenshot({ path: directory+`valid-${width}.png`, fullPage: true })
    await textarea.fill('😀'.repeat(1001))
    await expect(textarea).toHaveAttribute('aria-invalid', 'true')
    await expect(page.getByRole('alert')).toContainText('1001 / 1,000 Unicode code points.')
    await page.getByRole('button', { name: 'Save introduction', exact: true }).click()
    // Existing receipt status can retain the previous success message. The field alert is authoritative for this local length error.
    await expect(page.getByRole('alert')).toContainText('Shorten your introduction before saving.')
    await expect(state).toContainText('"writes":1')
    await expect(textarea).toHaveValue('😀'.repeat(1001))
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)
    assert.equal(overflow, false)
    await page.screenshot({ path: directory+`over-limit-${width}.png`, fullPage: true })
    report.checks.push({ width, positive: [500, 501, 1000], savedLength: 1000, negative: 1001, writesAfterRejection: 1, overflow })
    await page.close()
  }
  assert.deepEqual(report.errors, [])
  report.checks.sort((a, b) => a.width - b.width)
  report.result = 'PASS'
} finally {
  await browser?.close(); await server.close()
  await writeFile(directory+'report.json', JSON.stringify(report, null, 2)+'\n')
}
