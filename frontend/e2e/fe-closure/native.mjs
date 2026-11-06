import assert from 'node:assert/strict'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const here = import.meta.dirname
const frontend = resolve(here, '../..')
const runtime = JSON.parse(await readFile(resolve(here, 'runtime.json'), 'utf8'))
const output = resolve(here, 'evidence')
await mkdir(output, { recursive: true })
const report = { result: 'FAIL', qualification: 'REAL_DEVELOPMENT_STACK', cases: [], errors: [] }
const vite = await createServer({
  root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner',
  cacheDir: resolve(here, '.vite-cache'), logLevel: 'warn',
  server: { host: '127.0.0.1', port: Number(new URL(runtime.vite_origin).port), strictPort: true,
    proxy: { '/api': { target: runtime.gateway_url, changeOrigin: false } } },
  plugins: [{ name: 'fe43-runtime', configureServer(server) {
    server.middlewares.use((request, response, next) => {
      const path = request.url?.split('?')[0]
      if (path === '/__fe43/runtime.json') {
        response.setHeader('Content-Type', 'application/json'); response.setHeader('Cache-Control', 'no-store')
        response.end(JSON.stringify(runtime)); return
      }
      if (path === '/__fe43/' || path === '/__fe43') {
        server.transformIndexHtml(request.url, `<!doctype html><html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FE43 producer closure</title><link rel="stylesheet" href="/src/shared/styles/base.css"><link rel="stylesheet" href="/src/app/shell.css"><style>body{margin:0;background:#f6efe2;color:#231f1a;font-family:system-ui,sans-serif}.mcr-preview-frame{padding:12px;min-height:100vh;box-sizing:border-box}.mcr-mural-preview{padding:12px}</style></head><body><div id="root"></div><script type="module" src="/e2e/fe-closure/preview.tsx"></script></body></html>`)
          .then(html => { response.setHeader('Content-Type', 'text/html'); response.end(html) })
          .catch(next)
        return
      }
      next()
    })
  } }]
})
await vite.listen()
const origin = runtime.vite_origin

async function overflow(page) {
  return page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
}
async function targets(page) {
  return page.evaluate(() => [...document.querySelectorAll('button, select, input[type="checkbox"]')].flatMap(node => {
    if (node.id === 'fe43-save') return []
    const measured = node.matches('input[type="checkbox"]') ? node.closest('label') ?? node : node
    const rect = measured.getBoundingClientRect(), style = getComputedStyle(measured)
    if (rect.width === 0 || rect.height === 0 || style.visibility === 'hidden' || style.display === 'none') return []
    return rect.width < 43.5 || rect.height < 43.5
      ? [{ tag: node.tagName, label: node.getAttribute('aria-label') ?? node.textContent?.trim().slice(0, 40), width: Math.round(rect.width), height: Math.round(rect.height) }]
      : []
  }))
}
async function accents(page) {
  return page.evaluate(() => [...document.querySelectorAll('*')].flatMap(node => {
    const s = getComputedStyle(node)
    const left = parseFloat(s.borderLeftWidth), right = parseFloat(s.borderRightWidth)
    const colored = s.borderLeftColor !== 'rgba(0, 0, 0, 0)' && s.borderLeftColor !== 'transparent'
    const border = left >= 2 && right === 0 && colored
    const before = getComputedStyle(node, '::before')
    const pseudo = before.content !== 'none' && before.content !== 'normal' && before.position === 'absolute'
      && (before.left === '0px' || before.left === '0') && parseFloat(before.width) > 0 && parseFloat(before.width) <= 6
      && parseFloat(before.height) > 12
    return border || pseudo ? [{ tag: node.tagName, className: String(node.className).slice(0, 80), border, pseudo }] : []
  }))
}
const shots = []
let browser
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const [width, height] of [[360, 800], [390, 844], [430, 932]]) {
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    try {
      for (const [name, query, ready] of [
        ['mural-happy', 'case=mural&role=alice', async page => { await expect(page.getByText(/Wall synchronized at revision \d+\./)).toBeVisible({ timeout: 30_000 }) }],
        ['mural-unavailable', 'case=mural-unavailable&role=alice', async page => { await expect(page.getByText('Wall synchronization unavailable. Reconnect the world and refresh before editing.')).toBeVisible({ timeout: 30_000 }) }],
        ['profile-happy', 'case=profile&role=alice', async page => {
          await expect(page.getByRole('checkbox').first()).toBeEnabled({ timeout: 30_000 })
          await page.getByRole('button', { name: 'Harness save' }).click()
          await expect(page.getByText('Preferences saved. Sound only starts after you explicitly apply it in this session.').first()).toBeVisible({ timeout: 30_000 })
        }],
        ['profile-conflict', 'case=profile-conflict&role=alice', async page => {
          await expect(page.getByRole('checkbox').first()).toBeEnabled({ timeout: 30_000 })
          await page.getByRole('button', { name: 'Harness save' }).click()
          await expect(page.getByText(/Your profile changed elsewhere/).first()).toBeVisible({ timeout: 30_000 })
        }],
      ]) {
        const page = await context.newPage()
        const errors = []
        page.on('pageerror', error => errors.push(error.message))
        await page.goto(`${origin}/__fe43/?${query}`)
        try { await ready(page) } catch (error) {
          report.failed = { name, width, text: await page.locator('body').innerText().catch(() => '') }
          throw error
        }
        const result = { name, width, errors,
          overflow: await overflow(page), smallTargets: await targets(page), accents: await accents(page) }
        const file = `${name}-${width}.png`
        await page.screenshot({ path: resolve(output, file), fullPage: true })
        result.screenshot = file
        shots.push(file)
        report.cases.push(result)
        assert.deepEqual(errors, [], `${name}/${width} page errors`)
        assert.equal(result.overflow, true, `${name}/${width} overflows`)
        assert.deepEqual(result.smallTargets, [], `${name}/${width} sub-44 control`)
        assert.deepEqual(result.accents, [], `${name}/${width} decorative left accent`)
        await page.close()
      }
    } finally { await context.close() }
  }
  report.result = 'PASS'
} catch (error) {
  report.error = error instanceof Error ? error.message : String(error)
} finally {
  await browser?.close().catch(() => {})
  await vite.close()
  report.screenshots = shots
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2))
  await writeFile(resolve(here, 'browser.done'), 'done\n')
}
if (report.result !== 'PASS') throw new Error(report.error ?? 'browser qualification failed')
console.log(JSON.stringify({ result: report.result, cases: report.cases.length, screenshots: shots.length }))
