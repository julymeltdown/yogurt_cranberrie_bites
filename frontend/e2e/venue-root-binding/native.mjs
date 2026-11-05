import { createRequire } from 'node:module'
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { immutableInputs, frontend, pins } from './readset.mjs'
import { aliases } from '../../src/features/admin/service-policy/__tests__/vitest.config.mjs'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES, scratch = process.env.MCR_FE38_SCRATCH, output = process.env.MCR_FE38_OUTPUT
if (!dependencies || !scratch || !output) throw Error('Fresh accepted native allocation required')
const require = createRequire(resolve(dependencies, 'package.json'))
const report = { classification: 'CONTROLLED_FIXTURE_NOT_DEFAULT_APP_LIVE_DEVICE_OR_VENU_ACCEPTANCE', pins, widths: [], pageErrors: [] }
mkdirSync(output, { recursive: true })
let server, browser
const assert = (value, why) => { if (!value) throw Error(why) }
try {
  const { createServer } = await import(pathToFileURL(require.resolve('vite')))
  const { default: react } = await import(pathToFileURL(require.resolve('@vitejs/plugin-react')))
  const { chromium } = require('@playwright/test')
  server = await createServer({ configFile: false, root: frontend, plugins: [immutableInputs(), react()], resolve: { alias: aliases },
    cacheDir: resolve(scratch, 'venue-root-vite'), server: { host: '127.0.0.1', port: 5384, strictPort: true, fs: { allow: [frontend, dependencies] } } })
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true,
    args: ['--disable-background-networking', '--disable-component-update', '--disable-default-apps', '--no-first-run', '--disable-crash-reporter'] })
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, deviceScaleFactor: 1 })
    const page = await context.newPage(); page.setDefaultTimeout(8000); page.on('pageerror', e => report.pageErrors.push(String(e)))
    await page.goto('http://127.0.0.1:5384/e2e/venue-root-binding/native.html')
    await page.getByRole('tab', { name: 'FRIENDS', exact: true }).waitFor()
    const canvas = await page.locator('canvas').elementHandle()
    await page.getByRole('tab', { name: 'FRIENDS', exact: true }).click()
    await page.evaluate(() => window.venueRootFixture.openMessage())
    await page.evaluate(() => window.venueRootFixture.draft('Retained controlled draft'))
    await page.getByRole('link', { name: 'Meeting route', exact: true }).click()
    await page.getByRole('textbox').waitFor()
    assert(await page.getByRole('textbox').inputValue() === 'Retained controlled draft', 'Draft was not retained')
    await page.screenshot({ path: resolve(output, `${width}-retained-message.png`), fullPage: true })
    await page.getByRole('link', { name: 'Map route', exact: true }).click()
    assert(await page.getByRole('tab', { name: 'FRIENDS', exact: true }).getAttribute('aria-selected') === 'true', 'Map state was lost across Outlet remount')
    await page.evaluate(() => window.venueRootFixture.openInvite())
    await page.getByRole('link', { name: 'Meeting route', exact: true }).click()
    await page.getByRole('button', { name: /View on map/i }).click()
    await page.waitForFunction(() => document.querySelector('[aria-label="Fixture route"]')?.textContent === '/map')
    await page.screenshot({ path: resolve(output, `${width}-map-focus-return.png`), fullPage: true })
    const state = await page.evaluate(() => window.venueRootFixture.inspect())
    assert(state.owners === 1 && state.clockSame && state.borrowedSame, 'Provider/clock/input identity changed')
    assert(state.counts.keys === 0 && state.counts.phoneStarts === 0 && state.counts.requests === 0, 'Unexpected command/watch/Auth request')
    assert(await canvas.evaluate(el => el.isConnected && document.querySelectorAll('canvas').length === 1), 'Borrowed canvas sentinel remounted')
    await page.evaluate(() => window.venueRootFixture.revoke())
    await page.getByText(/venue map is unavailable/).waitFor()
    assert(await page.getByRole('textbox').count() === 0, 'Private draft retained after withdrawal')
    await page.screenshot({ path: resolve(output, `${width}-withdrawn.png`), fullPage: true })
    report.widths.push({ width, state, retainedMessage: true, focusRouteReturn: true, withdrawal: true, sameBorrowedCanvasSentinel: true })
    await page.evaluate(() => window.venueRootFixture.dispose()); await context.close()
  }
  assert(report.pageErrors.length === 0, 'Page errors'); report.status = 'PASS'
} catch (error) { report.status = 'FAIL'; report.failure = String(error); throw error }
finally { await browser?.close(); await server?.close(); writeFileSync(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n') }
