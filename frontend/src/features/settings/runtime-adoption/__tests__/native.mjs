import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const directory = process.env.MCR_FE34_RUNTIME_EVIDENCE
if (!directory) throw new Error('Explicit owned evidence output directory required')
await mkdir(directory, { recursive: true })
const dev = await createServer({ configLoader: 'runner', cacheDir: `${directory}/vite-cache`, server: { host: '127.0.0.1', port: 0, strictPort: false } })
const errors = [], report = { fixtureOnly: true, status: 'NOT_RUN', errors }
let browser
try {
  await dev.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
  page.on('pageerror', error => errors.push(error.message))
  await page.goto(`http://127.0.0.1:${dev.httpServer.address().port}/src/features/settings/runtime-adoption/__tests__/preview.html`)
  const read = () => page.evaluate(() => window.__FE34_RUNTIME__.read())
  await expect(page.getByLabel('Enable sound')).not.toBeChecked()
  assert.equal((await read()).soundCalls, 0)
  await page.getByLabel('Enable sound').check(); await page.getByLabel('Low power', { exact: true }).check()
  await page.getByRole('button', { name: 'Save preferences', exact: true }).click()
  await expect(page.getByText('Preferences saved.', { exact: false })).toBeVisible()
  assert.equal((await read()).soundCalls, 0); assert.equal((await read()).effectCalls, 0)
  await page.getByRole('button', { name: 'Apply saved preferences in this session' }).click()
  await expect(page.getByText('Saved preferences applied in this session.')).toBeVisible()
  assert.equal((await read()).activeSound, 1); assert.equal((await read()).adoption.reducedMotion, true)
  await page.emulateMedia({ reducedMotion: 'no-preference' })
  await expect.poll(async () => (await read()).adoption.reducedMotion).toBe(false)
  assert.equal((await read()).soundCalls, 1)
  await page.getByLabel('Preferred era').selectOption('PARADE'); await page.getByRole('button', { name: 'Save preferences', exact: true }).click()
  await expect.poll(async () => (await read()).writes).toBe(2)
  await page.getByRole('button', { name: 'Apply saved preferences in this session' }).click()
  await expect(page.getByText('Could not apply preferences. Saved values are unchanged. Try again.')).toBeVisible()
  assert.equal((await read()).activeSound, 0); assert.equal((await read()).activeEffects, 0)
  await page.evaluate(() => window.__FE34_RUNTIME__.revoke())
  assert.equal((await read()).adoption.phase, 'disposed')
  assert.equal(await page.locator('canvas').count(), 0)
  assert.deepEqual(errors, [])
  Object.assign(report, { status: 'PASS', browser: browser.version(), cases: ['save/apply separation', 'explicit sound gesture only', 'OS override/update without audio restart', 'actual ThemeController failure', 'privacy disposal'], result: await read() })
  await context.close()
} finally {
  await browser?.close(); await dev.close()
  await writeFile(`${directory}/native.json`, `${JSON.stringify(report, null, 2)}\n`)
}
console.log(JSON.stringify(report))
