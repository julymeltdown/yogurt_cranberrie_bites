// Actual shipped /src/main.tsx and default routes; HTTP responses remain deterministic test fixtures.
import assert from 'node:assert/strict'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { createServer, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = process.env.MCR_WORLD_ROOT_EVIDENCE ?? '/tmp/mcr-world-root-browser'
const readJson = async path => JSON.parse(await readFile(path, 'utf8'))
const bootstrap = await readJson('../contracts/fixtures/core/bootstrap-open.json')
const rooms = await readJson('../contracts/fixtures/realtime/rooms.json')
const auth = await readJson('../contracts/fixtures/core/auth-success.json')
const capabilities = await readJson('../contracts/fixtures/core/auth-capabilities-disabled.json')
capabilities.providers.find(p => p.provider === 'EMAIL').enabled = true
capabilities.providers.find(p => p.provider === 'EMAIL').disabled_reason = null
// Only turn on world capability to exercise real root wiring. Shipped unprepared/synthetic art is never replaced.
bootstrap.capabilities.world = { available: true, reason: null }
const testInput = { revision: 'TEST_ONLY_ROOT_POLICY', reviewedBy: 'TEST ONLY browser harness', zoom: 2,
  movementPolicy: { speedPxPerSecond: 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 } }
await mkdir(evidence, { recursive: true })
let browser, server, production
const results = [], errors = []
try {
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const configured of [false, true]) {
    server = await createServer({ define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(bootstrap.event.event_id),
      'import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG': JSON.stringify(configured ? JSON.stringify(testInput) : '') },
      server: { host: '127.0.0.1', port: 0, strictPort: false } })
    await server.listen()
    const base = `http://127.0.0.1:${server.httpServer.address().port}`
    const context = await browser.newContext({ viewport: { width: 390, height: 844 } }), page = await context.newPage()
    const requests = [], scripts = [], sockets = []
    page.on('pageerror', error => errors.push(error.message))
    page.on('request', request => { const path = new URL(request.url()).pathname; if (request.resourceType() === 'script') scripts.push(path); if (path.startsWith('/api/') || path.startsWith('/world/')) requests.push(path) })
    page.on('websocket', socket => { if (!socket.url().includes('vite')) sockets.push(socket.url()) })
    await page.route('**/api/**', async route => {
      const path = new URL(route.request().url()).pathname
      if (!path.startsWith('/api/')) return route.continue()
      if (path === '/api/auth/capabilities') return route.fulfill({ json: capabilities })
      if (path === '/api/auth/login') return route.fulfill({ json: auth })
      if (path.endsWith('/bootstrap')) return route.fulfill({ json: bootstrap })
      if (path.endsWith('/rooms')) return route.fulfill({ json: rooms })
      throw new Error(`Unexpected API request ${path}`)
    })
    await page.goto(`${base}/login?returnTo=%2Fworld%2Fnearby`)
    await expect(page.getByRole('button', { name: 'Sign in with email' })).toBeEnabled()
    assert(!scripts.some(path => /phaser|\/src\/world\/|root-features|CommunityFeatures/i.test(path)), 'public page loaded world/private composition')
    await page.getByLabel('Email', { exact: true }).fill('fixture@example.test')
    await page.getByLabel('Password', { exact: true }).fill('fixture-password')
    await page.getByRole('button', { name: 'Sign in with email' }).click()
    await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Nearby in world', level: 2 })).toBeVisible()
    await expect(page.locator('[data-world-host]')).toContainText(/unavailable|could not open/i)
    await page.locator('[data-world-host]').evaluate(el => { el.dataset.rootIdentity = 'same-host' })
    await page.getByRole('link', { name: 'Choose a room', exact: true }).click()
    await expect(page.getByRole('heading', { name: 'Choose your room' })).toBeVisible()
    // Catalog reads remain separate from admission; no switch can succeed without a live replication release.
    await page.getByRole('link', { name: 'Nearby in world', exact: true }).click()
    await expect(page.locator('[data-world-host]')).toHaveAttribute('data-root-identity', 'same-host')
    assert.equal(await page.locator('canvas').count(), 0)
    assert(!scripts.some(path => /phaser/i.test(path)), 'unprepared resources loaded Phaser')
    assert.equal(requests.filter(path => path.endsWith('/bootstrap')).length, 1)
    assert(!requests.some(path => /ticket|\/world\//.test(path)), 'unprepared resources fetched assets/tickets')
    // Vite has its own HMR WebSocket; count only world transport endpoints.
    assert.equal(sockets.filter(url => /world-ws|\/ws\//.test(url)).length, 0)
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
    await page.screenshot({ path: `${evidence}/${configured ? 'unprepared-art' : 'missing-config'}-390.png`, fullPage: true })
    await page.getByRole('link', { name: 'MCR Concert World home' }).click()
    // Authenticated home redirects to world. Leave shell explicitly via browser navigation to public legal route.
    await page.goto(`${base}/legal/privacy`)
    await expect(page.getByRole('heading', { name: 'Privacy', exact: true })).toBeVisible()
    assert.equal(await page.locator('[data-world-host],canvas').count(), 0)
    results.push({ configured, state: 'PASS', requests, scripts: scripts.filter(path => /root-features|world\/composition|world-entry\/resolver|phaser/i.test(path)), worldSockets: 0 })
    await context.close(); await server.close(); server = null
  }
  production = await preview({ preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const page = await browser.newPage(), publicScripts = []
  page.on('request', req => { if (req.resourceType() === 'script') publicScripts.push(new URL(req.url()).pathname) })
  await page.goto(`http://127.0.0.1:${production.httpServer.address().port}/login`)
  await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible()
  assert(!publicScripts.some(path => /phaser|root-features|CommunityFeatures/i.test(path)))
  assert.equal(await page.locator('[data-world-host],canvas').count(), 0)
  assert.deepEqual(errors, [])
  const paths = ['src/main.tsx', 'src/app/ports.ts', 'src/app/WorldShell.tsx', 'src/app/router.tsx', 'src/app/WorldHost.tsx',
    'src/app/world-entry/configuration.ts', 'src/app/world-entry/production.ts', 'src/app/world-entry/root-features.tsx', 'src/app/world-entry/__tests__/production.test.tsx', 'src/app/world-entry/__tests__/root-browser.mjs']
  const hashes = Object.fromEntries(await Promise.all(paths.map(async path => [path, createHash('sha256').update(await readFile(path)).digest('hex')])))
  await writeFile(`${evidence}/report.json`, JSON.stringify({ state: 'MOCK_READY', browser: browser.version(), node: process.version, results, publicScripts, hashes,
    limitations: 'Actual main/Auth/C03/composition/resolver/default routes, mocked HTTP and shipped unprepared metadata. No approved world art, real backend, Phaser canvas, actual world socket, multiuser or device acceptance.' }, null, 2) + '\n')
  console.log(`PASS shipped entry: public lazy, authenticated default rooms/nearby, missing config/unprepared art closed; ${evidence}`)
} finally {
  await browser?.close(); await server?.close()
  if (production) await new Promise(resolve => production.httpServer.close(resolve))
}
