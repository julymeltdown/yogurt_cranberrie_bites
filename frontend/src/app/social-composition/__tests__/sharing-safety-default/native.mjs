import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
const frontend = resolve(import.meta.dirname, '../../../../..'), repo = resolve(frontend, '..')
const evidence = process.env.MCR_SHARING_SAFETY_OUTPUT
assert(evidence?.startsWith(resolve(repo, 'docs/mcr/evidence/sharing-safety-default-app/') + '/'))
await mkdir(evidence, { recursive: false })
const git = (...args) => execFileSync('git', args, { cwd: repo }).toString().trim()
const input = { revision: 'test-only', reviewedBy: 'test-only', movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: resolve(process.env.TMPDIR, 'vite'), define: {
  'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('00000000-0000-4000-8000-000000000201'),
  'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(JSON.stringify(input)),
}, server: { host: '127.0.0.1', port: 0, hmr: false }, logLevel: 'warn' })
let browser, context, page, stage = 'start'
const report = { result: 'FAIL', head: git('rev-parse', 'HEAD'), diffSha256: createHash('sha256').update(git('diff', 'HEAD')).digest('hex'),
  qualification: 'One 390px actual main.tsx/UI sign-in/default routes and unmodified production readiness factory, canonical Auth/HTTP and incumbent World/Phaser. Controlled HTTP/C05 and synthetic geometric terrain only. No constructed harness Auth/controller/readiness. Live backend, provider, physical device, multiuser and QA08 NOT_RUN.', errors: [], stages: [] }
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce' })
  page = await context.newPage()
  page.on('pageerror', error => report.errors.push(error.message))
  const state = () => page.evaluate(() => window.__SHARING_SAFETY__?.state())
  const mode = value => page.evaluate(v => window.__SHARING_SAFETY__.mode(v), value)
  const shot = async name => { await page.screenshot({ path: resolve(evidence, `${name}-390.png`), fullPage: true }); report.stages.push(name) }
  const writes = async () => (await state()).requests.filter(r => r.method !== 'GET' && /social-requests|\/reports$|\/me\/blocks\//.test(r.path))
  const blocks = () => page.getByRole('region', { name: 'Blocked users', exact: true })
  const reducePanel = async () => {
    const reduce = page.getByRole('button', { name: 'Reduce panel', exact: true })
    if (await reduce.isVisible()) await reduce.click()
  }
  const people = async () => {
    await page.getByRole('link', { name: 'Meet', exact: true }).click()
    await reducePanel()
    await page.getByText('More actions', { exact: true }).click()
    await page.getByRole('navigation', { name: 'Meet sections', exact: true }).getByRole('link', { name: 'People', exact: true }).click()
    await reducePanel()
    await expect(page.getByRole('heading', { name: 'PEOPLE', exact: true })).toBeVisible()
  }
  const profile = async recipient => { await page.locator(`a[href="/people/${recipient}"]`).click(); await reducePanel(); await expect(page.getByRole('button', { name: 'Request sharing', exact: true })).toBeVisible() }
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/app/social-composition/__tests__/sharing-safety-default/native.html`)
  stage = 'actual UI sign-in'
  await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Password', { exact: true }).fill('fixture-password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Reduce panel', exact: true })).toBeVisible()
  await reducePanel()
  await expect.poll(async () => (await state())?.canvases).toBe(1)
  await expect(page.getByText('This section is unavailable for the current account or event.', { exact: true })).toBeVisible()
  const before = await state()
  const admissionIndex = before.requests.findIndex(r => r.path === '/api/auth/enrollment/admission')
  assert(admissionIndex >= 0)
  assert(before.requests.findIndex(r => r.path.endsWith('/bootstrap')) > admissionIndex)
  assert.equal(before.requests.filter(r => /\/intros$|\/me\/(intro|age-status|notifications|social-handles|social-requests)$/.test(r.path)).length, 0)
  assert.equal(before.requests.filter(r => r.path.endsWith('/feed')).length, 0)
  await page.evaluate(() => window.__SHARING_SAFETY__.baseline())
  await expect(page.getByRole('heading', { name: 'PEOPLE', exact: true })).toBeVisible()
  const recipient = await page.evaluate(() => window.__SHARING_SAFETY__.recipient)
  await profile(recipient)
  stage = 'sharing offer'
  await page.getByRole('button', { name: 'Request sharing', exact: true }).click()
  await reducePanel()
  const send = page.getByRole('button', { name: 'Send sharing request', exact: true })
  await expect(send).toBeDisabled(); assert.equal((await writes()).length, 0)
  await page.getByRole('checkbox', { name: /I consent to share my Instagram/ }).check()
  await shot('explicit-sharing-consent'); await send.click()
  await expect(page.getByText('The server confirmed this change.', { exact: true })).toBeVisible()
  assert.equal((await writes()).length, 1); assert.equal((await state()).requests.filter(r => r.path.includes('/reveal')).length, 0)
  assert.equal((await state()).sameHost, true); assert.equal((await state()).sameCanvas, true)
  stage = 'USER report original recovery'
  await people(); await profile(recipient); await page.getByRole('link', { name: 'Report profile', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Submit report', exact: true })).toBeVisible()
  await mode('unknown'); await page.getByRole('button', { name: 'Submit report', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Check original request status', exact: true })).toBeVisible()
  const originalReport = (await writes())[1]; assert(originalReport.key)
  await page.getByRole('button', { name: 'Close report', exact: true }).click()
  await page.getByRole('link', { name: 'Me', exact: true }).click()
  await page.getByText('Profile options', { exact: true }).click()
  await page.getByRole('link', { name: 'Blocked users', exact: true }).click()
  await reducePanel()
  await expect(blocks()).toBeVisible()
  await mode('resolved'); await page.getByRole('button', { name: 'Check original request status', exact: true }).click()
  await expect(page.getByText('Report submitted and its current status verified.', { exact: true })).toBeVisible()
  assert.equal((await writes()).length, 2)
  const statuses = (await state()).requests.filter(r => r.path.endsWith('/commands'))
  assert.equal(statuses.length, 1)
  assert.equal(new URL(statuses[0].url).searchParams.get('idempotency_key'), originalReport.key)
  assert.equal(new URL(statuses[0].url).searchParams.get('operation_id'), 'createConcertReport')
  await shot('original-report-recovered')
  stage = 'Profile block and unblock'
  await mode('valid'); await people(); await profile(recipient)
  await page.getByRole('button', { name: 'Block user', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Confirm block', exact: true })).toBeVisible()
  assert.equal((await writes()).length, 2); await shot('block-confirmation')
  const beforeBlock = (await state()).policyRevision
  await page.getByRole('button', { name: 'Confirm block', exact: true }).click()
  await expect(blocks()).toBeVisible()
  await reducePanel()
  await expect.poll(async () => (await state()).policyRevision).toBe(beforeBlock + 1)
  await expect.poll(async () => (await state()).frames.some(f => f.type === 'policy.baseline' && f.policyRevision === beforeBlock + 1)).toBe(true)
  await page.getByRole('button', { name: 'Unblock user', exact: true }).click()
  await page.getByRole('button', { name: 'Confirm unblock', exact: true }).click()
  await expect(page.getByText('No blocked users on this page.', { exact: true })).toBeVisible()
  await reducePanel()
  await expect.poll(async () => (await state()).policyRevision).toBe(beforeBlock + 2)
  await expect.poll(async () => (await state()).frames.some(f => f.type === 'policy.baseline' && f.policyRevision === beforeBlock + 2)).toBe(true)
  await shot('unblocked-consent-remains-revoked')
  stage = 'actual default WorldHelp binding'
  await page.getByRole('link', { name: 'World', exact: true }).click()
  await page.getByRole('button', { name: 'World tools', exact: true }).click()
  await page.getByRole('link', { name: 'Help', exact: true }).click()
  const help = page.getByRole('article', { name: 'Controls and meeting safety', exact: true })
  await expect(help).toBeVisible()
  await expect(help.getByRole('heading', { name: 'One world, at your pace', exact: true })).toBeVisible()
  for (const name of ['Move around', 'Look around', 'Virtual is not physical', 'Meet with care']) await expect(help.getByRole('heading', { name, exact: true })).toBeVisible()
  await expect(page.getByText('How to play is not available yet', { exact: true })).toHaveCount(0)
  const destinations = help.getByRole('navigation', { name: 'Explore after reading', exact: true })
  for (const name of ['Browse places', 'Return to world']) {
    const link = destinations.getByRole('link', { name, exact: true }); await link.scrollIntoViewIfNeeded(); await expect(link).toBeVisible()
    assert((await link.boundingBox()).height >= 44)
  }
  await page.getByRole('button', { name: 'Back to previous panel', exact: true }).click()
  await expect(page.getByRole('button', { name: 'World tools', exact: true })).toBeVisible()
  report.stages.push('actual-default-world-help')
  const final = await state()
  assert.deepEqual((await writes()).map(r => r.method), ['POST', 'POST', 'PUT', 'DELETE'])
  assert.equal(final.peakCanvases, 1); assert.equal(final.peakSockets, 1)
  assert.equal(final.requests.filter(r => r.path === '/api/auth/login').length, 1)
  assert(final.requests.filter(r => r.path.startsWith('/api/concert/') || r.path === '/api/auth/enrollment/admission').every(r => r.authorized && r.cache === 'no-store'))
  assert.equal(final.requests.filter(r => /\/feed$|\/reveal|\/auth\/refresh/.test(r.path)).length, 0)
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  assert.deepEqual(report.errors, [])
  report.final = final; report.originalReportKey = originalReport.key
  await context.close(); context = null; await browser.close(); browser = null
  report.browserClosed = true; report.result = 'PASS'
} catch (error) {
  report.failure = { stage, error: String(error), state: await page?.evaluate(() => window.__SHARING_SAFETY__?.state()), text: await page?.locator('body').innerText() }
  await page?.screenshot({ path: resolve(evidence, 'failure.png') }); process.exitCode = 1
} finally {
  await context?.close(); await browser?.close(); await server.close()
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n')
}
