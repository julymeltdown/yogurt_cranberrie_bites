// Run with the pinned Node from this worktree's frontend directory. Uses installed Chrome only.
import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'

const executablePath = process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const evidence = process.env.MCR_FE07_EVIDENCE ?? '/tmp/mcr-fe07-evidence'
await mkdir(evidence, { recursive: true })
const dev = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false } })
let production
let browser
const results = []
try {
  await dev.listen()
  const address = dev.httpServer.address()
  assert(address && typeof address !== 'string')
  const base = `http://127.0.0.1:${address.port}`
  browser = await chromium.launch({ executablePath, headless: true })
  const errors = []
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`${base}/src/app/__tests__/fixtures/preview.html`)
    await expect(page.getByRole('heading', { name: 'Checking your session' })).toBeVisible()
    assert.equal(await page.locator('[data-world-host]').count(), 0)
    await page.getByText('FE07 test controls', { exact: true }).click()
    await page.getByRole('button', { name: 'Authorize fixture' }).click()
    await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Giveaway', exact: true })).toBeFocused()
    await page.getByText('FE07 test controls', { exact: true }).click()
    const host = await page.locator('[data-world-host]').elementHandle()
    const initial = await page.evaluate(() => window.__FE07_STATS__.mounts)
    await page.screenshot({ path: `${evidence}/deep-gift-${width}.png` })
    for (const name of ['World', 'Feed', 'Meet', 'Gifts', 'Wall', 'Me']) {
      await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name, exact: true }).click()
      assert(await host.evaluate(element => element.isConnected), 'host remounted')
      assert.equal(await page.evaluate(() => window.__FE07_STATS__.mounts), initial)
    }
    await page.getByRole('link', { name: 'Gifts', exact: true }).click()
    const opener = page.getByRole('link', { name: 'Open test giveaway' })
    await opener.focus()
    await page.keyboard.press('Enter')
    await expect(page.getByRole('heading', { name: 'Giveaway', exact: true })).toBeFocused()
    await page.goBack()
    await expect(opener).toBeFocused()
    const panelBody = page.locator('.shell-panel-body')
    await panelBody.evaluate(element => { element.scrollTop = 180 })
    await page.getByRole('link', { name: 'World', exact: true }).click()
    await page.screenshot({ path: `${evidence}/world-${width}.png` })
    await page.getByRole('link', { name: 'Gifts', exact: true }).click()
    assert(await panelBody.evaluate(element => element.scrollTop >= 170), 'list scroll lost')
    await page.getByRole('button', { name: 'Expand panel' }).click()
    assert.equal(await page.evaluate(() => window.__FE07_STATS__.visible), false)
    await page.getByRole('button', { name: 'Reduce panel' }).click()
    await page.getByRole('button', { name: 'Minimize panel' }).click()
    await expect(opener).not.toBeVisible()
    await page.getByRole('button', { name: 'Show panel content' }).click()
    await expect(opener).toBeVisible()
    await opener.click()
    await page.getByRole('link', { name: 'Manage test giveaway' }).click()
    const draft = page.getByRole('textbox', { name: 'Test draft' })
    await draft.fill('Test draft stays in the active panel')
    await page.keyboard.press('Escape')
    await expect(draft).toBeVisible()
    assert.equal(await page.evaluate(() => window.__FE07_STATS__.inputBlocked), true)
    await page.setViewportSize({ width, height: 420 })
    await expect(draft).toBeVisible()
    await expect.poll(async () => {
      const bottom = await page.getByRole('navigation', { name: 'Main navigation' }).boundingBox()
      return bottom && bottom.y >= 0 && bottom.y + bottom.height <= 421
    }, { message: 'navigation clipped in reduced viewport' }).toBe(true)
    await page.setViewportSize({ width, height })
    await page.getByRole('button', { name: 'Back to previous panel' }).click()
    await page.getByRole('button', { name: 'Back to previous panel' }).click()
    await page.screenshot({ path: `${evidence}/gifts-${width}.png` })
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal page overflow')
    for (const target of await page.locator('.shell-tabs a, .shell-panel-header button, .shell-header a').all()) {
      const box = await target.boundingBox()
      assert(box && box.width >= 44 && box.height >= 44, 'touch target below 44px')
    }
    const active = page.getByRole('link', { name: 'Gifts', exact: true })
    await active.focus()
    await page.keyboard.press('Tab')
    await page.keyboard.press('Shift+Tab')
    await expect(active).toBeFocused()
    assert.notEqual(await active.evaluate(element => getComputedStyle(element).outlineStyle), 'none', 'missing focus indicator')
    await page.getByText('FE07 test controls', { exact: true }).click()
    await page.getByRole('button', { name: 'Open report fixture' }).click()
    await page.getByText('FE07 test controls', { exact: true }).click()
    await page.getByRole('button', { name: 'Minimize panel' }).click()
    await expect(page.locator('.shell-panel-body')).not.toBeVisible()
    await page.getByText('FE07 test controls', { exact: true }).click()
    await page.getByRole('button', { name: 'Next report fixture' }).click()
    await expect(page.locator('.shell-panel-body')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Report', exact: true })).toBeFocused()
    assert.equal(await page.evaluate(() => window.__FE07_STATS__.mounts), initial)
    await page.getByRole('button', { name: 'Switch account' }).click()
    await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
    await page.getByRole('button', { name: 'Switch event' }).click()
    await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
    await page.getByRole('button', { name: 'Admin exit' }).click()
    await expect(page.getByRole('heading', { name: 'Access denied' })).toBeVisible()
    assert.equal(await page.evaluate(() => window.__FE07_STATS__.games), 0)
    assert.equal(await page.evaluate(() => window.__FE07_STATS__.connections), 0)
    const stats = await page.evaluate(() => window.__FE07_STATS__)
    assert.equal(stats.peakGames, 1); assert.equal(stats.peakConnections, 1)
    results.push({ width, height, stats: { ...stats, signals: undefined }, state: 'PASS' })
    console.log(`PASS FE07 Chrome ${width}x${height}: deep link, persistent host, focus/back, panel states/scroll/query targets, reduced viewport, identity, denied admin`)
    await context.close()
  }
  // The built production entry has no auth or fixture switch and opens no world/socket.
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const address2 = production.httpServer.address()
  assert(address2 && typeof address2 !== 'string')
  const productionBase = `http://127.0.0.1:${address2.port}`
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } })
  let sockets = 0
  page.on('websocket', () => sockets++)
  page.on('pageerror', error => errors.push(error.message))
  await page.goto(productionBase)
  await expect(page.getByRole('heading', { name: /One world.*Every era/ })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Enter the world', exact: true })).toBeDisabled()
  await expect(page.getByText('World entry is not open yet.', { exact: true })).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Public information' })).toBeVisible()
  await page.goto(`${productionBase}/gifts/00000000-0000-4000-8000-0000000000a5?fixture=ready`)
  await expect(page.getByRole('heading', { name: 'World entry is not open yet' })).toBeVisible()
  assert.equal(await page.locator('[data-world-host]').count(), 0)
  assert.equal(await page.evaluate(() => window.__FE07_STATS__), undefined)
  assert.equal(sockets, 0)
  await page.screenshot({ path: `${evidence}/production-entry-390.png` })
  assert.deepEqual(errors, [])
  await writeFile(`${evidence}/browser-results.json`, JSON.stringify({ browser: browser.version(), results,
    production: 'PASS: public fallback, no fixture bypass, no native WebSocket',
    limits: 'Instrumented ports only; actual Phaser/socket integration, physical devices, native mobile keyboard and screen readers NOT_RUN' }, null, 2))
  console.log(`PASS production entry; Chrome ${browser.version()}; screenshots/evidence ${evidence}`)
} finally {
  await browser?.close()
  await dev.close()
  await new Promise(resolve => production ? production.httpServer.close(resolve) : resolve())
}
