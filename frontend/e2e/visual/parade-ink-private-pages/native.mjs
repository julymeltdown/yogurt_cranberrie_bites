import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'
const origin = process.env.MCR_PI_ORIGIN, output = process.env.MCR_PI_OUTPUT
assert(origin && output && process.env.MCR_PI_ALLOCATION)
assert.equal(new URL(origin).hostname, '127.0.0.1')
await mkdir(output, { recursive: true })
const browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
const report = { status: 'IN_PROGRESS', browser: browser.version(), startedAt: new Date().toISOString(), qualification: 'ACTUAL_MAIN_CANONICAL_AUTH_PRODUCTION_READINESS_CONTROLLED_HTTP_C05_NOT_LIVE_OR_DEVICE', cases: [], screenshots: [], errors: [] }
const retained = JSON.parse(await readFile(process.env.MCR_PI_RETAINED, 'utf8'))
report.retainedScreenshots = retained.screenshots
report.reusedCaptures = []
const persistReport = () => writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
let context, page, stage = 'start'
const routes = [
  ['social', '/me/social', 'SOCIAL LINKS', '.pi-private-social'], ['requests', '/me/requests', 'REQUESTS', '.pi-private-social'],
  ['age', '/me/age', 'AGE & PRIVACY', '.pi-private-age'], ['blocks', '/me/blocks', 'Blocked users', '.pi-private-blocks'],
  ['account', '/me/account', 'Account controls', '.pi-private-account'], ['inbox', '/notifications', 'INBOX', '.pi-private-inbox'],
]
const state = () => page.evaluate(() => window.__PARADE_PRIVATE__.state())
const mode = value => page.evaluate(v => window.__PARADE_PRIVATE__.mode(v), value)
const reducePanel = async () => { const control = page.getByRole('button', { name: 'Reduce panel', exact: true }); if (await control.isVisible()) await control.click() }
const recoverPolicyChange = async () => { await expect(page.locator('[data-mcr-shell]')).toHaveAttribute('data-panel-size', 'full'); await reducePanel(); await expect.poll(async () => (await state()).activeSockets).toBe(1) }
const nav = async (path, heading) => { await page.evaluate(p => window.__PARADE_PRIVATE__.navigate(p), path); if (path !== '/archive') { await expect(page.locator('[data-mcr-shell]')).toHaveAttribute('data-panel-size', 'full'); await reducePanel() } const selector = routes.find(row => row[1] === path)?.[3] ?? '.pi-private-archive'; await expect(page.locator(selector).first().getByRole('heading', { name: heading, exact: true })).toBeVisible() }
async function shot(name, selector) {
  const key = `${name}-${page.viewportSize().width}`
  if (report.retainedScreenshots.some(shot => shot.key === key)) { report.reusedCaptures.push(key); return }
  await page.evaluate(async () => { await document.fonts.ready })
  const metrics = await page.evaluate(selector => {
    const region = document.querySelector(selector), css = region && getComputedStyle(region)
    const rect = el => { const r = el.getBoundingClientRect(), c = getComputedStyle(el); return { x: r.x, y: r.y, width: r.width, height: r.height, font: c.fontFamily, size: c.fontSize, leading: c.lineHeight, color: c.color, background: c.backgroundColor, backgroundImage: c.backgroundImage, border: c.borderTopWidth, radius: c.borderRadius } }
    return { viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio }, root: region ? rect(region) : null,
      heading: region?.querySelector('h1') ? rect(region.querySelector('h1')) : null,
      bodyFont: css?.fontFamily, horizontalOverflow: document.documentElement.scrollWidth > innerWidth,
      controls: [...(region?.querySelectorAll('button,input:not([type=checkbox]),select,a') ?? [])].filter(el => el.getClientRects().length).map(el => ({ label: el.getAttribute('aria-label') ?? el.textContent?.trim(), ...rect(el) })),
      privacy: { leakedDiagnostic: document.body.textContent.includes('PRIVATE_PRODUCER_DETAIL_MUST_NOT_RENDER') },
    }
  }, selector)
  assert(metrics.root, name + ' missing root'); assert.equal(metrics.horizontalOverflow, false)
  assert.equal(metrics.root.background, 'rgb(249, 249, 248)'); assert.equal(metrics.root.backgroundImage, 'none')
  assert(metrics.bodyFont.includes('Parade Ink Text'))
  for (const control of metrics.controls) { assert(control.height >= 43.9, name + ' small target ' + control.label); assert(control.width <= metrics.viewport.width + .1, name + ' wide control ' + control.label) }
  assert.equal(metrics.privacy.leakedDiagnostic, false)
  const file = `${name}-${metrics.viewport.width}.png`
  await page.screenshot({ path: resolve(output, file), fullPage: false })
  report.screenshots.push({ file, capturedAt: new Date().toISOString(), metrics })
  await persistReport()
}
async function start(width, initialMode = 'loaded') {
  context = await browser.newContext({ viewport: { width, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce', deviceScaleFactor: 1 })
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  await page.goto(origin + `e2e/visual/parade-ink-private-pages/preview.html?mode=${initialMode}`)
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test'); await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await page.getByRole('button', { name: 'Reduce panel', exact: true }).click()
  await expect.poll(async () => (await state()).canvases).toBe(1)
  await expect(page.getByText('This section is unavailable for the current account or event.', { exact: true })).toBeVisible()
  assert.equal((await state()).requests.filter(r => /\/me\/(social-handles|social-requests|age-status|notifications)$/.test(r.path)).length, 0)
  await page.evaluate(() => window.__PARADE_PRIVATE__.baseline())
  await expect(page.getByRole('heading', { name: 'SOCIAL LINKS', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Remove Instagram', exact: true })).toBeEnabled()
}
try {
  for (const width of [360, 390, 430]) {
    stage = `seven normal routes ${width}`; await start(width)
    for (const [name, path, heading, selector] of routes) {
      await nav(path, heading)
      if (name === 'requests') await expect(page.getByRole('button', { name: 'Open request', exact: true })).toBeVisible()
      if (name === 'blocks') await expect(page.getByRole('button', { name: 'Unblock user', exact: true })).toBeVisible()
      if (name === 'inbox') await expect(page.getByRole('button', { name: 'Mark as read', exact: true })).toBeVisible()
      await shot(name, selector)
    }
    const normal = await state(); assert.equal(normal.peakCanvases, 1); assert.equal(normal.peakSockets, 1); assert(normal.sameHost && normal.sameCanvas)
    assert.equal(normal.requests.filter(r => /reveal/.test(r.path)).length, 0)
    if (width === 390) {
      stage = 'input and explicit sharing consent'
      await nav('/me/social', 'SOCIAL LINKS')
      const replacement = page.getByLabel('Replacement Instagram account name', { exact: true })
      await replacement.fill('bad..name'); await expect(page.getByRole('button', { name: 'Replace privately', exact: true })).toBeDisabled()
      await replacement.fill('controlled_fixture')
      await page.setViewportSize({ width: 390, height: 480 }); await replacement.focus(); await replacement.scrollIntoViewIfNeeded()
      await shot('social-short-input', '.pi-private-social')
      await page.setViewportSize({ width: 390, height: 844 })
      await page.getByRole('button', { name: 'Remove Instagram', exact: true }).click(); await page.getByRole('button', { name: 'Keep handle', exact: true }).click()
      assert.equal((await state()).requests.filter(r => r.method !== 'GET' && /social-handles/.test(r.path)).length, 0)
      await nav('/me/requests', 'REQUESTS'); await page.getByRole('button', { name: 'Open request', exact: true }).click(); await reducePanel()
      const approve = page.getByRole('button', { name: 'Approve selected platforms', exact: true }); await expect(approve).toBeDisabled()
      await page.getByRole('checkbox', { name: 'Exchange Instagram account names', exact: true }).check(); await expect(approve).toBeDisabled()
      await page.getByRole('checkbox', { name: /I explicitly consent/ }).check(); await shot('requests-consent', '.pi-private-social')
      await approve.click(); await expect(page.getByRole('button', { name: 'Reveal Instagram', exact: true })).toBeVisible()
      assert.equal((await state()).requests.filter(r => /reveal/.test(r.path)).length, 0)
      await page.getByRole('button', { name: 'Reveal Instagram', exact: true }).click(); await expect(page.getByText('@controlled_fixture', { exact: true })).toBeVisible()
      await shot('request-reveal', '.pi-private-social')
      await expect(page.getByText('@controlled_fixture', { exact: true })).toHaveCount(0, { timeout: 7000 })
      await page.getByRole('button', { name: 'Revoke sharing', exact: true }).click(); await page.getByRole('button', { name: 'Confirm revocation', exact: true }).click()
      await expect(page.getByText('@controlled_fixture', { exact: true })).toHaveCount(0)
      await recoverPolicyChange()
      await page.getByRole('button', { name: 'Open request', exact: true }).click(); await reducePanel()
      await expect(page.getByText('REVOKED', { exact: true })).toBeVisible()
      for (const value of ['REJECTED', 'EXPIRED']) {
        await page.getByRole('button', { name: 'Back to requests', exact: true }).click(); await reducePanel(); await mode('request-' + value)
        await page.getByRole('button', { name: 'Refresh requests', exact: true }).click(); await page.getByRole('button', { name: 'Open request', exact: true }).click(); await reducePanel()
        await expect(page.getByText(value, { exact: true })).toBeVisible(); await shot('request-' + value.toLowerCase(), '.pi-private-social')
      }
      report.cases.push({ name: '390-sharing-consent-grant-expiry-revocation-and-terminal-states', status: 'PASS', state: await state() })
      await persistReport()
      await context.close(); context = null; await start(390)
      await mode('loaded'); await nav('/me/age', 'AGE & PRIVACY')
      for (const [value, label] of [['VERIFIED','Verification current'],['EXPIRED','Verification expired'],['UNSUPPORTED','Verification unsupported'],['PROVIDER_UNAVAILABLE','Verification provider unavailable']]) {
        await page.evaluate(v => window.__PARADE_PRIVATE__.age(v), value); await page.getByRole('button', { name: 'Refresh age status', exact: true }).click(); await expect(page.getByRole('heading', { name: label, exact: true })).toBeVisible()
        await expect(page.getByRole('button', { name: 'Start verification', exact: true })).toBeDisabled()
      }
      await shot('age-provider-unavailable', '.pi-private-age')
      stage = 'blocked-user native modal focus and input'
      await nav('/me/blocks', 'Blocked users'); const unblock = page.getByRole('button', { name: 'Unblock user', exact: true }); await unblock.click()
      const dialog = page.getByRole('dialog', { name: 'Unblock this user?', exact: true }), keep = dialog.getByRole('button', { name: 'Keep blocked', exact: true })
      await expect(keep).toBeFocused(); await page.keyboard.press('Shift+Tab'); await expect(dialog.getByRole('button', { name: 'Confirm unblock', exact: true })).toBeFocused()
      await page.keyboard.press('Tab'); await expect(keep).toBeFocused()
      await page.setViewportSize({ width: 390, height: 480 }); await shot('blocks-short-dialog', '.pi-private-blocks')
      await page.keyboard.press('Escape'); await expect(dialog).toHaveCount(0); await expect(unblock).toBeFocused()
      await page.setViewportSize({ width: 390, height: 844 }); await unblock.click(); await page.getByRole('button', { name: 'Confirm unblock', exact: true }).click()
      await recoverPolicyChange()
      await expect(page.getByText('No blocked users on this page.', { exact: true })).toBeVisible()
      stage = 'account cancellation and confirmation'
      await nav('/me/account', 'Account controls'); await page.getByRole('button', { name: 'Sign out', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Stay signed in', exact: true })).toBeFocused(); await page.keyboard.press('Escape')
      await page.getByRole('button', { name: 'Request account deletion', exact: true }).click()
      const confirm = page.getByRole('button', { name: 'Confirm deletion request', exact: true }); await expect(confirm).toBeDisabled()
      await page.getByLabel('Deletion confirmation', { exact: true }).fill('DELETE_MY_ACCOUNT'); await expect(confirm).toBeEnabled()
      await page.setViewportSize({ width: 390, height: 480 }); await confirm.scrollIntoViewIfNeeded(); await shot('account-short-confirmation', '.pi-private-account')
      await page.keyboard.press('Escape'); await page.setViewportSize({ width: 390, height: 844 })
      await page.getByRole('button', { name: 'Request account deletion', exact: true }).click(); await expect(page.getByLabel('Deletion confirmation', { exact: true })).toHaveValue('')
      await page.getByRole('button', { name: 'Keep account', exact: true }).click()
      assert.equal((await state()).requests.filter(r => r.path === '/api/auth/account' && r.method === 'DELETE').length, 0)
      stage = 'inbox command, empty and failure'
      await nav('/notifications', 'INBOX'); await page.getByRole('button', { name: 'Mark as read', exact: true }).click(); await expect(page.getByText('Read-state command confirmed.', { exact: true })).toBeVisible()
      await page.getByRole('button', { name: 'Open update', exact: true }).click(); await expect.poll(() => page.evaluate(() => location.pathname)).toBe('/me/requests')
      await nav('/notifications', 'INBOX'); await mode('empty'); await page.getByRole('button', { name: 'Refresh inbox', exact: true }).click(); await expect(page.getByText('Your inbox is empty.', { exact: true })).toBeVisible(); await shot('inbox-empty', '.pi-private-inbox')
      await mode('error'); await page.getByRole('button', { name: 'Refresh inbox', exact: true }).click(); await expect(page.getByRole('alert')).toBeVisible(); await shot('inbox-error', '.pi-private-inbox'); await mode('loaded')
      report.cases.push({ name: '390-private-consent-expiry-cancel-focus-commands-states', status: 'PASS', state: await state() })
      await persistReport()
    }
    stage = `public archive ${width}`
    await nav('/archive', 'After the show'); await expect(page.getByText(/We sang every word/)).toBeVisible(); await shot('archive', '.pi-private-archive')
    const publicState = await state(), reads = publicState.requests.filter(r => r.path.endsWith('/archive'))
    assert(reads.length > 0 && reads.every(r => !r.authorized && r.credentials === 'omit' && r.cache === 'no-store'))
    assert.equal(publicState.canvases, 0); await expect(page.getByRole('textbox')).toHaveCount(0)
    if (width === 390) {
      await page.getByRole('button', { name: /View archived image 1/ }).click(); await expect(page.getByText('Archived image unavailable. No live or private image is substituted.', { exact: true })).toBeVisible()
      await page.getByRole('button', { name: 'Next page', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Posts after the show', exact: true })).toBeFocused()
      await mode('error'); await page.getByRole('button', { name: 'Reload archive', exact: true }).click(); await expect(page.getByRole('alert')).toBeVisible(); await shot('archive-error', '.pi-private-archive')
      await mode('empty'); await page.getByRole('button', { name: 'Reload archive', exact: true }).click(); await expect(page.getByText('No permitted posts on this page.', { exact: true })).toBeVisible(); await shot('archive-empty', '.pi-private-archive')
    }
    report.cases.push({ name: `seven-default-routes-${width}`, status: 'PASS', state: publicState })
    await persistReport()
    await context.close(); context = null
  }
  assert.deepEqual(report.errors, []); report.status = 'PASS'
 } catch (error) {
  report.status = 'FAIL'; report.failure = { stage, error: String(error) }
  await persistReport()
  report.failure.text = await page?.locator('body').innerText().catch(() => null)
  report.failure.state = await page?.evaluate(() => window.__PARADE_PRIVATE__?.state()).catch(() => null)
  await persistReport()
  await page?.screenshot({ path: resolve(output, 'failure.png') }).catch(() => null)
  process.exitCode = 1
} finally {
  report.finishedAt = new Date().toISOString()
  try { await persistReport() } finally { await context?.close(); await browser.close() }
}
