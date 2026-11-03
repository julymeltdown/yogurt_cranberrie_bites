// Test-only native same-origin HTTP service; never imported by product modules.
import assert from 'node:assert/strict'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const fixture = JSON.parse(await readFile('../contracts/fixtures/domain/public-post.json', 'utf8'))
const evidence = 'src/features/feed-thread/__tests__/evidence'
await mkdir(evidence, { recursive: true })
let posts = [], comments = [], sequence = 300, writes = 0
const receipts = new Map(), calls = []
const id = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const dev = await createServer({ plugins: [{ name: 'test-only-social-http', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/concert/')) return next()
    const url = new URL(req.url, `http://${req.headers.host}`), path = url.pathname, method = req.method
    assert.equal(req.headers.authorization, 'Bearer test-only-token')
    res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store')
    const send = (status, value) => { res.statusCode = status; res.end(JSON.stringify(value)) }
    calls.push({ method, path })
    if (method === 'GET') {
      if (path.endsWith('/feed')) return send(200, { items: posts, next_cursor: null })
      if (path.endsWith('/notices')) return send(200, { items: [], next_cursor: null })
      if (path.endsWith('/commands')) return send(200, receipts.get(url.searchParams.get('idempotency_key')))
      const postId = path.split('/').at(-1), post = posts.find(p => p.post_id === postId)
      return post ? send(200, { post, comments: comments.filter(c => c.post_id === postId), next_cursor: null }) : send(404, {})
    }
    assert.equal(req.headers['x-mcr-intent'], 'browser')
    const key = req.headers['idempotency-key']; assert.match(key, /^[0-9a-f-]{36}$/)
    assert.equal(receipts.has(key), false, 'No duplicate command dispatch')
    const chunks = []; for await (const chunk of req) chunks.push(chunk)
    const body = chunks.length ? JSON.parse(Buffer.concat(chunks).toString()) : null
    writes++
    if (path.endsWith('/posts') && method === 'POST') {
      const post = { ...fixture, post_id: id(++sequence), text: body.text, media_ids: body.media_ids, links: body.links, created_at: new Date().toISOString(), comment_count: 0, reaction_count: 0, reacted_by_me: false, revision: 1 }
      posts.unshift(post)
      receipts.set(key, { receipt_id: id(++sequence), event_id: fixture.event_id, actor_id: fixture.author.user_id, operation_id: 'createConcertPost', idempotency_key: key, state: 'SUCCEEDED', resource_id: post.post_id, resource_revision: 1, failure_reason: null, created_at: new Date().toISOString(), expires_at: '2099-01-01T00:00:00Z', result_status: 200 })
      if (body.text.includes('uncertain')) return send(503, { code: 'UNAVAILABLE', message: 'raw-private-error-must-never-render', details: { reason: 'DEPENDENCY_UNAVAILABLE' } })
      return send(200, post)
    }
    const postId = path.split('/')[6], post = posts.find(p => p.post_id === postId)
    if (!post) return send(404, {})
    if (path.endsWith('/comments') && method === 'POST') {
      assert.equal(body.post_revision, post.revision)
      const comment = { comment_id: id(++sequence), post_id: postId, author: fixture.author, text: body.text, revision: 1, created_at: new Date().toISOString() }
      comments.push(comment); post.comment_count++; post.revision++; return send(200, comment)
    }
    if (path.endsWith('/reaction')) {
      post.reacted_by_me = method === 'PUT'; post.reaction_count = post.reacted_by_me ? 1 : 0; post.revision++
      return send(200, { post_id: postId, reacted_by_me: post.reacted_by_me, reaction_count: post.reaction_count, revision: post.revision })
    }
    return send(404, {})
  })
} }], server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
const errors = [], results = []
try {
  await dev.listen(); const base = `http://127.0.0.1:${dev.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    posts = []; comments = []; receipts.clear(); writes = 0
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`${base}/src/features/feed-thread/__tests__/fixtures/preview.html`)
    await expect(page.getByText('No posts yet.')).toBeVisible()
    await page.getByRole('button', { name: 'Write a post' }).click()
    await page.getByLabel('Post text', { exact: true }).fill(`A test-only uncertain post ${width}`)
    await page.getByRole('button', { name: 'Publish post', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect(page.getByText(/The outcome is unknown/)).toBeVisible()
    await expect(page.getByRole('button', { name: 'Publish post', exact: true })).toBeDisabled()
    assert.equal(writes, 1); assert.equal((await page.locator('body').innerText()).includes('raw-private-error'), false)
    await page.getByRole('button', { name: 'Check command status' }).click()
    await page.getByRole('button', { name: 'Open published post' }).click()
    await expect(page.getByText(`A test-only uncertain post ${width}`, { exact: true })).toBeVisible()
    await page.getByLabel('Comment text', { exact: true }).fill('Test-only reply')
    await page.getByRole('button', { name: 'Send comment' }).click()
    await expect(page.getByText('Test-only reply', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'React', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Remove reaction', exact: true })).toHaveAttribute('aria-pressed', 'true')
    assert.equal(writes, 3); assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
    if (width === 390) await page.screenshot({ path: `${evidence}/thread-390.png`, fullPage: true })
    await page.getByRole('button', { name: 'Revoke privacy' }).click()
    await expect(page.getByText(`A test-only uncertain post ${width}`, { exact: true })).toHaveCount(0)
    await expect(page.getByText('Test-only reply', { exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Feed', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Write a post' })).toBeVisible()
    await page.getByRole('button', { name: 'Read-only event' }).click()
    await expect(page.getByRole('button', { name: 'Write a post' })).toHaveCount(0)
    results.push({ width, uncertainWriteStatusRecovery: 'PASS', comments: 'PASS', reaction: 'PASS', privacyEviction: 'PASS', readOnly: 'PASS', horizontalOverflow: false, writes })
    await context.close()
  }
  assert.deepEqual(errors, [])
  await writeFile(`${evidence}/browser-report.json`, JSON.stringify({ scope: 'Test-only native HTTP fixtures; no live backend/device acceptance', browser: await browser.version(), results, errors, requests: calls.length }, null, 2) + '\n')
  console.log(JSON.stringify({ result: 'PASS', widths: results.map(r => r.width), requests: calls.length, errors }))
} finally { await browser?.close(); await dev.close() }
