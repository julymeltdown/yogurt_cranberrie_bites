// PI11 task-34 layout QA. Real feature components in the retained shell frame;
// synthetic controller state; no live service, production art or location claim.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { mkdtemp, rm, mkdir, writeFile, copyFile, unlink } from 'node:fs/promises'
const worktree = '/Users/lhs/Desktop/mcr-worktrees/exec-w6-pi11', evidence = '/Users/lhs/Desktop/mcr/.omo/evidence/mcr-finish-project/task-34-pi11/layout'
const front = resolve(worktree, 'frontend'), output = evidence
const require = createRequire(resolve(front, 'package.json'))
const { createServer } = await import(require.resolve('vite'))
const { chromium, expect } = require('@playwright/test')
const pageDir = resolve(front, 'src/features/world-rooms/__tests__'), pageFiles = ['task34-layout.tsx', 'task34-layout.html']
for (const name of pageFiles) await copyFile(resolve('/Users/lhs/Desktop/mcr/.omo/evidence/mcr-finish-project/task-34-pi11/page', name), resolve(pageDir, name))
const tmp = await mkdtemp('/private/tmp/mcr-task34-')
const server = await createServer({ root: front, configLoader: 'runner', cacheDir: resolve(tmp, 'vite'), logLevel: 'error', server: { host: '127.0.0.1', port: 5430, hmr: false } })
const report = { status: 'IN_PROGRESS', source: (await import('node:child_process')).execFileSync('git', ['rev-parse', 'HEAD'], { cwd: worktree, encoding: 'utf8' }).trim(), cases: [], captures: [] }
const interactive = '.world-access button, .world-access a, .world-help a, .pi-avatar-popover button, .pi-avatar-popover a, .shell-panel button, .shell-panel a'
function overlap(a, b) {
  const x = Math.min(a.right, b.right) - Math.max(a.left, b.left), y = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top)
  return x > 1 && y > 1 ? { width: x, height: y } : null
}
async function geometry(page, label) {
  const result = await page.evaluate(selector => {
    const elements = Array.from(document.querySelectorAll(selector)).filter(element => {
      const style = getComputedStyle(element), rect = element.getBoundingClientRect()
      return style.display !== 'none' && style.visibility !== 'hidden' && rect.width > 0 && rect.height > 0
    })
    const controls = elements.map(element => {
      const rect = element.getBoundingClientRect(), style = getComputedStyle(element)
      return { name: element.getAttribute('aria-label') || element.textContent?.trim() || element.tagName,
        left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height,
        clippedX: element.scrollWidth - element.clientWidth, clippedY: element.scrollHeight - element.clientHeight,
        outline: style.outlineStyle }
    })
    const overlaps = []
    for (let i = 0; i < controls.length; i++) for (let j = i + 1; j < controls.length; j++) {
      const a = controls[i], b = controls[j]
      if (a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom) overlaps.push({ a: a.name, b: b.name })
    }
    return { overflow: document.documentElement.scrollWidth - innerWidth, controls, overlaps,
      panel: (() => { const r = document.querySelector('.shell-panel')?.getBoundingClientRect(); return r && { top: r.top, bottom: r.bottom, height: r.height } })() }
  }, interactive)
  for (const control of result.controls) {
    if (control.width < 44 || control.height < 44) throw Error(`${label}: control ${control.name} is ${control.width}x${control.height}`)
    if (control.clippedX > 1 || control.clippedY > 1) throw Error(`${label}: control ${control.name} clips text ${control.clippedX}x${control.clippedY}`)
  }
  if (result.overflow > 0) throw Error(`${label}: horizontal overflow ${result.overflow}px`)
  if (result.overlaps.length) throw Error(`${label}: overlapping controls ${JSON.stringify(result.overlaps)}`)
  return result
}
async function capture(page, name) {
  await page.evaluate(() => document.fonts.ready)
  await page.waitForTimeout(120)
  await page.screenshot({ path: resolve(output, name), fullPage: false })
  report.captures.push(name)
}
await mkdir(output, { recursive: true })
await server.listen()
const browser = await chromium.launch({ channel: 'chrome', headless: true })
report.browser = browser.version()
try {
  const cases = [[390, 844, 'full'], [430, 844, 'full'], [390, 420, 'full'], [360, 844, 'full']]
  for (const [width, height, panel] of cases) {
    const context = await browser.newContext({ viewport: { width, height }, isMobile: true, hasTouch: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
    for (const screen of ['rooms', 'nearby', 'help']) {
      const page = await context.newPage(), errors = []
      page.on('pageerror', error => errors.push(error.message))
      await page.goto(`http://127.0.0.1:5430/src/features/world-rooms/__tests__/task34-layout.html?screen=${screen}&panel=${panel}`)
      await page.getByRole('heading', { level: 2 }).first().waitFor()
      const label = `${screen}-${width}x${height}-${panel}`
      const row = { label, screen, width, height, panel, errors: [] }
      row.layout = await geometry(page, label)
      if (screen === 'rooms' && width === 390 && height === 844 && panel === 'full') {
        const change = page.getByRole('button', { name: 'Switch to SECOND ROOM', exact: true })
        await change.click()
        await expect(page.getByText('Room switch confirmed.', { exact: true })).toBeVisible()
        await expect(page.getByText('You are now in SECOND ROOM.', { exact: true })).toBeVisible()
        row.mandatorySwitch = await page.getByRole('region', { name: 'Room switch status' }).evaluate(element => element.contains(document.activeElement))
        row.layout = await geometry(page, `${label}-admitted`)
      }
      if (screen === 'nearby' && [390, 430].includes(width) && panel === 'full') {
        await page.getByRole('button', { name: /View .* avatar/ }).first().click()
        await expect(page.getByRole('dialog')).toBeVisible()
        row.layout = await geometry(page, `${label}-popover`)
        await capture(page, `${label}-popover.png`)
        await page.keyboard.press('Escape')
      }
      if (height <= 550) {
        const count = await page.locator(interactive).count()
        row.focusChecks = []
        for (let index = 0; index < count; index++) {
          const control = page.locator(interactive).nth(index)
          await control.scrollIntoViewIfNeeded(); await control.focus()
          const check = await control.evaluate(element => {
            const r = element.getBoundingClientRect(), hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)
            return { name: element.getAttribute('aria-label') || element.textContent?.trim(), top: r.top, bottom: r.bottom,
              visible: r.top >= 0 && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth, hit: hit === element || element.contains(hit) }
          })
          if (!check.visible || !check.hit) throw Error(`${label}: control ${check.name} is ${check.visible ? 'occluded' : 'clipped'} after focus`)
          row.focusChecks.push(check.name)
        }
      }
      if (screen === 'help') {
        const last = page.getByRole('link', { name: 'Return to world', exact: true })
        await last.scrollIntoViewIfNeeded(); await last.focus()
        row.lastVisible = await last.evaluate(element => {
          const r = element.getBoundingClientRect()
          return r.top >= 0 && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth
        })
        if (!row.lastVisible) throw Error(`${label}: final help link is not fully visible after focus`)
      }
      if (errors.length) throw Error(`${label}: ${errors.join(' | ')}`)
      await capture(page, `${label}.png`)
      row.status = 'PASS'
      report.cases.push(row)
      await page.close()
    }
    await context.close()
  }
  report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.error = String(error.stack ?? error); process.exitCode = 1 }
finally {
  await browser.close(); await server.close(); await rm(tmp, { recursive: true, force: true })
  for (const name of pageFiles) await unlink(resolve(pageDir, name)).catch(() => {})
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ status: report.status, cases: report.cases.length, captures: report.captures.length, error: report.error?.slice(0, 400) }))
}
