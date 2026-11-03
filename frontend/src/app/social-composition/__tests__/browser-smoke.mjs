import assert from 'node:assert/strict'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { build, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const source = resolve(import.meta.dirname, '..'), root = resolve(source, '../../..'), out = resolve(import.meta.dirname, 'evidence')
await mkdir(out, { recursive: true }); const bundle = await mkdtemp(resolve(tmpdir(), 'mcr-social-'))
const post = JSON.parse(await readFile(resolve(root, '../contracts/fixtures/domain/public-post.json'), 'utf8'))
const intro = JSON.parse(await readFile(resolve(root, '../contracts/fixtures/domain/public-intro.json'), 'utf8'))
const event = post.event_id, user = post.author.user_id, uid = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const report = { result: 'FAIL', scope: 'Production-built social adapter fixture with real canonical controllers/Auth/HTTP/privacy, test-only world lease and server. No actual product root wiring, real game/socket, BE33 or IN03/device acceptance.', widths: [], checks: [] }
const paths = ['ports.ts', 'routes.ts', 'owner.ts', 'provider.tsx', 'panels.tsx', 'composition.css', 'navigation.tsx', 'index.ts', '__tests__/preview.tsx', '__tests__/fixtures.ts', '__tests__/preview.css', '__tests__/browser-smoke.mjs']
report.sourceHashes = Object.fromEntries(await Promise.all(paths.map(async p => [p, createHash('sha256').update(await readFile(resolve(source, p))).digest('hex')])))
let server, browser
try {
  await build({ root, logLevel: 'error', build: { outDir: bundle, emptyOutDir: true, rolldownOptions: { input: resolve(import.meta.dirname, 'preview.html') } } })
  server = await preview({ root, logLevel: 'error', build: { outDir: bundle }, preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), errors = []
    let deniedProfile = false, writes = 0
    page.on('pageerror', e => errors.push(e.message))
    await page.route('**/api/**', async route => {
      const r = route.request(), p = new URL(r.url()).pathname
      const respond = (data, status = 200) => route.fulfill({ status, contentType: 'application/json', headers: { 'Cache-Control': 'no-store' }, body: JSON.stringify(data) })
      assert.equal(r.headers().authorization, 'Bearer SYNTHETIC_SOCIAL')
      if (r.method() !== 'GET') { writes++; return route.abort('failed') }
      if (p.endsWith('/unread-count')) return respond({ actor_id: user, event_id: event, unread_total: 37, as_of: '2026-11-04T00:00:00Z' })
      if (p.endsWith('/notifications')) return respond({ items: [{ notification_id: uid(90), event_id: event, kind: 'FEED', target: { target_type: 'POST', target_id: post.post_id }, created_at: '2026-11-01T00:00:00Z', read_at: null, revision: 1 }], next_cursor: null })
      if (p.endsWith('/me/age-status')) return respond({ user_id: user, event_id: event, status: 'UNKNOWN', verified_at: null, expires_at: null, verification_available: false })
      if (p.endsWith('/me/intro')) return respond({ intro_id: uid(80), event_id: event, owner_id: user, text: 'Private owner draft', age_bounds: { minimum_age: null, maximum_age_exclusive: null }, expires_at: null, state: 'DRAFT', revision: 1, created_at: '2026-11-01T00:00:00Z' })
      if (p.endsWith('/intros')) return respond({ items: [intro], next_cursor: null })
      if (p.includes('/people/')) return deniedProfile ? respond({ code: 'RESOURCE_UNAVAILABLE', message: 'private denied body', details: { reason: 'RESOURCE_UNAVAILABLE' } }, 404) : respond(intro.author)
      if (p.endsWith('/notices')) return respond({ items: [], next_cursor: null })
      if (p.endsWith('/feed')) return respond({ items: [post], next_cursor: null })
      if (p.includes('/posts/')) return respond({ post, comments: [], next_cursor: null })
      return respond({ code: 'RESOURCE_UNAVAILABLE', message: 'private body', details: { reason: 'RESOURCE_UNAVAILABLE' } }, 404)
    })
    await page.goto(`${origin}/src/app/social-composition/__tests__/preview.html`)
    await expect(page.getByText(post.text, { exact: true })).toBeVisible()
    const host = await page.locator('[data-world-host]').elementHandle(), initial = await page.evaluate(() => window.__SOCIAL__.stats())
    assert.equal(initial.peakGames, 1); assert.equal(initial.peakConnections, 1)
    await page.keyboard.press('Tab'); assert(await page.evaluate(() => getComputedStyle(document.activeElement).outlineStyle !== 'none'))
    await page.getByRole('button', { name: /^Open thread/ }).click()
    await page.getByRole('button', { name: 'Edit post', exact: true }).click()
    await page.getByLabel('Post text', { exact: true }).fill('Discard this edit')
    await page.getByRole('button', { name: 'Discard edit', exact: true }).click()
    await expect(page.getByLabel('Post text', { exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Back to post', exact: true }).click()
    await page.getByRole('button', { name: 'Compose', exact: true }).click(); await page.getByLabel('Post text', { exact: true }).fill('Retained social draft')
    await page.getByRole('button', { name: 'Inbox', exact: true }).click(); await expect(page.getByText('37 unread in this event', { exact: true })).toBeVisible()
    await expect(page.getByLabel('Canonical unread total')).toHaveText('37'); await page.screenshot({ path: resolve(out, `${width}-inbox.png`), fullPage: true })
    await page.getByRole('button', { name: 'Open update', exact: true }).click(); await expect(page.getByRole('heading', { name: 'THREAD', exact: true })).toBeVisible()
    assert((await page.evaluate(() => window.__SOCIAL__.requests())).filter(r => r.path.endsWith(`/posts/${post.post_id}`)).length >= 2)
    await page.getByRole('button', { name: 'Edit post', exact: true }).click(); await expect(page.getByText('Keep or discard the current edit before changing its target. Your draft is retained.')).toBeVisible()
    await page.getByRole('button', { name: 'Compose', exact: true }).click(); await expect(page.getByLabel('Post text', { exact: true })).toHaveValue('Retained social draft')
    await page.getByRole('button', { name: 'People', exact: true }).click(); await page.getByRole('link', { name: intro.author.display_name, exact: true }).click()
    await expect(page.getByRole('heading', { name: intro.author.display_name, exact: true })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Request sharing' })).toBeDisabled(); await expect(page.getByRole('button', { name: 'Block user' })).toBeDisabled()
    deniedProfile = true; await page.getByRole('button', { name: 'People', exact: true }).click(); await page.getByRole('link', { name: intro.author.display_name, exact: true }).click()
    await expect(page.getByText('This profile is unavailable.', { exact: true })).toBeVisible(); await expect(page.getByRole('heading', { name: intro.author.display_name, exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Intro', exact: true }).click(); const text = page.getByLabel('Introduction', { exact: true }); await expect(text).toBeEnabled(); await text.fill('Keep owner introduction')
    await page.getByRole('button', { name: 'Age', exact: true }).click(); await expect(page.getByRole('button', { name: 'Start verification' })).toBeDisabled()
    await page.getByRole('button', { name: 'Intro', exact: true }).click(); await expect(text).toHaveValue('Keep owner introduction')
    await page.getByRole('button', { name: 'Requests', exact: true }).click(); await expect(page.getByText('This section is unavailable for the current account or event.')).toBeVisible()
    assert.equal(await host.evaluate(el => el === document.querySelector('[data-world-host]')), true)
    assert.equal((await page.evaluate(() => window.__SOCIAL__.stats())).mounts, initial.mounts)
    await page.getByRole('button', { name: 'Compose', exact: true }).click(); await page.getByRole('button', { name: 'Publish post', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Check command status' })).toBeVisible(); const command = await page.evaluate(() => window.__SOCIAL__.command())
    await page.getByRole('button', { name: 'Inbox', exact: true }).click(); await expect(page.getByText('Resolve the current action before leaving this panel. Its original command is retained.')).toBeVisible()
    assert.deepEqual(await page.evaluate(() => window.__SOCIAL__.command()), command); assert.equal(writes, 1)
    await page.getByRole('link', { name: 'Inbox router link', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Stay with current action' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Continue navigation' })).toBeDisabled()
    assert.equal(await page.evaluate(() => window.__SOCIAL__.current()), '/feed/new')
    await page.getByRole('button', { name: 'Stay with current action' }).click()
    await page.evaluate(() => { void window.__SOCIAL__.back() })
    await expect(page.getByRole('button', { name: 'Stay with current action' })).toBeVisible()
    assert.equal(await page.evaluate(() => window.__SOCIAL__.current()), '/feed/new')
    assert.deepEqual(await page.evaluate(() => window.__SOCIAL__.command()), command); assert.equal(writes, 1)

    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)); assert(await page.locator('button').evaluateAll(bs => bs.every(b => b.getBoundingClientRect().height >= 44)))
    await page.screenshot({ path: resolve(out, `${width}-pending.png`), fullPage: true })
    await page.evaluate(() => window.__SOCIAL__.policy()); await expect(page.getByLabel('Post text', { exact: true })).toHaveCount(0)
    await expect(page.getByLabel('Canonical unread total')).toHaveText('Unavailable'); assert.equal((await page.evaluate(() => window.__SOCIAL__.stats())).games, 0)
    assert.deepEqual(errors, []); report.widths.push(width); await page.close()
  }
  report.result = 'PASS'; report.checks = ['real canonical controller and fullcount composition', 'StrictMode peak one instrumented world lease; same host/mount through navigation', 'discarded edit does not reopen; composer and intro drafts retained, including edit-button guard', 'fresh inbox link preflight plus destination reread', 'denied profile evicts old content', 'sharing/block/provider/requests absent failclosed', 'unresolved command retains key/draft and blocks navigation with one write', 'data-router Link and history POP blocked without changing path/key/draft', 'policy clears private DOM/count and world lease', 'keyboard focus/44px/no horizontal overflow/no browser errors']
} finally { await browser?.close(); await server?.httpServer.close(); await rm(bundle, { recursive: true, force: true }); await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report, null, 2))
