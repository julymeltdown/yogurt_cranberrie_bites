// TEST ONLY: actual HUD DOM/CSS capture; no app/Auth/World/Phaser claim.
import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import tailwind from '@tailwindcss/vite'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../../../../..'), frontend = resolve(root, 'frontend')
const out = process.env.PI08_OUTPUT, photo = process.env.PI08_PHOTO, port = Number(process.env.PI08_PORT ?? 5367)
assert(out && photo && port === 5367, 'PI08_OUTPUT, PI08_PHOTO and strict port 5367 required')
const reference = await readFile(photo)
assert.equal(createHash('sha256').update(reference).digest('hex'), '9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27')
const report = { status: 'FAIL', classification: 'ACTUAL_HUD_COMPONENTS_CONTROLLED_FIXTURE_NOT_LIVE', reference: {
  sha256: createHash('sha256').update(reference).digest('hex'), crop: [436, 60, 832, 847], normalization: 390 / 396 }, captures: {}, deltas: {}, errors: [] }
const server = await createServer({ root: frontend, configFile: false, publicDir: false, plugins: [react(), tailwind()],
  cacheDir: resolve(out, 'vite-cache'), logLevel: 'warn',
  optimizeDeps: { entries: [], noDiscovery: true, include: ['react', 'react-dom', 'react-dom/client', 'react/jsx-runtime',
    'react/jsx-dev-runtime', 'react-router-dom', '@tanstack/react-query', 'react-hook-form', '@hookform/resolvers/zod', 'zod', 'ky', 'zustand'] },
  resolve: { dedupe: ['react', 'react-dom', 'react-router', 'react-router-dom', '@tanstack/react-query'] },
  server: { host: '127.0.0.1', port, strictPort: true, hmr: false } })
let browser, page
const measure = () => page.evaluate(() => {
  const box = selector => { const element = document.querySelector(selector); const r = element.getBoundingClientRect()
    return { x: r.x, y: r.y, width: r.width, height: r.height } }
  return { viewport: [innerWidth, innerHeight], DPR: devicePixelRatio, overflow: document.documentElement.scrollWidth > innerWidth,
    title: box('.pi08-world-title'), context: box('.pi08-world-context'), navigation: box('.pi08-navigation'),
    action: box('.pi08-context-action'), stage: box('.shell-stage'), bottomPadding: getComputedStyle(document.querySelector('.world-shell')).paddingBottom,
    faces: [...document.fonts].map(face => ({ family: face.family.replaceAll('"', ''), status: face.status })) }
})
async function settle() {
  await page.evaluate(async () => { await document.fonts.load('400 18px "Parade Ink Text"'); await document.fonts.load('700 26px "Parade Ink Display"'); await document.fonts.ready })
  await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))))
}
async function capture(name, width, height) {
  await page.setViewportSize({ width, height }); await settle()
  const metrics = await measure()
  assert.equal(metrics.overflow, false, name + ' overflow')
  assert.equal(metrics.DPR, 1, name + ' DPR')
  for (const family of ['Parade Ink Text', 'Parade Ink Display']) assert(metrics.faces.some(face => face.family === family && face.status === 'loaded'), name + ' font ' + family)
  report.captures[name] = metrics
  await page.screenshot({ path: resolve(out, name + '.png') })
  return metrics
}
try {
  await mkdir(out, { recursive: true }); await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--disable-breakpad'] })
  report.browser = browser.version()
  page = await browser.newPage({ viewport: { width: 390, height: 775 }, deviceScaleFactor: 1, reducedMotion: 'reduce' })
  page.on('pageerror', error => report.errors.push(error.message))
  await page.goto(`http://127.0.0.1:${port}/src/features/parade-ink/world-hud/__tests__/finish-02/preview.html`)
  await expect(page.locator('.pi08-world-context')).toBeVisible({ timeout: 20000 })
  const metrics390 = await capture('world-390', 390, 775)
  const referenceRegions = { title: { x: 3.9393939393939394, y: 4.924242424242424, width: 381.1363636363636, height: 46.28787878787879 },
    context: { x: 10.833333333333334, y: 617.5, width: 367.3484848484849, height: 72.87878787878788 },
    navigation: { x: 4.924242424242424, y: 703.1818181818181, width: 380.1515151515151, height: 60.07575757575757 } }
  for (const key of Object.keys(referenceRegions)) report.deltas[key] = Object.fromEntries(['x', 'y', 'width', 'height'].map(field =>
    [field, metrics390[key][field] - referenceRegions[key][field]]))
  const within = (key, tolerance) => ['x', 'y', 'width', 'height'].every(field => Math.abs(report.deltas[key][field]) <= tolerance)
  // Title uses the published 48px token against a 46.29px crop; navigation includes its top rule.
  assert(within('title', 2) && within('context', 1) && within('navigation', 2), 'Fixed HUD regions outside CSS-pixel rounding')
  for (const [name, width, height] of [['world-360', 360, 800], ['world-430', 430, 932]]) {
    const metrics = await capture(name, width, height)
    assert.equal(metrics.context.x, metrics390.context.x, name + ' context inline inset')
    assert(Math.abs(metrics.context.width - (width - 22)) <= 1, name + ' context inline width')
    assert(Math.abs((metrics.context.y + metrics.context.height + 12) - metrics.navigation.y) <= 1, name + ' context to navigation gap')
  }
  const artifacts = await page.evaluate(async ({ photo, shot }) => {
    const image = source => new Promise((done, reject) => { const value = new Image(); value.onload = () => done(value); value.onerror = reject; value.src = source })
    const [ref, app] = await Promise.all([image(photo), image(shot)]), scale = 390 / 396, height = Math.round(787 * scale)
    const crop = document.createElement('canvas'); crop.width = 390; crop.height = height
    crop.getContext('2d').drawImage(ref, 436, 60, 396, 787, 0, 0, 390, height)
    const pair = document.createElement('canvas'); pair.width = 780; pair.height = height
    pair.getContext('2d').drawImage(crop, 0, 0); pair.getContext('2d').drawImage(app, 390, 0)
    const overlay = document.createElement('canvas'); overlay.width = 390; overlay.height = height
    const overlayContext = overlay.getContext('2d'); overlayContext.drawImage(crop, 0, 0); overlayContext.globalAlpha = .5; overlayContext.drawImage(app, 0, 0)
    const diff = document.createElement('canvas'); diff.width = 390; diff.height = height
    const diffContext = diff.getContext('2d'); diffContext.drawImage(crop, 0, 0); const first = diffContext.getImageData(0, 0, 390, height)
    diffContext.clearRect(0, 0, 390, height); diffContext.drawImage(app, 0, 0); const second = diffContext.getImageData(0, 0, 390, height)
    for (let index = 0; index < first.data.length; index += 4) {
      for (let channel = 0; channel < 3; channel++) first.data[index + channel] = Math.abs(first.data[index + channel] - second.data[index + channel])
      first.data[index + 3] = 255
    }
    diffContext.putImageData(first, 0, 0)
    return { reference: crop.toDataURL(), pair: pair.toDataURL(), overlay: overlay.toDataURL(), diff: diff.toDataURL() }
  }, { photo: 'data:image/jpeg;base64,' + reference.toString('base64'), shot: 'data:image/png;base64,' + (await readFile(resolve(out, 'world-390.png'))).toString('base64') })
  for (const [name, data] of Object.entries(artifacts)) await writeFile(resolve(out, 'photo2-' + name + '.png'), Buffer.from(data.split(',')[1], 'base64'))
  assert.equal(report.errors.length, 0, 'Page errors')
  report.status = 'PASS_HUD_GEOMETRY_AND_COMPONENTS'
} catch (error) {
  report.failure = { message: String(error), stack: error.stack }; throw error
} finally {
  if (page && report.status === 'FAIL') await page.screenshot({ path: resolve(out, 'failure.png') }).catch(() => {})
  if (browser) await browser.close()
  await server.close()
  await writeFile(resolve(out, 'hud-geometry.json'), JSON.stringify(report, null, 2) + '\n')
}
