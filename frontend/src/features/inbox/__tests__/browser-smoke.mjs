import assert from 'node:assert/strict'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { build, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const feature = resolve(import.meta.dirname, '..'), root = resolve(feature, '../../..'), out = resolve(import.meta.dirname, 'evidence')
await mkdir(out, { recursive: true })
const bundle = await mkdtemp(resolve(tmpdir(), 'mcr-fe35-'))
const report = { result: 'FAIL', scope: 'FE35 production-built isolated React UI; deterministic HTTP fixtures with canonical Auth/privacy. No live BE33, root integration or device acceptance.', widths: [], checks: [] }
report.sourceHashes = Object.fromEntries(await Promise.all(['model/controller.ts', 'model/contract.ts', 'model/targets.ts', 'ui/Inbox.tsx', 'ui/inbox.css', '__tests__/preview.tsx', '__tests__/browser-smoke.mjs'].map(async p => [p, createHash('sha256').update(await readFile(resolve(feature, p))).digest('hex')])))
const uid = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const event = uid(2), notification = { notification_id: uid(3), event_id: event, kind: 'PARTY', target: { target_type: 'PARTY', target_id: uid(4) }, created_at: '2026-11-04T00:00:00Z', read_at: null, revision: 1 }
let server, browser
try {
  await build({ root, logLevel: 'error', build: { outDir: bundle, emptyOutDir: true, rolldownOptions: { input: resolve(import.meta.dirname, 'preview.html') } } })
  server = await preview({ root, logLevel: 'error', build: { outDir: bundle }, preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430, 1280]) {
    const page = await browser.newPage({ viewport: { width, height: 900 } }), errors = [], writes = []
    let mode = 'ready', read = false, newArrival = false, pending = null
    page.on('pageerror', e => errors.push(e.message))
    await page.route('**/api/**', async route => {
      const r = route.request(), path = new URL(r.url()).pathname
      const respond = (body, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) })
      if (r.method() === 'PATCH') {
        writes.push({ path, key: r.headers()['idempotency-key'], body: r.postDataJSON() })
        if (mode === 'pending') {
          pending = { receipt_id: uid(8), actor_id: uid(1), event_id: event, operation_id: path.endsWith('read-all') ? 'markAllConcertNotifications' : 'markConcertNotification', idempotency_key: writes.at(-1).key, state: 'PENDING', resource_id: null, resource_revision: null, result_status: null, failure_reason: null, created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-05T00:00:00Z' }
          return respond(pending, 202)
        }
        read = true; newArrival = path.endsWith('read-all')
        return respond(newArrival ? { read_count: 1, read_at: '2026-11-04T00:01:00Z' } : { ...notification, read_at: '2026-11-04T00:01:00Z', revision: 2 })
      }
      if (path.endsWith('/commands')) { read = true; return respond({ ...pending, state: 'SUCCEEDED', resource_id: uid(3), resource_revision: 2, result_status: 200 }) }
      if (!path.endsWith('/notifications')) return respond({ code: 'RESOURCE_UNAVAILABLE', message: 'private raw detail', details: { reason: 'RESOURCE_UNAVAILABLE' } }, 404)
      if (mode === 'offline') return route.abort('failed')
      const rows = mode === 'empty' ? [] : [{ ...notification, ...(read ? { read_at: '2026-11-04T00:01:00Z', revision: 2 } : {}) }, ...(newArrival ? [{ ...notification, notification_id: uid(9), target: null, created_at: '2026-11-04T00:02:00Z' }] : [])]
      return respond({ items: new URL(r.url()).searchParams.get('unread_only') === 'true' ? rows.filter(n => !n.read_at) : rows, next_cursor: null })
    })
    await page.goto(`${origin}/src/features/inbox/__tests__/preview.html`)
    await expect(page.getByText('1 unread shown', { exact: true })).toBeVisible()
    await page.keyboard.press('Tab'); await expect(page.getByRole('checkbox', { name: 'Unread only' })).toBeFocused()
    assert(await page.evaluate(() => getComputedStyle(document.activeElement).outlineStyle !== 'none'))
    assert(await page.locator('button').evaluateAll(buttons => buttons.every(b => b.getBoundingClientRect().height >= 44)))
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    assert(await page.locator('.mcr-inbox').evaluate(root => [...root.querySelectorAll('*')].every(el => {
      const s = getComputedStyle(el), before = getComputedStyle(el, '::before'), after = getComputedStyle(el, '::after')
      return s.borderLeftWidth === s.borderRightWidth && [before, after].every(p => p.content === 'none' || p.content === 'normal')
    })))
    await page.screenshot({ path: resolve(out, `${width}-inbox.png`), fullPage: true })
    await page.getByRole('button', { name: 'Open update', exact: true }).click()
    await expect(page.getByText('This destination was removed, changed, or cannot be opened.')).toBeVisible()
    assert.equal(await page.evaluate(() => window.__FE35__.navigated()), ''); assert.equal(writes.length, 0)
    mode = 'pending'; await page.getByRole('button', { name: 'Mark as read', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Check command status' })).toBeVisible(); await expect(page.getByText('1 unread shown', { exact: true })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Mark all as read' })).toBeDisabled()
    await page.screenshot({ path: resolve(out, `${width}-pending.png`), fullPage: true })
    await page.getByRole('button', { name: 'Check command status' }).click(); await expect(page.getByText('0 unread shown', { exact: true })).toBeVisible(); assert.equal(writes.length, 1)
    assert.equal(writes[0].body.expected_revision, 1); assert.match(writes[0].key, /^[0-9a-f-]{36}$/)
    mode = 'empty'; await page.getByRole('button', { name: 'Refresh inbox' }).click(); await expect(page.getByText('Your inbox is empty.')).toBeVisible()
    mode = 'offline'; await page.getByRole('button', { name: 'Refresh inbox' }).click(); await expect(page.getByText('Connection unavailable. Reconnect, then refresh the inbox.')).toBeVisible()
    mode = 'ready'; read = false; await page.getByRole('button', { name: 'Refresh inbox' }).click(); await expect(page.getByText('1 unread shown', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Mark all as read' }).click(); await expect(page.getByText('1 unread shown', { exact: true })).toBeVisible(); await expect(page.getByRole('article')).toHaveCount(2); assert.equal(writes.length, 2)
    await page.evaluate(() => window.__FE35__.invalidate()); await expect(page.getByRole('article')).toHaveCount(0)
    await page.getByRole('button', { name: 'Refresh inbox' }).click(); await expect(page.getByRole('article')).toHaveCount(2)
    await page.evaluate(() => window.__FE35__.available(false)); await expect(page.getByRole('article')).toHaveCount(0)
    await page.evaluate(() => window.__FE35__.available(true)); await expect(page.getByRole('article')).toHaveCount(2)
    await page.evaluate(() => window.__FE35__.logout()); await expect(page.getByRole('article')).toHaveCount(0); await expect(page.getByText('Inbox is unavailable for the current account or event.')).toBeVisible()
    assert.deepEqual(errors, []); report.widths.push(width); await page.close()
  }
  report.result = 'PASS'; report.checks = ['production fixture bundle', '44px buttons and keyboard focus', 'no horizontal overflow or left accents', 'fresh target authorization denied safely without writes', 'pending command does not optimistically mark read', 'explicit status recovery without mutation replay', 'empty and network error', 'read-all cutoff leaves new arrival unread', 'invalidation/availability/logout clear private rows', 'no browser errors']
} finally {
  await browser?.close(); await server?.httpServer.close(); await rm(bundle, { recursive: true, force: true })
  await writeFile(resolve(out, 'browser-report.json'), JSON.stringify(report, null, 2) + '\n')
}
console.log(JSON.stringify(report, null, 2))
