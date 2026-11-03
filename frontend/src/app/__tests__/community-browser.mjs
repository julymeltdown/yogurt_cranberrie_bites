import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'

const evidence = resolve(process.env.MCR_COMMUNITY_EVIDENCE ?? '/tmp/mcr-community-evidence')
await mkdir(evidence, { recursive: true })
const server = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser, production
const results = [], errors = []
try {
  await server.listen()
  const base = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of [[390, 844], [1280, 900]]) {
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`${base}/src/app/__tests__/fixtures/community.html`)
    const gift = page.getByRole('link', { name: 'Fan badges', exact: true })
    await expect(gift).toBeVisible()
    const before = await page.evaluate(() => window.__COMMUNITY__.state())
    const host = await page.locator('[data-world-host]').elementHandle()
    await page.screenshot({ path: `${evidence}/gifts-${width}.png` })
    await gift.focus(); await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Fan badges', exact: true })).toBeVisible()
    await page.goBack(); await expect(gift).toBeFocused()
    await page.getByRole('link', { name: 'My gifts →', exact: true }).click()
    await expect(page.getByText('Only you can see this draft.')).toBeVisible()
    await page.getByRole('button', { name: 'RECEIVING', exact: true }).click()
    await expect(page.getByRole('link', { name: 'View giveaway', exact: true })).toBeVisible()
    await page.screenshot({ path: `${evidence}/my-gifts-${width}.png` })
    await page.getByRole('link', { name: 'World', exact: true }).click()
    await page.getByRole('link', { name: 'Gifts', exact: true }).click()
    await expect(gift).toBeVisible()
    assert(await host.evaluate(element => element.isConnected))
    const after = await page.evaluate(() => window.__COMMUNITY__.state())
    assert.equal(after.mounts, before.mounts)
    assert.equal(after.requests.filter(url => new URL(url).pathname.endsWith('/giveaways') && !new URL(url).pathname.endsWith('/me/giveaways')).length, 1)
    assert.equal(after.peakGames, 1); assert.equal(after.peakConnections, 1)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    await page.getByRole('link', { name: 'Meet', exact: true }).click()
    await page.getByRole('link', { name: 'Before the encore', exact: true }).click()
    const apply = page.getByRole('button', { name: 'Apply to party', exact: true })
    await expect(apply).toBeEnabled(); await apply.click()
    await expect(page.getByText('The server is still processing this command.', { exact: true })).toBeVisible()
    await page.getByRole('link', { name: 'Gifts', exact: true }).click(); await expect(gift).toBeVisible()
    await page.getByRole('link', { name: 'Meet', exact: true }).click()
    await expect(page.getByText('The server is still processing this command.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Check original command status', exact: true }).click()
    await expect(page.getByText('The command was confirmed.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'MY PLAN', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Cancel application', exact: true })).toBeEnabled()
    await page.screenshot({ path: `${evidence}/my-plan-${width}.png` })
    await page.getByRole('link', { name: 'Gifts', exact: true }).click(); await expect(gift).toBeVisible()
    const beforeRevocation = await page.evaluate(() => window.__COMMUNITY__.state())
    assert.equal(beforeRevocation.mounts, before.mounts)
    assert.equal(await page.evaluate(() => window.__COMMUNITY__.revoke()), 0)
    await expect(gift).toHaveCount(0)
    await expect(page.getByRole('heading', { name: 'Opening your event', exact: true })).toBeVisible()
    await expect.poll(() => page.evaluate(() => window.__COMMUNITY__.state().bootstrapReads)).toBeGreaterThan(before.bootstrapReads)
    await page.evaluate(() => window.__COMMUNITY__.releaseDisabled())
    await expect(page.getByRole('heading', { name: 'Gifts are unavailable', exact: true })).toBeVisible()
    assert.equal((await page.evaluate(() => window.__COMMUNITY__.state())).requests.length, beforeRevocation.requests.length)
    await page.screenshot({ path: `${evidence}/policy-${width}.png` })
    await page.evaluate(() => window.__COMMUNITY__.dispose())
    const final = await page.evaluate(() => window.__COMMUNITY__.state())
    assert.equal(final.games, 0); assert.equal(final.connections, 0)
    assert.equal(final.mounts, final.disposals); assert(final.signals.every(Boolean))
    results.push({ width, height, state: 'PASS', final })
    await context.close()
  }
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const page = await browser.newPage(), scripts = []
  page.on('request', request => { if (request.resourceType() === 'script') scripts.push(request.url()) })
  await page.goto(`http://127.0.0.1:${production.httpServer.address().port}/login`)
  await expect(page.getByRole('heading', { name: /sign in/i })).toBeVisible()
  assert(!scripts.some(url => /CommunityFeatures|phaser/i.test(url)), 'public entry loaded private feature/world bundle')
  assert.deepEqual(errors, [])
  const paths = ['src/app/CommunityFeatures.tsx', 'src/app/WorldShell.tsx', 'src/app/WorldHost.tsx', 'src/app/access.tsx', 'src/app/router.tsx',
    'src/app/__tests__/community.test.tsx', 'src/app/__tests__/community-browser.mjs', 'src/app/__tests__/fixtures/community.tsx', 'src/app/__tests__/fixtures/community-data.ts']
  const hashes = Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash('sha256').update(await readFile(path)).digest('hex')])))
  await writeFile(`${evidence}/report.json`, JSON.stringify({ state: 'PASS', browser: browser.version(), node: process.version,
    fixture: 'default app composition, actual FE01 transport, synthetic HTTP data and instrumented world leases; no real server/Phaser/socket',
    results, publicScripts: scripts.map(url => new URL(url).pathname), hashes }, null, 2) + '\n')
  console.log(`PASS community composition: 2 widths, default routes, focus/back, persistent owner, policy revalidation, disposal, public bundle isolation. ${evidence}`)
} finally {
  await browser?.close()
  if (production) await new Promise(resolve => production.httpServer.close(resolve))
  await server.close()
}
