// QA06 mobile visual acceptance runner. Real default application composition at
// 360/390/430 CSS px. Synthetic HTTP/C05 and isolated map admission; no product approval.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, statfs, writeFile, rm } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium } from '@playwright/test'
import { CLOCK, FAMILIES, FAMILY_CASES, IA, RELEASE, WIDTHS } from './matrix.mjs'

const repo = resolve(import.meta.dirname, '../../..'), frontend = resolve(repo, 'frontend')
const out = process.env.QA06_MOBILE_OUTPUT
assert(out?.startsWith('/'), 'QA06_MOBILE_OUTPUT must be a new absolute evidence directory')
await mkdir(out, { recursive: true })
await mkdir(resolve(out, 'captures'), { recursive: true })
const space = await statfs(repo)
assert(space.bavail * space.bsize >= 8 * 1024 ** 3, 'Disk floor below 8 GiB: no capture permitted')
const hash = b => createHash('sha256').update(b).digest('hex')
const git = (...args) => execFileSync('git', args, { cwd: repo, encoding: 'utf8' }).trim()
const targetBytes = await readFile(resolve(repo, 'docs/mcr/design/reference-target-2026-11-04.png'))

const BLOCKERS = [
  [/venue map is unavailable/i, 'reviewed siteplan + landmark provider not composed in this fixture'],
  [/Place information could not be checked/i, 'reviewed landmark/place detail provider (place response rejected by the places entry contract)'],
  [/Community action unavailable/i, 'community entry release/clock authority not composed (no VITE_MCR_COMMUNITY_CLOCK_INPUT qualification)'],
  [/Gift access is unavailable|Event access required|Party information is unavailable/i, 'community clock/entry authority not composed; gift and party data cannot be admitted'],
  [/Wall unavailable/i, 'qualified world mural-scene release/anchor not composed'],
  [/This section is unavailable/i, 'requests/social provider not composed for this fixture'],
  [/safety service is unavailable/i, 'safety service producer (blocks/report transport) not composed in this fixture'],
  [/Administration unavailable/i, 'operator role/event settings server authority not available to a participant fixture'],
  [/Real-world meeting guidance is unavailable/i, 'reviewed landmark/place provider not composed'],
  [/Editing has ended or is unavailable/i, 'post edit ownership/state producer unavailable for the other-author fixture post'],
  [/This document is not available yet/i, 'approved legal document copy not supplied'],
  [/This archive is unavailable/i, 'published archive content producer not supplied'],
  [/Email account actions are not available right now/i, 'email signup/verification provider not composed in this fixture'],
  [/Review tools unavailable|Review unavailable/i, 'operator/moderator review authority not available to a participant fixture'],
  [/PROVIDER SIGN-IN IS UNAVAILABLE/i, 'OAuth provider sign-in not composed in this fixture'],
]
function blocker(text) { for (const [pattern, label] of BLOCKERS) if (pattern.test(text)) return label; return null }

const report = {
  task: 'QA06', kind: 'mcr-qa06-mobile-visual-v1', startedAt: new Date().toISOString(),
  candidate: git('rev-parse', 'HEAD'), base: '7562f3b96', branch: git('rev-parse', '--abbrev-ref', 'HEAD'),
  qualification: 'DESKTOP_CHROME_TOUCH_VIEWPORTS_ONLY',
  physicalIOS: 'NOT_RUN', physicalAndroid: 'NOT_RUN', androidEmulator: 'PENDING', softwareKeyboard: 'NOT_RUN', hardwareSafeArea: 'NOT_RUN',
  reference: { tenScreen: { path: 'docs/mcr/design/reference-target-2026-11-04.png', sha256: hash(targetBytes), width: 1536, height: 1024 },
    paradeInk: 'docs/mcr/design/parade-ink-2026-11-05/parade-ink.jpg',
    venueStateMap: 'docs/mcr/design/venue-redesign-2026-11-05/reference-state-map.json' },
  families: FAMILIES, clock: CLOCK, widths: WIDTHS, cases: [], findings: [], createdResources: [],
}
let browser
const server = await createServer({ root: frontend, configLoader: 'runner', logLevel: 'error',
  define: { 'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify(RELEASE) },
  server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false } })
report.createdResources.push('vite-dev-server')
try {
  await server.listen(); const base = server.resolvedUrls.local[0]
  browser = await chromium.launch({ channel: 'chrome', headless: true }); report.createdResources.push('chrome')
  report.browser = browser.version(); report.node = process.version
  // Warm module graph so the first public route is not measured against a cold transform.
  { const c = await browser.newContext({ viewport: { width: 390, height: 844 } }), p = await c.newPage()
    await p.goto(base + 'e2e/visual/qa06/preview.html?' + new URLSearchParams({ entry: '/world' }))
    await p.waitForFunction(() => window.__QA06__, {}, { timeout: 60000 }).catch(() => {})
    await c.close() }
  const capture = async (caseId, item, width, opts = {}) => {
    const context = await browser.newContext({ viewport: { width, height: opts.height ?? 844 }, deviceScaleFactor: 1, hasTouch: true, isMobile: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
    const page = await context.newPage(), errors = [], requests = []
    page.on('pageerror', error => errors.push(error.message.slice(0, 240)))
    page.on('response', response => { const url = new URL(response.url()); if (url.pathname.startsWith('/api/')) requests.push(response.status() + ' ' + url.pathname) })
    page.setDefaultTimeout(15000)
    const entry = { caseId, family: item.family, ia: item.ia ?? null, route: item.route, state: item.state, width,
      height: opts.height ?? 844, devicePixelRatio: 1, browser: report.browser, status: 'IN_PROGRESS' }
    try {
      await page.clock.setFixedTime(new Date(CLOCK))
      const query = { entry: item.route, ...(item.era ? { era: item.era } : {}), ...(opts.mode ? { mode: opts.mode } : {}) }
      await page.goto(base + 'e2e/visual/qa06/preview.html?' + new URLSearchParams(query))
      await page.waitForFunction(() => window.__QA06__, {}, { timeout: 45000 })
      await page.waitForTimeout(item.public ? 900 : 2500)
      if (opts.offline) { await page.evaluate(() => window.__QA06__.offline()); await page.waitForTimeout(1200) }
      if (opts.focus) await page.evaluate(() => { const element = document.querySelector('main button:not(:disabled), main a[href], main input'); element?.focus() })
      if (!opts.focus) await page.evaluate(() => { if (document.activeElement instanceof HTMLElement) document.activeElement.blur() })
      await page.evaluate(() => document.fonts.ready)
      const state = await page.evaluate(() => window.__QA06__.state())
      const metrics = await page.evaluate(() => {
        const box = element => { if (!element) return null; const r = element.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
        const visible = element => element.checkVisibility() && element.getBoundingClientRect().width > 1 && element.getBoundingClientRect().height > 1
        const controls = [...document.querySelectorAll('button,a[href],input,select,textarea,summary,[tabindex="0"]')].filter(visible)
        const target = element => element.matches('input[type=checkbox],input[type=radio]') ? (element.labels?.[0] ?? element) : element
        return { viewport: { width: innerWidth, height: innerHeight }, document: { width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight },
          overflow: document.documentElement.scrollWidth > innerWidth + 1,
          text: document.querySelector('main')?.innerText.slice(0, 4000) ?? '',
          headings: [...document.querySelectorAll('h1,h2,h3')].filter(visible).map(element => element.textContent?.trim()).slice(0, 12),
          smallControls: controls.filter(element => !element.matches(':disabled') && (target(element).getBoundingClientRect().width < 43.5 || target(element).getBoundingClientRect().height < 43.5))
            .map(element => ({ label: (element.getAttribute('aria-label') || element.textContent || element.tagName).trim().slice(0, 80), tag: element.tagName, rect: box(target(element)) })),
          frame: box(document.querySelector('.world-shell, .public-entry, .public-poster, .auth-sign-in')),
          panel: box(document.querySelector('.shell-panel')),
          titlebar: box(document.querySelector('.mcr-chrome-panel-header, .public-header, .auth-paper-title')),
          tabs: box(document.querySelector('.mcr-chrome-eras, .party-tabs, .feed-tabs, .mcr-profile-tabs, .gifts-reference [role=tablist]')),
          footer: box(document.querySelector('.mcr-chrome-parade-footer, .feed-footer, .public-entry-action')),
          navigation: box(document.querySelector('.mcr-chrome-bottom')),
          world: box(document.querySelector('.shell-world')),
          images: [...document.images].filter(visible).map(element => ({ src: element.currentSrc.startsWith('blob:') ? 'BLOB' : element.currentSrc.startsWith('data:') ? 'DATA' : new URL(element.currentSrc).pathname, complete: element.complete, naturalWidth: element.naturalWidth, rect: box(element) }))
        }
      })
      const file = `${caseId}-${width}${opts.suffix ? '-' + opts.suffix : ''}.png`
      await page.screenshot({ path: resolve(out, 'captures', file), fullPage: false })
      entry.capture = 'captures/' + file
      entry.captureSha256 = hash(await readFile(resolve(out, 'captures', file)))
      entry.stateObserved = { path: state.path, published: state.published, theme: state.theme?.selected ?? null, entities: state.entities, canvases: state.canvases }
      entry.measurements = { ...metrics, text: metrics.text.slice(0, 1200) }
      entry.requests = [...new Set(requests)]
      entry.pageErrors = errors
      entry.blocker = blocker(metrics.text)
      entry.findings = []
      if (metrics.overflow) entry.findings.push('HORIZONTAL_OVERFLOW')
      if (metrics.smallControls.length) entry.findings.push('TARGET_UNDER_44')
      if (errors.length) entry.findings.push('PAGE_ERROR:' + errors[0])
      if (!metrics.text.trim()) entry.findings.push('EMPTY_MAIN')
      entry.verdict = errors.length || !metrics.text.trim() ? 'FAIL'
        : entry.blocker ? 'NOT_REACHABLE'
          : entry.findings.length ? 'DIFFER' : 'PASS'
      if (opts.verdict) entry.verdict = opts.verdict
      if (opts.materialDifference) entry.materialDifference = opts.materialDifference
      report.findings.push(...entry.findings.map(code => ({ caseId, width, code })))
    } catch (error) { entry.verdict = 'FAIL'; entry.error = String(error.message).slice(0, 600) }
    finally { await page.evaluate(() => window.__QA06__?.dispose?.()).catch(() => {}); await context.close() }
    report.cases.push(entry)
    console.log(JSON.stringify({ caseId, width, verdict: entry.verdict }))
    await writeFile(resolve(out, 'report.partial.json'), JSON.stringify(report, null, 2) + '\n')
    return entry
  }
  const only = process.env.QA06_CASE ? new Set(process.env.QA06_CASE.split(',')) : null
  const widths = process.env.QA06_WIDTHS ? process.env.QA06_WIDTHS.split(',').map(Number) : WIDTHS
  report.selectedWidths = widths
  for (const width of widths) {
    for (const item of IA) {
      if (only && !only.has(item.id) && !only.has(item.family)) continue
      if (item.id === 'S02') { await capture('S02', { ...item, ia: item.id, route: '/feed' }, width, { mode: 'error' }); continue }
      if (item.id === 'S03') { await capture('S03', { ...item, ia: item.id }, width, { offline: true }); continue }
      await capture(item.id, { ...item, ia: item.id }, width)
    }
    for (const item of FAMILY_CASES) {
      if (only && !only.has(item.id)) continue
      if (item.id === 'PI03-inline-reply') { await capture(item.id, item, width); continue }
      await capture(item.id, { ...item, ia: null }, width)
    }
  }
  if (!only) for (const width of [390]) {
    await capture('PI01-short', { family: 'PI01', route: '/feed', state: 'short-height panel chrome' }, width, { height: 420, suffix: 'short420' })
    await capture('PI01-focus', { family: 'PI01', route: '/feed', state: 'keyboard focus outline' }, width, { suffix: 'focus', focus: true })
  }
} finally {
  report.finishedAt = new Date().toISOString()
  await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n')
  await writeFile(resolve(out, 'sha256.txt'), report.cases.filter(c => c.capture).map(c => `${c.captureSha256}  ${c.capture}`).join('\n') + '\n')
  await browser?.close(); await server.close()
  await rm(resolve(out, 'report.partial.json'), { force: true })
}
const counts = report.cases.reduce((all, c) => ({ ...all, [c.verdict]: (all[c.verdict] ?? 0) + 1 }), {})
console.log(JSON.stringify({ counts, cases: report.cases.length, output: out }))
