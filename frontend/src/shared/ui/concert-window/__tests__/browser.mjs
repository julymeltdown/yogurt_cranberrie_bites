// From frontend/: pinned Node; existing Chrome only. No install or download.
import assert from 'node:assert/strict'
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { build, preview } from 'vite'
import react from '@vitejs/plugin-react'
import { chromium, expect } from '@playwright/test'

const dir = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(dir, '../../../../../..')
const frontend = path.join(root, 'frontend')
const chrome = process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const outDir = await mkdtemp(path.join(tmpdir(), 'mcr-uiv01-'))
const evidence = path.join(dir, 'evidence')
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
let server, browser
const screenshots = [], errors = [], externalRequests = [], results = []
try {
  await build({ configFile: false, root: frontend, plugins: [react()],
    build: { outDir, emptyOutDir: true, rollupOptions: { input: path.join(dir, 'preview.html') } } })
  server = await preview({ configFile: false, root: frontend, build: { outDir },
    preview: { host: '127.0.0.1', port: 0, strictPort: false } })
  const address = server.httpServer.address()
  assert(address && typeof address !== 'string')
  const origin = `http://127.0.0.1:${address.port}`
  browser = await chromium.launch({ executablePath: chrome, headless: true })
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce', hasTouch: true })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    page.on('request', request => {
      if (!request.url().startsWith(origin) && !request.url().startsWith('data:')) externalRequests.push(request.url())
      assert.equal(request.method(), 'GET', 'Presentation preview must not send mutations')
    })
    await page.goto(`${origin}/src/shared/ui/concert-window/__tests__/preview.html`)
    const window = page.getByRole('region', { name: 'Meet', exact: true })
    await expect(window).toBeVisible()
    const state = page.getByLabel('Example state')
    async function capture(name) {
      const file = `${width}-${name}.png`
      await page.screenshot({ path: path.join(evidence, file) })
      screenshots.push({ file, width, state: name, sha256: hash(await readFile(path.join(evidence, file))) })
    }
    async function checkLayout() {
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `overflow ${width}`)
      for (const target of await page.locator('button, select, nav a').all()) {
        if (!await target.isVisible()) continue
        const box = await target.boundingBox()
        assert(box && box.width >= 44 && box.height >= 44, `44px target ${width}: ${await target.textContent()}`)
      }
    }
    await checkLayout()
    await capture('loaded')
    assert((await page.getByRole('heading', { name: 'Area A queue group' }).boundingBox()).y < 390, 'First row must appear promptly')
    const parties = page.getByRole('tab', { name: 'Parties', exact: true })
    const people = page.getByRole('tab', { name: 'People', exact: true })
    await parties.focus()
    await page.keyboard.press('ArrowRight')
    await expect(people).toBeFocused()
    await expect(people).toHaveAttribute('aria-selected', 'true')
    await expect(page.getByRole('tabpanel', { name: 'People' })).toContainText('No example introductions')
    await page.keyboard.press('End')
    await expect(page.getByRole('tab', { name: 'My plan' })).toBeFocused()
    await page.keyboard.press('Home')
    await expect(parties).toBeFocused()
    await page.keyboard.press('Tab')
    await expect(page.getByRole('tabpanel', { name: 'Parties' })).toBeFocused()
    const notes = page.getByRole('button', { name: 'Layout notes', exact: true })
    await notes.focus()
    assert.equal(await notes.evaluate(e => getComputedStyle(e).outlineStyle), 'solid', 'Visible keyboard outline')
    assert.equal(await notes.evaluate(e => getComputedStyle(e).transitionDuration), '0s', 'Reduced motion')
    await capture('keyboard')
    await page.keyboard.press('Enter')
    await expect(notes).toHaveAttribute('aria-expanded', 'true')
    await expect(page.locator('#layout-notes')).toBeVisible()
    await page.getByRole('button', { name: 'Details', exact: true }).click()
    for (const value of ['empty', 'unavailable', 'pending']) {
      await state.selectOption(value)
      await checkLayout()
      if (value === 'empty') await expect(window.getByRole('status')).toContainText('No parties here yet')
      if (value === 'unavailable') await expect(notes).toBeDisabled()
      if (value === 'pending') {
        const pending = page.getByRole('button', { name: 'Awaiting owner response' })
        await expect(pending).toBeDisabled()
        await expect(pending).toHaveAttribute('aria-busy', 'true')
        await expect(window.getByRole('status').first()).toContainText('Pending does not mean joined')
      }
      await capture(value)
    }
    await state.selectOption('loaded')
    await page.getByRole('button', { name: 'Close Meet' }).focus()
    await page.keyboard.press('Enter')
    await expect(window).not.toBeVisible()
    const reopen = page.getByRole('button', { name: 'Open concert window' })
    await expect(reopen).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(window).toBeVisible()
    const nav = page.getByRole('navigation', { name: 'Preview destinations' })
    await expect(nav.getByRole('link', { name: 'Meet', exact: true })).toHaveAttribute('aria-current', 'page')
    await nav.getByRole('link', { name: 'Wall', exact: true }).click()
    assert.equal(new URL(page.url()).hash, '#example-wall')
    await expect(page.locator('#example-wall')).toBeInViewport()
    results.push({ width, result: 'PASS', keyboard: true, controls44px: true, noOverflow: true, primaryLocalDisclosure: true, closeFocusRestored: true })
    await context.close()
  }
  assert.deepEqual(errors, [])
  assert.deepEqual(externalRequests, [])
  const sources = ['index.tsx', 'concert-window.css', '__tests__/preview.html', '__tests__/preview.tsx', '__tests__/preview.css', '__tests__/browser.mjs']
  const sourceHashes = {}
  for (const relative of sources) {
    const absolute = path.resolve(dir, '..', relative)
    sourceHashes[path.relative(root, absolute)] = hash(await readFile(absolute))
  }
  const report = { result: 'PASS', task: 'UIV01', assignmentBase: 'be85021c1e28f0b0f4a6182c4300a93b16179f5b',
    testedCheckoutHead: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
    scope: 'Local deterministic component presentation; no backend, app adoption, character art or physical-device acceptance.',
    browser: browser.version(), node: process.version, reducedMotion: true, results, sourceHashes, screenshots,
    reference: { path: 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg', sha256: hash(await readFile(path.join(root, 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg'))) },
    errors, externalRequests }
  await writeFile(path.join(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(`PASS UIV01: 3 widths, 15 screenshots; keyboard, local primary disclosure, truthful states, 44px controls, navigation, no overflow/errors/external requests. Chrome ${browser.version()}`)
} finally {
  await browser?.close()
  await new Promise(resolve => server ? server.httpServer.close(resolve) : resolve())
  await rm(outDir, { recursive: true, force: true })
}
