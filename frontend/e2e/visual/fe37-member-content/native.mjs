// Actual default App under controlled frozen HTTP; no actual backend acceptance.
import assert from 'node:assert/strict'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { root, frontend, requireDependency, aliases } from './config.mjs'
import { createMemberContentServer, memberIds, id, wire } from '../../../src/app/operations-composition/__tests__/member-content-production/server.ts'
const { createServer } = await import(requireDependency.resolve('vite'))
const { default: react } = await import(requireDependency.resolve('@vitejs/plugin-react'))
const { chromium, expect } = requireDependency('@playwright/test')
const out = process.env.MCR_FE37_OUTPUT, temporary = process.env.MCR_FE37_TMP, port = Number(process.env.MCR_OPS_PORT)
assert(out && temporary && port === 5371, 'Fresh finite guarded offer required')
const plan = process.env.MCR_FE37_NATIVE_PLAN ?? 'full'
assert(['full', 'remainder'].includes(plan))
await mkdir(out, { recursive: true })
const report = { status: 'FAIL', source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  qualification: 'Actual main/router/AdminBoundary/default composition/current controllers and registry; controlled HTTP. Canonical CSS, no PI01 overlay or Photo2 claim. Real producer/EOF/downstream world session/physical device NOT_RUN.',
  plan, cases: [], captures: [], errors: [], cleanup: {} }
const server = await createServer({ root: frontend, configFile: false, logLevel: 'error', plugins: [react()],
  cacheDir: resolve(temporary, 'vite-native'), resolve: { alias: aliases },
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(wire.event) },
  server: { host: '127.0.0.1', port, strictPort: true, fs: { allow: [root, process.env.MCR_FE37_DEPS_FRONTEND] } },
  optimizeDeps: { include: ['react', 'react-dom/client', 'react/jsx-runtime', 'react-router-dom', '@tanstack/react-query', 'zod', 'ky', 'phaser'] } })
let browser, context, page, stage = 'start'
async function capture(name) { const file = name + '.png'; await page.screenshot({ path: resolve(out, file), fullPage: true }); report.captures.push(file) }
const navigate = async path => page.evaluate(path => { history.pushState(null, '', path); dispatchEvent(new PopStateEvent('popstate')) }, path)
async function start(width, options = {}) {
  const controlled = createMemberContentServer(); controlled.state.role = options.denied ? null : options.operator ? 'OPERATOR' : 'MODERATOR'
  controlled.state.settingsStatus = options.operator ? 403 : 200
  if (options.missingProfile) controlled.moderation.memberListOverride = { items: [{ ...controlled.members.get(memberIds[0]), profile: null }], next_cursor: null }
  if (options.deleted) controlled.content[0] = { ...controlled.content[0], state: 'DELETED' }
  context = await browser.newContext({ viewport: { width, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 1, reducedMotion: 'reduce' })
  page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
  await page.route(url => url.pathname.startsWith('/api/'), async route => {
    const r = route.request()
    try { const response = await controlled.handle(new Request(r.url(), { method: r.method(), headers: r.headers(), ...(r.postData() ? { body: r.postData() } : {}) }))
      await route.fulfill({ status: response.status, headers: Object.fromEntries(response.headers), body: await response.text() })
    } catch { await route.abort('failed') }
  })
  await page.goto(`http://127.0.0.1:${port}/login`)
  await page.getByLabel('Email', { exact: true }).fill('review@example.com'); await page.getByLabel('Password', { exact: true }).fill('TEST_ONLY_password')
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click(); await page.waitForURL('**/world')
  await navigate(options.route ?? `/admin/users/${memberIds[0]}`)
  return controlled
}
async function bounds() {
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  assert.equal(await page.locator('main main').count(), 0)
  assert.equal(await page.getByText('PRIVATE_PROFILE_BIO', { exact: true }).count(), 0)
  const sizes = await page.locator('.moderation button:not(:disabled),.moderation select,.moderation textarea').evaluateAll(elements => elements.map(e => {
    const r = e.getBoundingClientRect(); return { text: e.textContent, width: r.width, height: r.height }
  }))
  for (const size of sizes) assert(size.width >= 44 && size.height >= 44, JSON.stringify(size))
}
async function prepare(action) {
  await page.getByLabel('Action', { exact: true }).selectOption(action)
  await page.getByLabel('Reason for this action', { exact: true }).fill('Reviewed the factual moderation reason.')
  await page.keyboard.press('Tab'); await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeFocused()
  await page.keyboard.press('Enter'); await expect(page.getByRole('button', { name: 'Confirm action', exact: true })).toBeFocused()
}
const confirm = async () => page.getByRole('button', { name: 'Confirm action', exact: true }).click()
async function contentSelect(target) { await page.getByRole('button', { name: `Review ${target}`, exact: true }).click(); await expect(page.locator('#moderation-record')).toBeFocused() }
const selected = () => page.locator('[aria-labelledby="moderation-record"]')
try {
  await server.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--disable-background-networking', '--disable-component-update'] }); report.browser = browser.version()
  for (const width of plan === 'full' ? [360, 390, 430] : []) {
    stage = `${width}/member`; const controlled = await start(width, { operator: width === 430 })
    await expect(page.getByRole('heading', { name: 'Review Review member 1', exact: true })).toBeFocused(); await bounds(); await capture(`member-${width}`)
    await prepare('SUSPEND'); await capture(`member-confirm-${width}`)
    await page.getByRole('button', { name: 'Cancel confirmation', exact: true }).click(); await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeFocused()
    assert.equal(controlled.writes().length, 0); controlled.moderation.mode = 'pending'; await prepare('SUSPEND'); await confirm()
    await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeVisible(); await capture(`member-pending-${width}`)
    const key = controlled.writes()[0].headers.get('Idempotency-Key')
    stage = `${width}/route-survival`; await navigate(`/admin/users/${memberIds[1]}`)
    await expect(page.getByRole('heading', { name: 'Review Review member 2', exact: true })).toBeVisible()
    await navigate('/admin/content'); await expect(page.getByRole('heading', { name: 'Content review', exact: true })).toBeVisible(); await bounds(); await capture(`content-${width}`)
    await navigate('/admin/reports'); await expect(page.getByRole('heading', { name: 'Report queue', exact: true })).toBeVisible()
    await navigate(`/admin/users/${memberIds[0]}`); await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeVisible()
    controlled.moderation.statusHttp = 404; await page.getByRole('button', { name: 'Check original command', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText('outcome remains unknown'); await expect(page.getByLabel('Action', { exact: true })).toBeDisabled(); assert.equal(controlled.writes().length, 1)
    await capture(`member-unknown-${width}`)
    controlled.moderation.statusHttp = 200; controlled.settle(key, 'SUCCEEDED'); await page.getByRole('button', { name: 'Check original command', exact: true }).click()
    await expect(selected().getByText('SUSPENDED', { exact: true })).toBeVisible(); await capture(`member-suspended-${width}`)
    controlled.moderation.mode = 'success'; await page.getByRole('button', { name: 'Refresh records', exact: true }).click()
    await expect(page.getByLabel('Action', { exact: true })).toBeEnabled(); await prepare('UNSUSPEND'); await confirm(); await expect(selected().getByText('ACTIVE', { exact: true })).toBeVisible()
    stage = `${width}/content-actions`; await navigate('/admin/content'); await expect(page.getByRole('heading', { name: 'Content review', exact: true })).toBeVisible()
    for (const row of width === 390 ? [...controlled.content] : [controlled.content[0]]) {
      await page.getByRole('button', { name: 'Refresh records', exact: true }).click(); await contentSelect(row.target_id)
      await prepare('HIDE'); await confirm(); await expect(selected().getByText('HIDDEN', { exact: true })).toBeVisible(); await capture(`content-${row.target_type.toLowerCase()}-hidden-${width}`)
      await page.getByRole('button', { name: 'Refresh records', exact: true }).click(); await contentSelect(row.target_id)
      await prepare('RESTORE'); await confirm(); await expect(selected().getByText('VISIBLE', { exact: true })).toBeVisible()
    }
    await capture(`content-restored-${width}`); await bounds()
    const status = controlled.requests.filter(r => new URL(r.url).pathname.endsWith('/commands'))
    assert(status.every(r => new URL(r.url).searchParams.get('operation_id') === 'moderateConcertUser' && new URL(r.url).searchParams.get('idempotency_key') === key))
    assert(controlled.requests.filter(r => new URL(r.url).pathname.endsWith('/admin/users')).every(r => memberIds.includes(new URL(r.url).searchParams.get('user_id'))))
    report.cases.push({ width, role: controlled.state.role, settingsStatus: controlled.state.settingsStatus, statusGets: status.length, moderationPosts: controlled.writes().length,
      checks: ['minimal-profile', 'exact-filter', 'confirm-cancel-focus-no-write', 'pending-A-B-content-reports-A-same-key', '404-unknown-disabled', 'corroborated-suspend-unsuspend', 'hide-restore', 'no-nested-main-or-overflow'] })
    await context.close(); context = undefined
  }
  for (const mode of plan === 'full' ? ['failed', 'conflict', 'withdrawal', 'malformed', 'missing-profile', 'deleted', 'denied'] : ['withdrawal', 'malformed', 'missing-profile', 'deleted', 'denied']) {
    stage = `390/${mode}`; const controlled = await start(390, { denied: mode === 'denied', missingProfile: mode === 'missing-profile', deleted: mode === 'deleted',
      route: mode === 'malformed' ? '/admin/users/bad-id' : mode === 'deleted' ? '/admin/content' : `/admin/users/${memberIds[0]}` })
    if (mode === 'denied') { await expect(page.getByRole('heading', { name: 'Review tools unavailable', exact: true })).toBeVisible(); assert(!controlled.requests.some(r => new URL(r.url).pathname.includes('/admin/'))) }
    else if (mode === 'malformed') { await expect(page.getByRole('heading', { name: 'Review unavailable', exact: true })).toBeVisible(); assert(!controlled.requests.some(r => new URL(r.url).pathname.includes('/admin/users'))) }
    else if (mode === 'missing-profile') { await expect(page.getByRole('alert')).toContainText('request failed'); assert.equal(await page.locator('.moderation tbody tr').count(), 0) }
    else if (mode === 'deleted') { await expect(page.getByRole('heading', { name: 'Content review', exact: true })).toBeVisible(); await contentSelect(id(701)); assert.equal(await page.getByLabel('Action', { exact: true }).count(), 0) }
    else {
      await expect(page.getByRole('heading', { name: 'Review Review member 1', exact: true })).toBeVisible()
      if (mode === 'withdrawal') { controlled.state.role = null; controlled.replaceDecision(); await page.getByRole('button', { name: 'Check current access', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Review tools unavailable', exact: true })).toBeVisible(); assert.equal(await page.locator('.moderation').count(), 0) }
      else { controlled.moderation.mode = mode === 'failed' ? 'pending' : 'conflict'; await prepare('SUSPEND'); await confirm()
        if (mode === 'failed') { await expect(page.getByRole('button', { name: 'Check original command', exact: true })).toBeVisible(); controlled.settle(controlled.writes()[0].headers.get('Idempotency-Key'), 'FAILED'); await page.getByRole('button', { name: 'Check original command', exact: true }).click() }
        await expect(page.getByRole('alert')).toContainText(mode === 'failed' ? 'server recorded failure' : 'record changed'); await expect(page.getByLabel('Action', { exact: true })).toBeDisabled()
      }
    }
    await capture(`state-${mode}-390`); report.cases.push({ name: mode, status: 'PASS' }); await context.close(); context = undefined
  }
  assert.deepEqual(report.errors, []); report.status = 'PASS'
} catch (error) { report.failure = { stage, error: String(error) }; if (page && !page.isClosed()) await capture('failure'); throw error }
finally { await context?.close(); await browser?.close(); await server.close(); report.cleanup = { browserClosed: true, serverClosed: true }; await writeFile(resolve(out, 'native.json'), JSON.stringify(report, null, 2) + '\n') }
