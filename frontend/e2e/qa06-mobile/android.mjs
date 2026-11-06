// QA06 Android Chrome pass on the available Android 15 emulator (not a physical device).
// Uses raw CDP so a background tab cannot stall a Playwright navigation.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium } from '@playwright/test'
import { RELEASE, WIDTHS } from './matrix.mjs'

const repo = resolve(import.meta.dirname, '../../..'), frontend = resolve(repo, 'frontend')
const out = process.env.QA06_MOBILE_OUTPUT
assert(out?.startsWith('/'), 'QA06_MOBILE_OUTPUT is required')
await mkdir(resolve(out, 'captures'), { recursive: true })
const adb = '/Users/lhs/Library/Android/sdk/platform-tools/adb'
const sh = args => execFileSync(adb, args, { encoding: 'utf8' }).trim()
const boot = sh(['shell', 'getprop', 'sys.boot_completed'])
const chromeVersion = (/versionName=([0-9.]+)/.exec(sh(['shell', 'dumpsys', 'package', 'com.android.chrome'])) ?? [])[1]
const model = sh(['shell', 'getprop', 'ro.product.model']), sdk = sh(['shell', 'getprop', 'ro.build.version.sdk'])
const androidVersion = sh(['shell', 'getprop', 'ro.build.version.release'])
const port = 5319
const server = await createServer({ root: frontend, configLoader: 'runner', logLevel: 'error',
  define: { 'import.meta.env.VITE_MCR_APPLICATION_RELEASE': JSON.stringify(RELEASE) },
  server: { host: '0.0.0.0', port, strictPort: true, hmr: false, allowedHosts: true } })
const routes = [
  ['W01', '/world'], ['F01', '/feed'], ['M07', '/meet/people'], ['U01', '/me'],
  ['U02', '/me/avatar'], ['W04', '/world/rooms'], ['U07', '/me/settings'], ['U10', '/notifications'],
  ['A01', '/'], ['A02', '/login'], ['W06', '/world/theme'], ['M09', '/people/00000000-0000-4000-8000-000000000002'],
]
const report = { kind: 'mcr-qa06-android', device: { model, sdk, android: androidVersion, boot, qualification: 'ANDROID_15_EMULATOR_CHROME_NOT_PHYSICAL' },
  chromeVersion, cases: [], ua: null }
const hash = b => createHash('sha256').update(b).digest('hex')
let browser
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
try {
  await server.listen()
  sh(['shell', 'am', 'force-stop', 'com.android.chrome'])
  sh(['reverse', 'tcp:5319', 'tcp:5319'])
  sh(['forward', 'tcp:9222', 'localabstract:chrome_devtools_remote'])
  sh(['shell', 'am', 'start', '-a', 'android.intent.action.VIEW', '-d', `http://localhost:${port}/e2e/visual/qa06/preview.html?entry=%2Fworld`, 'com.android.chrome'])
  await sleep(6000)
  browser = await chromium.connectOverCDP('http://127.0.0.1:9222')
  const context = browser.contexts()[0]
  const page = context.pages().find(candidate => candidate.url().includes('localhost:5319')) ?? context.pages()[0] ?? await context.newPage()
  const cdp = await context.newCDPSession(page)
  await cdp.send('Page.enable'); await cdp.send('Runtime.enable')
  const send = (method, params) => Promise.race([cdp.send(method, params), new Promise((_, reject) => setTimeout(() => reject(new Error('CDP_TIMEOUT ' + method)), 20000))])
  report.ua = (await cdp.send('Runtime.evaluate', { expression: 'navigator.userAgent', returnByValue: true })).result?.value
  for (const width of WIDTHS) {
    for (const [id, route] of routes) {
      const entry = { caseId: id, route, width, height: 844, devicePixelRatio: 2, state: 'android chrome' }
      try {
        await cdp.send('Emulation.setDeviceMetricsOverride', { width, height: 844, deviceScaleFactor: 2, mobile: true })
        await cdp.send('Emulation.setTouchEmulationEnabled', { enabled: true })
        const url = `http://localhost:${port}/e2e/visual/qa06/preview.html?` + new URLSearchParams({ entry: route })
        await Promise.race([page.bringToFront(), sleep(10000)])
        await send('Page.navigate', { url })
        let ready = false
        for (let attempt = 0; attempt < 30 && !ready; attempt++) {
          await sleep(1000)
          try { const value = await send('Runtime.evaluate', { expression: 'Boolean(window.__QA06__)', returnByValue: true }); ready = value.result?.value === true } catch { /* context swapping */ }
        }
        assert(ready, 'QA06 bridge did not initialize')
        await sleep(2000)
        try { await send('Runtime.evaluate', { expression: '(() => { if (document.activeElement instanceof HTMLElement) document.activeElement.blur() })()' }) } catch { /* capture follows regardless */ }
        const evalValue = async expression => { const value = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }); return value.result?.value }
        entry.stateObserved = await evalValue('(() => { const s = window.__QA06__.state(); return { path: s.path, published: s.published, canvases: s.canvases } })()')
        entry.metrics = await evalValue('({ viewport: { width: innerWidth, height: innerHeight }, dpr: devicePixelRatio, overflow: document.documentElement.scrollWidth > innerWidth + 1, text: (document.querySelector("main")?.innerText ?? "").slice(0, 300) })')
        const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
        const file = `captures/android-${id}-${width}.png`
        await writeFile(resolve(out, file), Buffer.from(shot.data, 'base64'))
        entry.capture = file; entry.captureSha256 = hash(Buffer.from(shot.data, 'base64'))
        entry.verdict = entry.metrics?.text.trim() ? 'CAPTURED' : 'FAIL'
      } catch (error) { entry.verdict = 'FAIL'; entry.error = String(error.message).slice(0, 400) }
      report.cases.push(entry); console.log(JSON.stringify({ id, width, verdict: entry.verdict }))
      await writeFile(resolve(out, 'android.partial.json'), JSON.stringify(report, null, 2) + '\n')
    }
  }
} finally {
  report.finishedAt = new Date().toISOString()
  await writeFile(resolve(out, 'android.json'), JSON.stringify(report, null, 2) + '\n')
  await browser?.close(); await server.close(); sh(['forward', '--remove', 'tcp:9222']); sh(['reverse', '--remove', 'tcp:5319'])
  try { execFileSync('/bin/rm', ['-f', resolve(out, 'android.partial.json')]) } catch { /* absent */ }
}
console.log(JSON.stringify({ cases: report.cases.length, failed: report.cases.filter(c => c.verdict === 'FAIL').length }))
