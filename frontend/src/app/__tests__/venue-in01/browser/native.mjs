import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const frontend = resolve(import.meta.dirname, '../../../../..')
const evidence = resolve(import.meta.dirname, '../../../../../../.omo/evidence/mcr-finish-project/task-28-venu-in01')
const widths = [[360, 800], [390, 844], [430, 932]]
await mkdir(resolve(evidence, 'captures/states'), { recursive: true })
await mkdir(resolve(evidence, 'metrics'), { recursive: true })

const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: '/tmp/mcr-venue-in01-cache',
  server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const report = { result: 'FAIL', qualification: 'CONTROLLED_FIXTURE', generatedAt: new Date().toISOString(),
  note: 'Actual VenueShellFeatures production sources, frozen clients, siteplan loader and Map/Notes/Meeting panels. Deterministic session, community clock, Location, landmark, Note and Meeting transport rows. No live backend, device, physical venue or character runtime.',
  widths: widths.map(([width, height]) => ({ width, height })), states: {}, cases: [], ia: [], errors: [] }
let browser, page, stage = 'start'

function track(next) {
  next.on('pageerror', error => report.errors.push(`${stage}: ${error.message}`))
  next.on('console', message => { if (message.type() === 'error' && !message.text().includes('Failed to load resource')) report.errors.push(`${stage}: console ${message.text()}`) })
  return next
}
async function go(target, query) {
  await target.goto(`${origin}/src/app/__tests__/venue-in01/browser/fixture.html?${query}`)
  await target.mouse.move(1, 1)
  await expect(target.getByRole('tab', { name: 'MAP' })).toBeEnabled({ timeout: 20000 })
}
async function route(target, query) {
  await target.goto(`${origin}/src/app/__tests__/venue-in01/browser/fixture.html?${query}`)
  await target.mouse.move(1, 1)
  await expect(target.locator('.venue-meeting')).toBeVisible({ timeout: 20000 })
}
async function openNotes(target) {
  await go(target, 'state=default')
  await target.getByRole('tab', { name: 'NOTES' }).click()
  await expect(target.getByLabel('Places with notes').getByRole('button', { name: 'Main Gate, 2 notes' })).toBeVisible({ timeout: 15000 })
}
async function openSelectedNotes(target) {
  await openNotes(target)
  await target.getByLabel('Places with notes').getByRole('button', { name: 'Main Gate, 2 notes' }).click()
  await expect(target.getByRole('heading', { name: 'MAIN GATE / 2 NOTES' })).toBeVisible({ timeout: 15000 })
}
async function openComposer(target) {
  await openSelectedNotes(target)
  await target.getByRole('button', { name: '+ NEW NOTE' }).click()
  await expect(target.getByRole('group', { name: 'Visible to' })).toBeVisible({ timeout: 10000 })
  await target.getByRole('textbox', { name: 'Note text' }).fill('Waiting near Main Gate in a black parade jacket.')
}
async function openSelectedFriend(target) {
  await go(target, 'state=map-markers')
  await target.getByRole('button', { name: 'View cluster of 3 friends' }).click()
  await expect(target.getByRole('region', { name: 'Friend cluster' })).toBeVisible({ timeout: 10000 })
  await target.locator('.pc-cluster-person').first().click()
  await expect(target.getByRole('dialog')).toBeVisible({ timeout: 10000 })
  await target.getByLabel('Friend meeting place').selectOption('main-gate')
}
const states = {
  'whole-plan': async p => { await go(p, 'state=default'); await p.getByRole('button', { name: 'Fit complete siteplan' }).click(); await p.waitForTimeout(300) },
  'selected-friend': openSelectedFriend,
  conversation: async p => { await route(p, 'state=conversation&route=invitation'); await p.getByRole('button', { name: 'Message about this place' }).click(); await expect(p.getByRole('textbox', { name: 'Message' })).toBeVisible({ timeout: 10000 }) },
  'place-card': async p => { await go(p, 'state=map-markers'); await p.locator('.pc-landmark[data-landmark-id="main-gate"]').evaluate(el => el.click()); await expect(p.getByLabel('MAIN GATE location card')).toBeVisible({ timeout: 10000 }) },
  friends: async p => { await go(p, 'state=default'); await p.getByRole('tab', { name: 'FRIENDS' }).click(); await expect(p.getByRole('button', { name: 'inkparade' })).toBeVisible({ timeout: 10000 }) },
  'map-markers': async p => { await go(p, 'state=map-markers'); await expect(p.getByRole('button', { name: 'View cluster of 3 friends' })).toBeVisible({ timeout: 10000 }) },
  'invitation-pending': async p => { await route(p, 'state=pending-invite&route=invitation'); await expect(p.getByRole('button', { name: 'ACCEPT' })).toBeVisible({ timeout: 15000 }) },
  'notes-selected': openSelectedNotes,
  'composer-friends': openComposer,
  markers: async p => { await go(p, 'state=map-markers'); await p.getByRole('tab', { name: 'NOTES' }).click(); await expect(p.getByLabel('Places with notes').getByRole('button', { name: 'Main Gate, 2 notes' })).toBeVisible({ timeout: 15000 }) },
  'composer-everyone': async p => { await openComposer(p); await p.getByRole('button', { name: 'EVERYONE' }).click(); await p.getByRole('button', { name: 'END OF DAY' }).click() },
  reply: async p => { await openSelectedNotes(p); await p.getByRole('button', { name: 'READ REPLIES' }).first().click(); await expect(p.getByText('I will be there in a moment.')).toBeVisible({ timeout: 15000 }) },
  cluster: async p => { await go(p, 'state=map-markers'); await p.getByRole('button', { name: 'View cluster of 3 friends' }).click(); await expect(p.getByRole('region', { name: 'Friend cluster' })).toBeVisible({ timeout: 10000 }) },
  'sharing-active': async p => { await go(p, 'state=sharing-active'); await expect(p.getByText('Sharing with friends')).toBeVisible({ timeout: 10000 }) },
  'sharing-consent': async p => { await go(p, 'state=default'); await p.getByRole('button', { name: 'SHARE LOCATION' }).click(); await expect(p.getByRole('checkbox', { name: 'I want to share my phone location.' })).toBeVisible({ timeout: 10000 }) },
  'selected-phone': async p => { await go(p, 'state=default'); await p.getByRole('tab', { name: 'FRIENDS' }).click(); await p.getByRole('button', { name: 'inkparade' }).click(); await expect(p.getByRole('dialog')).toBeVisible({ timeout: 10000 }) },
  invite: async p => { await openSelectedFriend(p); await p.getByRole('button', { name: 'MEET HERE' }).click(); await expect(p.getByRole('button', { name: 'SEND INVITATION' })).toBeVisible({ timeout: 15000 }) },
  notes: openNotes,
  unavailable: async p => { await go(p, 'state=unavailable'); await expect(p.getByText('Friends unavailable').first()).toBeVisible({ timeout: 15000 }) },
}

async function measure(target) {
  return target.evaluate(() => {
    const box = element => { const rect = element.getBoundingClientRect(); return { x: +rect.x.toFixed(1), y: +rect.y.toFixed(1), width: +rect.width.toFixed(1), height: +rect.height.toFixed(1) } }
    const one = selector => { const element = document.querySelector(selector); return element ? box(element) : null }
    const many = selector => Array.from(document.querySelectorAll(selector)).map(box)
    const controls = Array.from(document.querySelectorAll('button,a,select')).filter(element => {
      const rect = element.getBoundingClientRect(); return rect.width > 0 && rect.height > 0
    })
    const minHit = controls.length ? Math.min(...controls.map(element => { const rect = element.getBoundingClientRect(); return Math.min(rect.width, rect.height) })) : null
    const font = selector => { const element = document.querySelector(selector); return element ? getComputedStyle(element).fontSize : null }
    return { viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio }, overflowX: document.documentElement.scrollWidth > innerWidth,
      tabs: one('.pc-map-tabs'), tab: one('.pc-map-tabs button'), map: one('.pc-map-canvas'), sheet: one('.pc-sheet'), sharing: one('.pc-sharing'),
      friendRows: many('.pc-friend-row'), friendPortrait: one('.pc-friend-row .pc-portrait'), noteCards: many('.pc-note-card'), notePortrait: one('.pc-note-card .pc-portrait'),
      selectedPortrait: one('.pc-friend-detail .pc-portrait'), composer: one('.pc-composer'),
      primary: one('.pc-primary'), meeting: one('.venue-meeting'), controls: controls.length, minHit: minHit === null ? null : +minHit.toFixed(1),
      regions: { map: one('.pc-map'), tabPanel: one('.pc-tab-panel'), canvas: one('.pc-map-canvas'), friends: one('.pc-friends'),
        sheet: one('.pc-sheet'), noteCard: one('.pc-note-card'), composer: one('.pc-composer'), sharing: one('.pc-sharing'), friendDetail: one('.pc-friend-detail') },
      fonts: { name: font('.pc-name'), place: font('.pc-friend-place'), source: font('.pc-friend-source'), body: font('.pc-note-heading p'), noteMeta: font('.pc-note-heading small') },
      text: document.body.innerText.slice(0, 600) }
  })
}

try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const [width, height] of widths) {
    const context = await browser.newContext({ viewport: { width, height }, hasTouch: true, reducedMotion: 'reduce' })
    page = track(await context.newPage())
    for (const [state, prepare] of Object.entries(states)) {
      stage = `${width}/${state}`
      await prepare(page)
      await page.waitForTimeout(250)
      const metrics = await measure(page)
      assert.equal(metrics.overflowX, false, `${stage}: horizontal overflow`)
      assert.ok(metrics.minHit === null || metrics.minHit >= 43.9, `${stage}: hit target ${metrics.minHit}`)
      await page.evaluate(() => { if (document.activeElement instanceof HTMLElement) document.activeElement.blur() })
      await page.mouse.move(1, 1)
      await page.screenshot({ path: resolve(evidence, `captures/states/${state}-${width}.png`), animations: 'disabled' })
      await writeFile(resolve(evidence, `metrics/${state}-${width}.json`), JSON.stringify(metrics, null, 2))
      report.states[`${state}-${width}`] = metrics
    }
    report.cases.push({ width, height })
    await context.close()
  }
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.errors.push(`${stage}: ${error instanceof Error ? error.message : String(error)}`)
  if (page && typeof page.locator === 'function') report.body = await page.locator('body').innerText().catch(() => '')
} finally {
  await browser?.close(); await server.close()
  await writeFile(resolve(evidence, 'states.json'), JSON.stringify(report, null, 2))
}
if (report.result !== 'PASS') { console.error(JSON.stringify(report, null, 2)); process.exit(1) }
console.log(JSON.stringify({ result: report.result, states: Object.keys(report.states).length, errors: report.errors }))
