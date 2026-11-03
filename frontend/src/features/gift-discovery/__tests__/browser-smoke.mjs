// Native Chrome, actual FE09/FE02/FE07; explicitly synthetic domain fetch and world lease.
import assert from 'node:assert/strict'
import { createServer as netServer } from 'node:net'
import { createHash } from 'node:crypto'
import { readFile, readdir, mkdir, writeFile } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const frontend = resolve(import.meta.dirname, '../../../..'), root = resolve(frontend, '..')
const evidence = process.env.MCR_FE24_EVIDENCE
assert(evidence?.startsWith(resolve(import.meta.dirname, 'evidence') + '/'), 'Use a new evidence child in the assigned feature checkout')
await mkdir(evidence, { recursive: false })
const digest = value => createHash('sha256').update(value).digest('hex'), hashes = {}
async function collect(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === 'evidence') continue
    const file = resolve(directory, entry.name)
    if (entry.isDirectory()) await collect(file)
    else if (/\.(ts|tsx|mjs|html|css)$/.test(entry.name)) hashes[relative(root, file)] = digest(await readFile(file))
  }
}
await collect(resolve(frontend, 'src'))
const report = { result: 'FAIL', scope: 'FE24 fixture consumer/native browser only; synthetic fetch, identities and instrumented world lease; no real backend/game/socket/venue or FE25/26 acceptance',
  base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), sourceHashes: hashes,
  contracts: Object.fromEntries(await Promise.all(['contracts/generated/mcr-domain.openapi.json', 'contracts/domain/v1/giveaway.schema.json', 'contracts/domain/v1/policy.json', 'docs/mcr/party-gift-consumer-contract.md'].map(async file => [file, digest(await readFile(resolve(root, file)))]))),
  checks: [], widths: [], screenshots: [], cleanup: 'NOT_RUN' }
const reserve = netServer(); await new Promise(done => reserve.listen(0, '127.0.0.1', done)); const port = reserve.address().port
await new Promise(done => reserve.close(done))
const vite = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
let browser, stage = 'start'
try {
  await vite.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const [width, height] of [[390, 844], [1280, 900]]) {
    stage = `width-${width}`
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' }), page = await context.newPage(), errors = []
    page.on('pageerror', error => errors.push(error.message))
    const state = () => page.evaluate(() => window.__FE24__.state())
    const mode = (value, target = 'list') => page.evaluate(([value, target]) => window.__FE24__.mode(value, target), [value, target])
    const screenshot = async name => {
      const file = `${name}-${width}.png`
      await page.locator('.shell-panel-body').evaluate(element => { element.scrollTop = 0 })
      await page.screenshot({ path: resolve(evidence, file), fullPage: true }); report.screenshots.push(file)
    }
    await page.goto(`http://127.0.0.1:${port}/src/features/gift-discovery/__tests__/fixtures/preview.html`)
    await expect(page.getByRole('heading', { name: 'Find a little something' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Handmade fan badges', exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Expand panel' }).click()
    assert.equal((await state()).mounts, 1); assert.equal((await state()).resources, 1)
    await screenshot('discovery')
    await page.getByText('Filter giveaways', { exact: true }).click()
    await page.getByLabel('Status', { exact: true }).selectOption('OPEN')
    await page.getByLabel('Stock', { exact: true }).selectOption('true')
    await page.getByLabel('Public place', { exact: true }).selectOption('00000000-0000-4000-8000-000000000014')
    await page.getByLabel('Pickup starts from (KST)', { exact: true }).fill('2026-11-07T17:00')
    await page.getByLabel('Pickup starts before (KST)', { exact: true }).fill('2026-11-07T18:00')
    await page.getByRole('button', { name: 'Apply filters' }).click()
    await expect.poll(async () => (await state()).view.list.phase).toBe('ready')
    assert.equal((await state()).view.list.data.items.length, 1)
    const filtered = new URL((await state()).requests.at(-1))
    assert.equal(filtered.searchParams.get('pickup_after'), '2026-11-07T08:00:00.000Z')
    assert.equal(filtered.searchParams.get('in_stock'), 'true')
    await page.getByRole('button', { name: 'Clear filters' }).click()
    await expect(page.getByRole('link', { name: 'Ribbon keepsakes', exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Next giveaways' }).click()
    await expect(page.locator('.gift-status').getByText('Sold out', { exact: true })).toBeVisible()
    await expect(page.locator('.gift-status').getByText('Ended', { exact: true })).toBeVisible()
    await expect(page.locator('.gift-status').getByText('Cancelled', { exact: true })).toBeVisible()
    await page.getByRole('link', { name: 'Pocket lyric zines', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Reserve unavailable' })).toBeDisabled()
    await expect(page.getByText('Reservations unavailable: sold out.', { exact: true })).toBeVisible()
    await page.getByRole('link', { name: '← All giveaways', exact: true }).click()
    await page.getByRole('button', { name: 'Previous giveaways' }).click()
    await page.getByRole('link', { name: 'Handmade fan badges', exact: true }).click()
    await expect(page.getByRole('heading', { name: 'Handmade fan badges', exact: true })).toBeVisible()
    await expect(page.getByText('Your current reservation eligibility has not been confirmed.', { exact: true })).toBeVisible()
    await screenshot('detail')
    assert.equal(await page.locator('.gifts-view img').count(), 0)
    assert.equal((await state()).mounts, 1)
    await page.goBack()
    await expect(page.getByRole('link', { name: 'Handmade fan badges', exact: true })).toBeFocused()
    await page.getByRole('link', { name: 'My gifts →', exact: true }).click()
    await expect(page.getByText('Only you can see this draft.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Next giving', exact: true }).click()
    await expect(page.getByText('No giving activity on this page. You can continue to the next page.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Next giving', exact: true }).click()
    await expect(page.getByRole('link', { name: 'Completed prints', exact: true })).toBeVisible()
    const giving = (await state()).view.giving
    await page.getByRole('button', { name: 'RECEIVING', exact: true }).click()
    await expect(page.getByText('Held', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Next receiving', exact: true }).click()
    await expect(page.getByText('claimed', { exact: true })).toBeVisible()
    assert.deepEqual((await state()).view.giving, giving)
    await page.getByRole('button', { name: 'GIVING', exact: true }).click()
    await expect(page.getByRole('link', { name: 'Completed prints', exact: true })).toBeVisible()
    await screenshot('own-activity')
    assert.equal((await state()).mounts, 1)
    if (width === 390) {
      await page.getByRole('link', { name: 'Browse giveaways →', exact: true }).click()
      for (const [value, title] of [['error', 'Gifts could not be loaded'], ['denied', 'These gifts are not available to this account'], ['leak', 'Gift information could not be verified'], ['rate', 'Please wait before retrying']]) {
        await mode(value); await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible()
        if (value === 'rate') {
          await expect(page.getByRole('button', { name: 'Try again', exact: true })).toBeDisabled()
          await expect(page.getByRole('button', { name: 'Try again', exact: true })).toBeEnabled()
        }
        // Requalify the fixture after each mode; no server-backed policy claim.
        await page.evaluate(() => { window.__FE24__.policy(); window.__FE24__.rebind() })
        await expect(page.getByRole('link', { name: 'Handmade fan badges', exact: true })).toBeVisible()
      }
      await mode('empty'); await expect(page.getByText(/No giveaways on this page/)).toBeVisible()
      await mode('ok'); await page.getByRole('link', { name: 'Handmade fan badges', exact: true }).click()
      await mode('missing', 'detail'); await expect(page.getByRole('heading', { name: 'This giveaway is unavailable', exact: true })).toBeVisible()
      await screenshot('unavailable')
      await mode('ok', 'detail')
      // Start abort-resistant fixture work without awaiting it, then revoke and resolve late.
      await page.evaluate(() => { void window.__FE24__.mode('pending', 'detail') })
      await expect.poll(async () => (await state()).view.detail.phase).toBe('loading')
      await page.evaluate(() => window.__FE24__.policy())
      await expect(page.getByRole('heading', { name: 'Gift access is unavailable', exact: true })).toBeVisible()
      await page.evaluate(() => window.__FE24__.release())
      assert.equal((await state()).view.detail.data, null)
      await page.evaluate(() => window.__FE24__.rebind())
      await expect(page.getByRole('heading', { name: 'Handmade fan badges', exact: true })).toBeVisible()
      await page.evaluate(() => window.__FE24__.replace())
      await expect.poll(async () => (await state()).view.context?.scope.eventId).toBe('00000000-0000-4000-8000-000000000385')
      await expect.poll(async () => (await state()).view.detail.phase).toBe('ready')
      assert.equal((await state()).view.detail.data.event_id, '00000000-0000-4000-8000-000000000385')
      assert.equal((await state()).peakResources, 1)
      report.checks.push('390px filters/query/time conversion, list/detail/back focus, stock/time/paused/cancelled states, independent own cursors incl empty continuation, errors/rate/privacy rejection, policy late-result eviction, account+event replacement')
    }
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow')
    for (const control of await page.locator('.gifts-view button:visible,.gifts-view a:visible,.gifts-view input:visible,.gifts-view select:visible,.gifts-view summary:visible').all()) {
      const box = await control.boundingBox(); assert(box && box.width >= 44 && box.height >= 44, 'small feature touch target')
    }
    assert.equal(await page.getByText('PRIVATE SERVER TEXT MUST NOT RENDER', { exact: false }).count(), 0)
    assert.deepEqual(errors, [])
    await page.evaluate(() => window.__FE24__.dispose())
    const last = await state(); assert.equal(last.resources, 0); assert.equal(last.contextListeners, 0); assert.equal(last.view.disposed, true)
    report.widths.push({ width, height, result: 'PASS', peakShellResourceOwners: last.peakResources, cleanup: 'PASS' })
    await context.close()
  }
  report.result = 'PASS'
} catch (error) { report.stage = stage; report.failure = { name: error.name, message: error.message }; process.exitCode = 1 }
finally {
  await browser?.close(); await vite.close(); report.cleanup = 'PASS'
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify({ result: report.result, widths: report.widths, screenshots: report.screenshots, failure: report.failure }))
