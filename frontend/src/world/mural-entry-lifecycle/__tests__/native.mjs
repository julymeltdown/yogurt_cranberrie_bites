import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile, rm } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const evidence = new URL('./evidence/', import.meta.url), repo = new URL('../../../../../', import.meta.url)
const hash = value => createHash('sha256').update(value).digest('hex')
const paths = ['frontend/src/world/mural-entry-lifecycle/index.ts', 'frontend/src/world/mural-entry-lifecycle/__tests__/native.ts',
  'frontend/src/world/mural-entry-lifecycle/__tests__/native.html', 'frontend/src/world/mural-entry-lifecycle/__tests__/native.mjs',
  'frontend/src/world/mural-entry-lifecycle/__tests__/fixture.ts', 'frontend/src/app/mural-entry/ports.ts', 'frontend/src/app/mural-entry/owner.ts',
  'frontend/src/world/mural/phaser.ts', 'frontend/src/world/mural/sink.ts', 'frontend/src/features/mural/sync/controller.ts',
  'frontend/src/features/mural/sync/http.ts', 'frontend/src/features/mural/editor/controller.ts', 'frontend/src/shared/api/errors.ts',
  'contracts/domain/v1/common.schema.json', 'contracts/domain/v1/mural.schema.json']
const sources = Object.fromEntries(await Promise.all(paths.map(async path => [path, hash(await readFile(new URL(path, repo)))])))
await mkdir(evidence, { recursive: true })
const dev = await createServer({ configLoader: 'runner', cacheDir: new URL('./.cache/', evidence).pathname,
  server: { host: '127.0.0.1', port: 0, strictPort: false } })
const results = [], errors = []
let browser
try {
  await dev.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1 }), page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`http://127.0.0.1:${dev.httpServer.address().port}/src/world/mural-entry-lifecycle/__tests__/native.html`)
    const read = () => page.evaluate(() => window.__MURAL_LIFECYCLE__?.read())
    await expect.poll(async () => (await read())?.texturePresent).toBe(true)
    const initial = await read(); assert.equal(initial.revision, 10); assert.equal(initial.objects, 1); assert.equal(initial.created, 1)
    await page.getByRole('button', { name: 'Confirm fixture pixel with lost response' }).click()
    await expect.poll(async () => (await read())?.phase).toBe('unknown')
    const unknown = await read(); assert.equal(unknown.posts.length, 1); assert.equal(unknown.commandKey, unknown.posts[0])
    await page.getByRole('button', { name: 'Withdraw world', exact: true }).click()
    const withdrawn = await read(); assert.equal(withdrawn.available, false); assert.equal(withdrawn.texturePresent, false); assert.equal(withdrawn.objects, 0); assert.equal(withdrawn.timers, 0)
    await page.getByRole('button', { name: 'Supply fresh qualification' }).click()
    await expect.poll(async () => (await read())?.texturePresent).toBe(true)
    const recovered = await read(); assert.equal(recovered.revision, 11); assert.equal(recovered.commandKey, unknown.commandKey)
    assert.equal(recovered.posts.length, 1); assert.equal(recovered.created, 1); assert.equal(recovered.games, 1); assert.equal(recovered.objects, 1)
    await page.getByRole('button', { name: 'Check original command' }).click()
    await expect.poll(async () => (await read())?.phase).toBe('confirmed')
    const confirmed = await read(); assert.equal(confirmed.commandKey, null); assert.equal(confirmed.posts.length, 1)
    const png = `recovered-${width}.png`; await page.screenshot({ path: new URL(png, evidence).pathname })
    await page.getByRole('button', { name: 'Invalidate privacy' }).click()
    const revoked = await read(); assert.equal(revoked.available, false); assert.equal(revoked.texturePresent, false); assert.equal(revoked.objects, 0)
    assert.equal(revoked.worldListeners, 0); assert.equal(revoked.timers, 0); assert.equal(revoked.disposed, true)
    await page.evaluate(() => window.__MURAL_LIFECYCLE__.dispose())
    await expect(page.locator('#world canvas')).toHaveCount(0)
    results.push({ width, initial, unknown, withdrawn, recovered, confirmed, revoked, screenshot: png, sha256: hash(await readFile(new URL(png, evidence))) })
    await context.close()
  }
  assert.deepEqual(errors, [])
  const receipt = { status: 'PASS', synthetic: true, browser: browser.version(), base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(), sources, results, errors,
    limits: 'Actual incumbent Phaser scene/sink/entry and injected C04 fixture HTTP; no product anchor, admission, live WebSocket or real producer.' }
  await writeFile(new URL('native.json', evidence), JSON.stringify(receipt, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', viewports: results.length, errors }))
} finally { await browser?.close(); await dev.close(); await rm(new URL('./.cache/', evidence), { recursive: true, force: true }) }
