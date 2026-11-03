import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

// Uses the existing account route fixture and production asset admission unchanged.
// Authentication/HTTP/world ports are synthetic; this is not a live journey.
const root = new URL('../../../../../../', import.meta.url), out = new URL('./evidence/account/', import.meta.url)
const read = async path => JSON.parse(await readFile(new URL(path, root)))
const bootstrap = await read('contracts/fixtures/core/bootstrap-open.json')
const login = await read('contracts/fixtures/core/auth-success.json')
const capabilities = await read('contracts/fixtures/core/auth-capabilities-disabled.json')
capabilities.providers = capabilities.providers.map(p => p.provider === 'EMAIL' ? { ...p, enabled: true, disabled_reason: null } : p)
const hash = b => createHash('sha256').update(b).digest('hex'), sources = {}, results = [], requests = [], errors = []
for (const path of ['frontend/src/features/avatar/AvatarEditor.tsx', 'frontend/src/features/avatar/avatar-editor.css', 'frontend/src/features/avatar/__tests__/visual-adoption/account-browser.mjs', 'frontend/src/app/account-entry/__tests__/preview.tsx', 'frontend/src/app/account-entry/assets.ts', 'frontend/src/app/account-entry/panels.tsx', 'frontend/src/app/account-entry/provider.tsx', 'frontend/src/app/account-entry/owner.ts', 'frontend/src/app/router.tsx', 'frontend/src/app/shell.css', 'frontend/src/entities/avatar/character-source-pack.json']) sources[path] = hash(await readFile(new URL(path, root)))
await mkdir(out, { recursive: true })
const dev = await createServer({ configLoader: 'runner', cacheDir: '/tmp/mcr-uiv04-avatar-account', server: { host: '127.0.0.1', port: 0, strictPort: false }, plugins: [{ name: 'avatar-account-fixture-http', configureServer(server) {
  server.middlewares.use(async (req, res, next) => {
    if (!req.url?.startsWith('/api/')) return next()
    requests.push({ path: req.url, method: req.method })
    let value
    if (req.url === '/api/auth/capabilities') value = capabilities
    else if (req.url === '/api/auth/login') value = login
    else if (req.url.endsWith('/bootstrap')) { value = structuredClone(bootstrap); value.avatar = value.participant.profile.avatar; value.capabilities.world = { available: true, reason: null } }
    else { res.statusCode = 500; res.end('{}'); return }
    res.setHeader('Content-Type', 'application/json'); res.setHeader('Cache-Control', 'no-store'); res.end(JSON.stringify(value))
  })
} }] })
let browser
try {
  await dev.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const url = `http://127.0.0.1:${dev.httpServer.address().port}/src/app/account-entry/__tests__/preview.html?route=/me/avatar`
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 1, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    await page.goto(url)
    await expect(page.getByText(/Reviewed avatar artwork is unavailable/)).toBeVisible()
    await expect(page.getByRole('button', { name: 'Save outfit', exact: true })).toBeDisabled()
    assert.equal(await page.locator('canvas').count(), 0)
    const stats = await page.evaluate(() => window.__ACCOUNT__.stats)
    assert.equal(stats.games, 1); assert.equal(stats.connections, 1)
    assert.equal(stats.peakGames, 1); assert.equal(stats.peakConnections, 1)
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    const expanded = page.getByRole('button', { name: 'Expand panel', exact: true })
    if (await expanded.count()) await expanded.click()
    const file = `default-avatar-unavailable-${width}.png`
    await page.screenshot({ path: new URL(file, out).pathname, fullPage: true })
    results.push({ width, route: '/me/avatar', realComposition: true, fixturePorts: true, reviewedArtUnavailable: true, canvasCount: 0, instrumentedWorld: { active: stats.games, peak: stats.peakGames, mounts: stats.mounts, disposals: stats.disposals }, png: file, sha256: hash(await readFile(new URL(file, out))) })
    await page.evaluate(() => window.__ACCOUNT__.dispose()); await context.close()
  }
  assert.deepEqual(errors, []); assert(!requests.some(r => r.method === 'PUT'))
  await writeFile(new URL('report.json', out), JSON.stringify({ status: 'PASS', base: 'b3889feeed0fa7485a2326cf5bd00ffc4bb4f894', browser: browser.version(), sources, results, requests, errors }, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', viewports: results.length, errors }))
} finally { await browser?.close(); await dev.close() }
