import assert from 'node:assert/strict'
import { chromium, expect } from '@playwright/test'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { bundle, serve } from './build.mjs'
const root = resolve(import.meta.dirname, '../../../../../..')
const evidence = resolve(import.meta.dirname, 'evidence')
await mkdir(evidence, { recursive: true })
const report = { result: 'FAIL', scope: 'UIV03 actual browse components / deterministic synthetic fixtures; existing root party/gift shell composition. Not live backend, production data, image qualification, real multiuser or whole-IA acceptance.',
  base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), browser: '', checks: [], geometry: [], screenshots: [], errors: [], sourceHashes: {} }
const sources = ['frontend/src/features/event-feed/EventFeed.tsx', 'frontend/src/features/event-feed/feed.css', 'frontend/src/features/party-discovery/ui/PartyDiscovery.tsx', 'frontend/src/features/party-discovery/ui/party-discovery.css', 'frontend/src/features/gift-discovery/ui/Gifts.tsx', 'frontend/src/features/gift-discovery/ui/gifts.css',
  'frontend/src/shared/ui/concert-window/index.tsx', 'frontend/src/shared/ui/concert-window/concert-window.css', 'frontend/src/app/CommunityFeatures.tsx', 'frontend/src/app/__tests__/fixtures/community.tsx', 'frontend/src/app/__tests__/fixtures/community-data.ts', 'frontend/src/app/router.tsx',
  'contracts/generated/mcr-domain.openapi.json', 'docs/mcr/design/ui-ux-reference-review.md', 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg',
  'frontend/src/features/party-discovery/__tests__/fixtures/preview.tsx', 'frontend/src/features/gift-discovery/__tests__/fixtures/preview.tsx',
  'frontend/src/features/event-feed/__tests__/visual-adoption/preview.tsx', 'frontend/src/features/event-feed/__tests__/visual-adoption/build.mjs', 'frontend/src/features/event-feed/__tests__/visual-adoption/browser.mjs']
for (const file of sources) report.sourceHashes[file] = createHash('sha256').update(await readFile(resolve(root, file))).digest('hex')
await bundle()
const server = await serve(), address = server.httpServer.address(), url = `http://127.0.0.1:${address.port}`
let browser
const standalone = '/src/features/event-feed/__tests__/visual-adoption/preview.html'
const selectors = { feed: '.feed-browse', party: '.party-browse', gift: '.gifts-browse' }
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 1, reducedMotion: 'reduce', timezoneId: 'Asia/Seoul', locale: 'en-GB' })
    const page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    const shot = async name => { const file = `${name}-${width}.png`; await page.screenshot({ path: resolve(evidence, file), fullPage: true }); report.screenshots.push(file) }
    for (const kind of ['feed', 'party', 'gift']) {
      for (const mode of ['loaded', 'empty', 'error', 'pending', 'readonly', 'unavailable']) {
        await page.goto(`${url}${standalone}?kind=${kind}&mode=${mode}`)
        await expect(page.locator(`${selectors[kind]} .concert-window__titlebar`)).toBeVisible()
        if (mode === 'loaded' || mode === 'readonly') {
          await expect(page.locator(kind === 'feed' ? '.feed-posts > li' : `${selectors[kind]} .concert-row`).first()).toBeVisible()
          const geometry = await page.locator(selectors[kind]).evaluate(element => {
            const first = element.querySelector('.feed-posts > li,.concert-row')?.getBoundingClientRect()
            const title = element.querySelector('.concert-window__titlebar')
            const controls = [...element.querySelectorAll('button,a,summary,input,select')].filter(el => el.checkVisibility())
            return { pageWidth: document.documentElement.scrollWidth, innerWidth, firstRowTop: first?.top, firstRowBottom: first?.bottom,
              titleBackground: getComputedStyle(title).backgroundColor,
              shortControls: controls.map(el => ({ label: el.textContent, width: el.getBoundingClientRect().width, height: el.getBoundingClientRect().height })).filter(r => r.width < 43.9 || r.height < 43.9) }
          })
          report.geometry.push({ width, kind, mode, ...geometry }); assert(geometry.pageWidth <= width, JSON.stringify(geometry)); assert(geometry.firstRowTop < (mode === 'readonly' ? 400 : 330)); assert.equal(geometry.shortControls.length, 0, JSON.stringify(geometry.shortControls))
          assert.equal(geometry.titleBackground, 'rgb(27, 25, 23)')
          if (mode === 'readonly' && kind === 'feed') await expect(page.getByRole('button', { name: 'Write a post' })).toHaveCount(0)
          if (mode === 'readonly' && kind === 'gift') await expect(page.getByText('Read-only event', { exact: true })).toBeVisible()
        } else if (mode === 'empty') await expect(page.getByText(kind === 'feed' ? 'No posts yet.' : kind === 'party' ? 'No matching parties' : 'No matching gifts', { exact: true })).toBeVisible()
        else if (mode === 'error') await expect(page.getByRole('alert').first()).toBeVisible()
        else if (mode === 'pending') await expect(page.getByRole('status').first()).toBeVisible()
        else await expect(page.getByRole('status').first()).toBeVisible()
        assert.equal(await page.locator(`${selectors[kind]} img, ${selectors[kind]} canvas`).count(), 0)
        await shot(`${kind}-${mode}`)
      }
      // Keyboard primary navigation and caller-owned routes in the real components.
      await page.goto(`${url}${standalone}?kind=${kind}`)
      if (kind === 'feed') {
        const all = page.getByRole('tab', { name: 'All', exact: true }); await all.focus(); await page.keyboard.press('ArrowRight')
        await expect(page.getByRole('tab', { name: 'Images' })).toBeFocused(); await expect(page.getByRole('tab', { name: 'Images' })).toHaveAttribute('aria-selected', 'true')
        await page.keyboard.press('End'); await expect(page.getByRole('tab', { name: 'Gifts' })).toBeFocused()
        await page.keyboard.press('Home'); await expect(all).toBeFocused(); await expect(page.locator('.feed-posts > li')).toHaveCount(3)
        await page.getByRole('button', { name: /Open thread/ }).first().focus(); await page.keyboard.press('Enter'); await expect(page.getByText('Fixture thread destination reached.')).toBeVisible()
        await page.goto(`${url}${standalone}?kind=feed`); await page.getByRole('button', { name: 'Write a post' }).click(); await expect(page.getByText('Fixture compose destination reached.')).toBeVisible()
      } else {
        const details = page.getByRole('link', { name: /View details for/ }).first(); await details.focus(); await page.keyboard.press('Enter')
        await expect(page.getByRole('heading', { name: kind === 'party' ? 'Fixture encore crew' : 'Fixture fan badges', exact: true })).toBeVisible()
        await expect(page.getByRole('button', { name: kind === 'party' ? 'Apply to party' : 'Reserve unavailable', exact: true })).toBeVisible()
        if (kind === 'gift') await expect(page.getByRole('button', { name: 'Reserve unavailable' })).toBeDisabled()
        await shot(`${kind}-detail`)
        await page.goto(`${url}${standalone}?kind=${kind}&mode=readonly`); await page.getByRole('link', { name: /View details for/ }).first().click()
        await expect(page.getByRole('button', { name: kind === 'party' ? 'Apply to party' : 'Reserve unavailable', exact: true })).toBeDisabled()
        await shot(`${kind}-readonly-detail`)
      }
      report.checks.push(`${width}/${kind}: loaded/empty/error/pending/read-only/unavailable; 44px controls; no overflow; keyboard primary callback/route and detail gate`)
    }
    for (const kind of ['party', 'gift']) {
      const code = kind === 'party' ? '__FE21__' : '__FE24__'
      await page.goto(`${url}/src/features/${kind === 'party' ? 'party-discovery' : 'gift-discovery'}/__tests__/fixtures/preview.html`)
      await expect(page.getByRole('link', { name: kind === 'party' ? 'Before the encore' : 'Handmade fan badges', exact: true })).toBeVisible()
      await page.getByRole('button', { name: 'Expand panel' }).click(); await shot(`${kind}-root-shell`)
      await page.getByRole('link', { name: /View details for/ }).first().click()
      await expect(page.getByRole('heading', { name: kind === 'party' ? 'Before the encore' : 'Handmade fan badges', exact: true })).toBeVisible()
      await shot(`${kind}-root-detail`)
      const state = await page.evaluate(code => window[code].state(), code)
      assert.equal(state.mounts, 1); assert.equal(kind === 'party' ? state.connections : state.resources, 1)
      if (kind === 'gift') {
        await page.getByRole('link', { name: 'Check my gifts →' }).click(); await expect(page.getByText('Only you can see this draft.')).toBeVisible(); await shot('gift-root-own')
      }
      await page.evaluate(code => window[code].dispose(), code)
      report.checks.push(`${width}/${kind}: existing createAppRoutes/FE07 shell, actual browse/detail; one instrumented world owner; ${kind === 'party' ? 'synthetic Phaser terrain, no real socket' : 'instrumented world lease and shared MyGifts draft consumer'}`)
    }
    // Default root composition, without panel overrides or an alternate provider.
    await page.goto(`${url}/src/app/__tests__/fixtures/community.html`)
    await expect(page.getByRole('link', { name: 'Fan badges', exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Expand panel' }).click(); await shot('gift-default-root')
    const baseline = await page.evaluate(() => window.__COMMUNITY__.state())
    await page.getByRole('link', { name: 'My gifts →', exact: true }).click()
    await expect(page.getByText('Only you can see this draft.')).toBeVisible(); await shot('gift-default-root-own')
    await page.getByRole('link', { name: 'Meet', exact: true }).click()
    await expect(page.getByRole('link', { name: 'Before the encore', exact: true })).toBeVisible(); await shot('party-default-root')
    await page.getByRole('link', { name: /View details for/ }).first().click()
    await page.getByRole('button', { name: 'Apply to party', exact: true }).click()
    await expect(page.getByText('The server is still processing this command.', { exact: true })).toBeVisible(); await shot('party-default-root-command-pending')
    await page.getByRole('link', { name: 'Gifts', exact: true }).click(); await expect(page.getByRole('link', { name: 'Fan badges', exact: true })).toBeVisible()
    await page.getByRole('link', { name: 'Meet', exact: true }).click()
    await expect(page.getByText('The server is still processing this command.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Check original command status', exact: true }).click()
    await expect(page.getByText('The command was confirmed.', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'MY PLAN', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Cancel application', exact: true })).toBeEnabled(); await shot('party-default-root-plan')
    const current = await page.evaluate(() => window.__COMMUNITY__.state())
    assert.equal(current.mounts, baseline.mounts); assert.equal(current.peakGames, 1); assert.equal(current.peakConnections, 1)
    await page.evaluate(() => window.__COMMUNITY__.dispose())
    const final = await page.evaluate(() => window.__COMMUNITY__.state())
    assert.equal(final.games, 0); assert.equal(final.connections, 0); assert.equal(final.mounts, final.disposals)
    report.checks.push(`${width}/default root: unmodified CommunityFeatures/createAppRoutes, real transport over fixture fetch; browse/detail/MY GIFTS/MY PLAN; pending application retained across tabs and reconciled; one world lease; disposal complete`)
    await context.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} finally {
  await browser?.close(); await new Promise(done => server.httpServer.close(done))
  await writeFile(resolve(evidence, 'report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ result: report.result, screenshots: report.screenshots.length, checks: report.checks, errors: report.errors }))
}
const reference = relative(evidence, resolve(root, 'docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg'))
await writeFile(resolve(evidence, 'comparison.html'), `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>UIV03 human reference comparison</title><style>body{font:16px/1.5 Arial;background:#eee5d3;color:#1d1a17;margin:20px}section{display:flex;align-items:start;gap:16px;flex-wrap:wrap}img{max-width:100%;height:auto}figure{margin:0;width:390px}h1{font-size:24px}</style><h1>UIV03 reference comparison — synthetic actual-component evidence</h1><p>Reference is the original full ten-screen image, unchanged. Compare FEED, MEET and GIVEAWAY in its lower row with the actual 390px components. Black window chrome, ivory frame, red actions, condensed headings and dense public information are adopted. No reference art was extracted. Missing qualified avatar/gift media is deliberately absent. Details precedes approval/reservation; visual adoption does not certify live behavior.</p><img width="1100" src="${reference}" alt="Original user-selected reference, unchanged"><section>${['feed','party','gift'].map(kind => `<figure><h2>${kind.toUpperCase()}</h2><img src="${kind}-loaded-390.png" alt="${kind} actual component, synthetic fixture at 390 pixels"></figure>`).join('')}</section></html>`)
