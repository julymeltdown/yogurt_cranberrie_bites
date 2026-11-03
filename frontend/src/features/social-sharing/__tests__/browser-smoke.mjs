// Native same-origin HTTP qualification service; deterministic metadata only.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const feature = resolve(import.meta.dirname, '..'), root = resolve(feature, '../../../..'), out = resolve(import.meta.dirname, 'evidence')
await mkdir(out, { recursive: true })
const approved = JSON.parse(await readFile(resolve(root, 'contracts/fixtures/domain/approved-social-request.json'), 'utf8'))
const { requester_id: requester, recipient_id: recipient, intro_id: intro, event_id: event } = approved
const id = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
let handles, requests, receipts, sequence, uncertain, calls, writes
function reset() { handles = new Map([requester, recipient].map(user => [user, [{ user_id: user, platform: 'INSTAGRAM', version: user === requester ? 2 : 3, configured: true }, { user_id: user, platform: 'X', version: user === requester ? 4 : 5, configured: true }]])); requests = []; receipts = new Map(); sequence = 100; uncertain = false; calls = []; writes = 0 }
const server = await createServer({ root: resolve(root, 'frontend'), logLevel: 'error', plugins: [{ name: 'test-only-social-sharing-http', configureServer(vite) {
  vite.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/concert/')) return next()
    try {
      const url = new URL(req.url, `http://${req.headers.host}`), path = url.pathname, method = req.method, actor = req.headers.authorization === 'Bearer social-test-only-token' ? requester : recipient
      assert(['Bearer social-test-only-token', 'Bearer social-recipient-token'].includes(req.headers.authorization))
      res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
      const send = (status, body) => { res.statusCode = status; res.end(JSON.stringify(body)) }
      calls.push({ method, path }) // Deliberately never record plaintext bodies or reveal responses.
      if (path.endsWith('/__fixture_uncertain')) { uncertain = true; return send(200, {}) }
      const request = requests.find(r => path.includes(r.request_id))
      if (method === 'GET') {
        if (path.endsWith('/social-handles')) return send(200, { items: handles.get(actor) })
        if (path.endsWith('/sharing-offer')) return send(200, { intro_id: intro, intro_revision: 1, recipient_id: recipient, available_handles: handles.get(recipient).filter(h => h.configured).map(h => ({ platform: h.platform, version: h.version })) })
        if (path.endsWith('/social-requests')) return send(200, { items: requests.filter(r => url.searchParams.get('direction') === 'INCOMING' ? r.recipient_id === actor : r.requester_id === actor), next_cursor: null })
        if (path.endsWith('/commands')) return send(200, receipts.get(url.searchParams.get('idempotency_key')))
        if (path.endsWith('/reveal')) {
          assert.equal(request?.state, 'APPROVED'); const platform = url.searchParams.get('platform'); assert(request.approved_platforms.includes(platform))
          const owner = actor === requester ? recipient : requester, consent = request.consents.find(c => c.platform === platform)
          return send(200, { request_id: request.request_id, platform, owner_id: owner, handle_version: owner === requester ? consent.requester_handle_version : consent.recipient_handle_version, handle: 'synthetic_private_name', grant_revision: 1, expires_at: request.expires_at })
        }
        return request ? send(200, request) : send(404, {})
      }
      assert.equal(req.headers['x-mcr-intent'], 'browser'); assert.match(req.headers['idempotency-key'], /^[0-9a-f-]{36}$/)
      const chunks = []; for await (const chunk of req) chunks.push(chunk)
      const body = chunks.length ? JSON.parse(Buffer.concat(chunks).toString()) : null; writes++
      if (path.includes('/social-handles/')) {
        const handle = handles.get(actor).find(h => h.platform === path.split('/').at(-1)); assert.equal(method === 'DELETE' ? Number(url.searchParams.get('expected_revision')) : body.expected_revision, handle.version)
        if (method === 'PUT') assert.equal(body.handle, 'replacement.name')
        handle.version++; handle.configured = method !== 'DELETE'
        return send(200, handle)
      }
      if (path.endsWith('/social-requests') && method === 'POST') {
        assert.deepEqual(Object.keys(body).sort(), ['consents', 'intro_id', 'intro_revision', 'recipient_id']); assert.equal(JSON.stringify(body).includes('synthetic_private_name'), false)
        const created = { ...approved, request_id: id(++sequence), state: 'PENDING', revision: 1, consents: body.consents, approved_platforms: [] }; requests.unshift(created)
        const key = req.headers['idempotency-key']; receipts.set(key, { receipt_id: id(++sequence), event_id: event, actor_id: actor, operation_id: 'createConcertSocialRequest', idempotency_key: key, state: 'SUCCEEDED', resource_id: created.request_id, resource_revision: 1, failure_reason: null, created_at: approved.created_at, expires_at: '2026-11-08T09:00:00Z', result_status: 200 })
        if (uncertain) { uncertain = false; return send(503, { code: 'UNAVAILABLE', message: 'private-server-error-never-render', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }) }
        return send(200, created)
      }
      assert(request); assert.equal(method === 'DELETE' ? Number(url.searchParams.get('expected_revision')) : body.expected_revision, request.revision)
      if (path.endsWith('/decision')) { assert.equal(actor, recipient); assert.equal(body.recipient_consents, true); assert.deepEqual(body.approved_platforms, ['INSTAGRAM']); request.state = 'APPROVED'; request.approved_platforms = body.approved_platforms }
      else request.state = path.endsWith('/revoke') ? 'REVOKED' : 'CANCELLED'
      request.revision++; return send(200, request)
    } catch (error) { res.statusCode = 500; res.end(JSON.stringify({ fixture_error: String(error) })) }
  })
} }], server: { host: '127.0.0.1', port: 0, strictPort: false } })
const report = { result: 'FAIL', scope: 'FE31 isolated UI + canonical session/privacy + native same-origin deterministic HTTP. No real backend, reciprocal authority, device, QA04 or IN03 acceptance.', widths: [], checks: [], sourceHashes: {} }
report.sourceHashes = Object.fromEntries(await Promise.all(['model/controller.ts', 'model/runtime.ts', 'model/contract.ts', 'ui/SocialSharing.tsx', 'ui/sharing.css', '__tests__/fixtures/preview.tsx', '__tests__/browser-smoke.mjs'].map(async path => [path, createHash('sha256').update(await readFile(resolve(feature, path))).digest('hex')])))
let browser; const errors = []
try {
  await server.listen(); const base = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    reset(); const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    await page.goto(`${base}/src/features/social-sharing/__tests__/fixtures/preview.html`)
    await expect(page.getByRole('textbox', { name: 'Replacement Instagram account name' })).toBeVisible()
    await page.getByRole('button', { name: 'Introduction offer' }).click()
    const instagram = page.getByRole('checkbox', { name: /I consent to share my Instagram/ }), x = page.getByRole('checkbox', { name: /I consent to share my X/ })
    await expect(instagram).not.toBeChecked(); await expect(page.getByRole('button', { name: 'Send sharing request' })).toBeDisabled()
    assert.equal(calls.filter(r => r.path.endsWith('/reveal')).length, 0)
    await instagram.check(); await x.check(); await page.getByRole('button', { name: 'Send sharing request' }).click()
    await expect(page.getByText('The server confirmed this change.')).toBeVisible(); await page.getByRole('button', { name: 'Open created request' }).click(); await expect(page.getByRole('status', { name: '' }).filter({ hasText: 'PENDING' })).toBeVisible()
    await page.getByRole('button', { name: 'Switch fixture actor' }).click(); await page.getByRole('button', { name: 'Open request' }).click()
    const approve = page.getByRole('button', { name: 'Approve selected platforms' }); await expect(approve).toBeDisabled()
    await page.getByRole('checkbox', { name: 'Exchange Instagram account names' }).check(); await expect(approve).toBeDisabled()
    await page.getByRole('checkbox', { name: 'I explicitly consent to share my selected account names with the requester.' }).check(); await approve.click()
    await expect(page.getByRole('button', { name: 'Reveal Instagram' })).toBeVisible(); await expect(page.getByRole('button', { name: 'Reveal X', exact: true })).toHaveCount(0)
    assert.equal(calls.filter(r => r.path.endsWith('/reveal')).length, 0)
    await page.getByRole('button', { name: 'Reveal Instagram' }).focus(); await page.keyboard.press('Enter'); await expect(page.getByText('@synthetic_private_name', { exact: true })).toBeVisible()
    assert.equal(calls.filter(r => r.path.endsWith('/reveal')).length, 1)
    await page.screenshot({ path: resolve(out, `sharing-${width}.png`), fullPage: true })
    assert.deepEqual(await page.evaluate(() => window.__FE31__.counts()), { queries: 0, mutations: 0 })
    await page.getByRole('button', { name: 'Hide account name' }).click(); await expect(page.getByText('@synthetic_private_name', { exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Reveal Instagram' }).click(); await expect(page.getByText('@synthetic_private_name', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Revoke sharing' }).click(); const before = writes; await page.getByRole('button', { name: 'Keep current sharing' }).click(); assert.equal(writes, before)
    await page.getByRole('button', { name: 'Revoke sharing' }).click(); await page.getByRole('button', { name: 'Confirm revocation' }).click()
    await expect(page.getByRole('status').filter({ hasText: 'REVOKED' })).toBeVisible(); await expect(page.getByText('@synthetic_private_name', { exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Switch fixture actor' }).click(); await page.getByRole('button', { name: 'Introduction offer' }).click(); await instagram.check()
    await page.request.get(`${base}/api/concert/events/${event}/__fixture_uncertain`, { headers: { Authorization: 'Bearer social-test-only-token' } })
    await page.getByRole('button', { name: 'Send sharing request' }).click(); await expect(page.getByRole('button', { name: 'Check change status' })).toBeVisible()
    const unknownWrites = writes; await expect(page.getByRole('button', { name: 'Send sharing request' })).toBeDisabled(); await page.getByRole('button', { name: 'Check change status' }).click(); await expect(page.getByText('The server confirmed this change.')).toBeVisible(); assert.equal(writes, unknownWrites)
    await page.getByRole('button', { name: 'Open created request' }).click(); await page.getByRole('button', { name: 'Switch fixture actor' }).click(); await page.getByRole('button', { name: 'Open request' }).first().click()
    await page.getByRole('checkbox', { name: 'Exchange Instagram account names' }).check(); await page.getByRole('checkbox', { name: 'I explicitly consent to share my selected account names with the requester.' }).check(); await page.getByRole('button', { name: 'Approve selected platforms' }).click()
    await page.getByRole('button', { name: 'Reveal Instagram' }).click(); await expect(page.getByText('@synthetic_private_name', { exact: true })).toBeVisible()
    await page.evaluate(() => window.__FE31__.expire()); await expect(page.getByRole('status').filter({ hasText: 'EXPIRED' })).toBeVisible(); await expect(page.getByText('@synthetic_private_name', { exact: true })).toHaveCount(0); await expect(page.getByRole('button', { name: 'Reveal Instagram' })).toHaveCount(0)
    await page.getByRole('button', { name: 'Switch fixture actor' }).click()
    await page.getByRole('button', { name: 'Handle settings', exact: true }).click()
    await page.getByRole('textbox', { name: 'Replacement Instagram account name' }).fill(' Replacement.Name ')
    await page.getByRole('button', { name: 'Replace privately', exact: true }).first().click(); await expect(page.getByRole('textbox', { name: 'Replacement Instagram account name' })).toHaveValue('')
    await page.getByRole('button', { name: 'Remove Instagram', exact: true }).click(); await page.getByRole('button', { name: 'Confirm removal' }).click(); await expect(page.getByRole('textbox', { name: 'New Instagram account name' })).toBeVisible()
    await page.getByRole('textbox', { name: 'New Instagram account name' }).fill('synthetic_draft'); await page.evaluate(() => window.__FE31__.policy()); assert.equal(await page.evaluate(() => window.__FE31__.draft()), '')
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
    assert.equal(await page.evaluate(() => localStorage.length + sessionStorage.length), 0)
    await expect(page.getByText('private-server-error-never-render', { exact: true })).toHaveCount(0)
    report.widths.push({ width, requests: calls.length, writes, reveals: calls.filter(r => r.path.endsWith('/reveal')).length }); await context.close()
  }
  assert.deepEqual(errors, []); report.result = 'PASS'; report.checks = ['no prechecked consent', 'metadata-only create payload', 'no automatic reveal', 'subset approval with reciprocal consent', 'explicit protected reveal by keyboard', 'hide clears plaintext', 'revoke confirmation and immediate scrub', 'uncertain command status without replay', 'expiry clears plaintext and disables reveal', 'handle normalization/replacement/removal', 'policy scrub', 'no private Query/storage entries', 'no overflow or browser errors']
} finally { await browser?.close(); await server.close(); await writeFile(resolve(out, 'browser-report.json'), JSON.stringify(report, null, 2) + '\n') }
console.log(JSON.stringify(report, null, 2))
