import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, rm } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const repo = new URL('../../../../../', import.meta.url), out = new URL('./evidence/caller/', import.meta.url)
const hash = value => createHash('sha256').update(value).digest('hex')
const paths = ['frontend/src/world/mural-entry-lifecycle/source.ts', 'frontend/src/world/mural-entry-lifecycle/index.ts',
  'frontend/src/world/mural-entry-lifecycle/__tests__/assembly-data.ts', 'frontend/src/world/mural-entry-lifecycle/__tests__/assembly-native.tsx',
  'frontend/src/world/mural-entry-lifecycle/__tests__/assembly-native.html', 'frontend/src/world/mural-entry-lifecycle/__tests__/assembly-native.mjs',
  'frontend/src/world/composition/index.ts', 'frontend/src/app/world-panels/index.tsx', 'frontend/src/app/world-entry/production.ts',
  'frontend/src/world/runtime.ts', 'frontend/src/world/scenes/MapScene.ts', 'frontend/src/world/mural/phaser.ts',
  'frontend/src/world/mural/sink.ts', 'frontend/src/world/mural/authority.ts', 'frontend/src/app/mural-entry/owner.ts',
  'frontend/src/app/mural-entry/provider.tsx', 'frontend/src/features/mural/editor/MuralEditor.tsx', 'frontend/src/features/mural/sync/http.ts']
const sources = Object.fromEntries(await Promise.all(paths.map(async path => [path, hash(await readFile(new URL(path, repo)))])))
await mkdir(out, { recursive: true })
const dev = await createServer({ configLoader: 'runner', cacheDir: new URL('./.cache/', out).pathname, server: { host: '127.0.0.1', port: 0 } })
const results = [], errors = []
let browser
try {
  await dev.listen(); browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1 }), page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    await page.goto(`http://127.0.0.1:${dev.httpServer.address().port}/src/world/mural-entry-lifecycle/__tests__/assembly-native.html`)
    const read = () => page.evaluate(() => window.__MURAL_ASSEMBLY__?.read())
    const inspect = () => page.evaluate(() => window.__MURAL_ASSEMBLY__.inspect())
    await expect(page.getByText('REV 10 · EDITABLE', { exact: true })).toBeVisible({ timeout: 15000 })
    await expect.poll(async () => (await inspect()).objects).toBe(1)
    const initial = await read(); assert.equal(initial.created, 1); assert.equal(initial.socketCount, 1); assert.equal(initial.factoryCalls, 1)
    await page.evaluate(() => window.__MURAL_ASSEMBLY__.lost())
    await page.getByRole('group', { name: 'Mural pixel preview', exact: true }).focus(); await page.keyboard.press('ArrowRight')
    await page.getByRole('button', { name: 'Confirm pixel', exact: true }).click()
    await expect.poll(async () => (await read()).editor).toBe('unknown')
    const unknown = await read(); assert.equal(unknown.posts.length, 1); assert.equal(unknown.command, unknown.posts[0])
    await page.getByRole('button', { name: 'Withdraw fixture qualification' }).click()
    await expect(page.getByText(/Wall unavailable/)).toBeVisible()
    const withdrawn = await read(); assert.equal(withdrawn.available, false); assert.equal(withdrawn.command, unknown.command)
    assert.deepEqual(await inspect(), { texture: false, objects: 0 })
    await page.getByRole('button', { name: 'Supply fresh fixture qualification' }).click()
    await expect(page.getByText('REV 11 · EDITABLE', { exact: true })).toBeVisible()
    const recovered = await read(); assert.equal(recovered.command, unknown.command); assert.equal(recovered.posts.length, 1)
    await page.evaluate(() => window.__MURAL_ASSEMBLY__.allowResult())
    await page.getByRole('button', { name: 'Check original command' }).click()
    await expect.poll(async () => (await read()).editor).toBe('confirmed')
    const confirmed = await read(); assert.equal(confirmed.command, null); assert.equal(confirmed.posts.length, 1)
    assert.equal(confirmed.created, 1); assert.equal(confirmed.active, 1); assert.equal(confirmed.peak, 1); assert.equal(confirmed.socketCount, 1)
    const screenshot = `caller-${width}.png`; await page.screenshot({ path: new URL(screenshot, out).pathname, fullPage: true })
    await page.getByRole('button', { name: 'Invalidate fixture privacy' }).click()
    await expect.poll(async () => (await read()).active).toBe(0)
    const revoked = await read(); assert.equal(revoked.available, false); assert.equal(revoked.sockets, 0); assert.equal(revoked.sourceListeners, 0)
    assert.equal(revoked.command, null); assert.deepEqual(await inspect(), { texture: false, objects: 0 })
    await page.evaluate(() => window.__MURAL_ASSEMBLY__.dispose())
    await expect(page.locator('#world canvas')).toHaveCount(0)
    results.push({ width, initial, unknown, withdrawn, recovered, confirmed, revoked, screenshot, sha256: hash(await readFile(new URL(screenshot, out))) })
    await context.close()
  }
  const page = await browser.newPage()
  page.on('pageerror', e => errors.push(e.message))
  await page.goto(`http://127.0.0.1:${dev.httpServer.address().port}/src/world/mural-entry-lifecycle/__tests__/assembly-native.html?source=omitted`)
  await expect.poll(() => page.evaluate(() => window.__MURAL_ASSEMBLY__?.read().phase)).toBe('live')
  await expect(page.getByText(/Wall unavailable/)).toBeVisible()
  const omitted = await page.evaluate(() => window.__MURAL_ASSEMBLY__.read()); assert.equal(omitted.factoryCalls, 0); assert.equal(omitted.available, false); assert.equal(omitted.posts.length, 0)
  await page.evaluate(() => window.__MURAL_ASSEMBLY__.dispose()); await page.close(); assert.deepEqual(errors, [])
  await writeFile(new URL('native.json', out), JSON.stringify({ status: 'PASS', synthetic: true, base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(), browser: browser.version(), sources, results, omitted, errors }, null, 2) + '\n')
  console.log(JSON.stringify({ status: 'PASS', widths: results.length, omitted: 'closed', errors }))
} finally { await browser?.close(); await dev.close(); await rm(new URL('./.cache/', out), { recursive: true, force: true }) }
