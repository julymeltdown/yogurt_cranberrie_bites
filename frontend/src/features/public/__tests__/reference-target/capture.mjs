// Bounded focused production-component captures, not the king's all-ten-route gate.
import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile, statfs, readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const root = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, process.env.MCR_PUBLIC_EVIDENCE_DIR ?? 'evidence/artwork')
const disk = await statfs(root)
assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, 'At least 8GiB free disk required before browser validation')
await mkdir(evidence, { recursive: true })
const server = await createServer({ root, configLoader: 'runner', cacheDir: '/tmp/mcr-public-fidelity-vite', server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
let browser, page, productionServer
const report = { result: 'FAIL', qualification: 'Actual Landing/PublicFrame and createAppRoutes, explicit synthetic MemorySession/document fixtures; actual production unavailable route also checked. Actual main.tsx anonymous entry also checked with explicit event/HTTP fixtures. No live auth/world/physical-device or all-ten-screen acceptance.', cases: [], screenshots: [], errors: [] }
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const base = `http://127.0.0.1:${server.httpServer.address().port}`
  for (const [width, height] of (process.env.MCR_REF_PREVIEW_ONLY ? [[390,844]] : [[360,800],[390,844],[430,932]])) {
    page = await browser.newPage({ viewport: { width, height }, reducedMotion: 'reduce' })
    const assets = []; let remountRequests = null
    page.on('request', request => { const url = new URL(request.url()); if(url.pathname.startsWith('/world/avatars/derived-v1/')) assets.push(url.pathname) })
    page.on('pageerror', e => report.errors.push(e.message))
    const fixture = `${base}/src/features/public/__tests__/reference-target/preview.html`
    await page.goto(fixture)
    const entry = page.getByRole('link', { name: 'Sign in to enter', exact: true })
    await expect(entry).toHaveAttribute('href', '/login?returnTo=%2Fworld')
    await expect(entry).toBeInViewport()
    if(process.env.MCR_REF_REQUIRE_ART) {
      await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state','ready')
      await expect(page.locator('.landing-crowd > span')).toHaveCount(4)
      await page.evaluate(async () => { const url=getComputedStyle(document.querySelector('.landing-crowd > span')).backgroundImage.slice(5,-2);const image=new Image();image.src=url;await image.decode();if(image.naturalWidth!==384||image.naturalHeight!==64)throw new Error('Wrong selected static atlas') })
    }
    const file = `landing-${width}.png`; await page.screenshot({ path: resolve(evidence, file) }); report.screenshots.push(file)
    const clip = await page.evaluate(() => { const head=document.querySelector('.public-header').getBoundingClientRect(), poster=document.querySelector('.public-poster').getBoundingClientRect();return {x:head.x,y:head.y,width:head.width,height:poster.bottom-head.y} })
    const posterFile=`poster-${width}.png`;await page.screenshot({path:resolve(evidence,posterFile),clip});report.screenshots.push(posterFile)
    if (width === 390 && process.env.MCR_REF_REQUIRE_ART) {
      // Capture the actual routed foreground at its native CSS geometry, hiding
      // only unrelated illustration/control layers for contour measurement.
      const isolate = await page.addStyleTag({ content: '.public-poster > :not(.landing-web):not(.landing-wordmark):not(.landing-scene) { visibility:hidden!important } .public-poster { background:#161413!important } .landing-print-wear { visibility:hidden!important }' })
      const file = 'foreground-390.png'
      await page.locator('.landing-wordmark').screenshot({ path: resolve(evidence, file) }); report.screenshots.push(file)
      await isolate.evaluate(style => style.remove())
    }
    const geometry = await page.evaluate(() => {
      const rect = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
      return { figureWidths: [...document.querySelectorAll('.landing-crowd > span')].map(el => el.getBoundingClientRect().width), poster: rect('.public-poster'), wordmark: rect('.landing-wordmark'), scene: rect('.landing-scene'), crowd: rect('.landing-crowd'), copy: rect('.landing-poster-copy'), cta: rect('.public-primary'), overflow: document.documentElement.scrollWidth > innerWidth }
    })
    assert.equal(geometry.overflow, false)
    assert(geometry.cta.height >= 44)
    if (process.env.MCR_REF_REQUIRE_ART) assert(geometry.figureWidths.every(width => width / geometry.crowd.width > .34), 'Approved gallery must retain landing figure scale regardless of stylesheet import order')
    assert.equal(await page.evaluate(() => window.__REFERENCE_PUBLIC__.state().mounts), 0)
    if (!process.env.MCR_REF_PREVIEW_ONLY) {
      await page.keyboard.press('Tab'); await expect(page.getByRole('link', { name: 'Skip to content' })).toBeFocused(); await page.keyboard.press('Enter'); await expect(page.locator('#public-main')).toBeFocused()
      for(let step=0;step<8 && !await entry.evaluate(el=>el===document.activeElement);step++) await page.keyboard.press('Tab')
      await expect(entry).toBeFocused(); assert.notEqual(await entry.evaluate(el => getComputedStyle(el).outlineStyle), 'none')
      await expect(page.getByText('A place for the crowd.',{exact:true})).not.toBeVisible()
      await page.getByText('About this space',{exact:true}).click();await expect(page.getByText('A place for the crowd.',{exact:true})).toBeVisible();await page.getByText('About this space',{exact:true}).click()
      await entry.click(); await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible(); assert.equal((await page.evaluate(() => window.__REFERENCE_PUBLIC__.state())).path, '/login')
      await page.evaluate(() => window.__REFERENCE_PUBLIC__.back()); await expect(entry).toBeVisible()
      const nav = page.getByRole('navigation', { name: 'Public information' })
      await nav.getByRole('link', { name: 'Terms', exact: true }).click(); await expect(page.getByRole('heading', { name: 'Terms', exact: true })).toBeFocused(); assert.equal(await page.locator('.public-entry--poster').count(),0)
      await page.getByRole('link', { name: 'Back to home' }).click(); await expect(entry).toBeVisible()
      if(process.env.MCR_REF_REQUIRE_ART) { await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state','ready');assert.equal(assets.filter(path=>path.endsWith('.json')).length,1);assert.equal(assets.filter(path=>path.endsWith('.png')).length,1); remountRequests = [...assets] }
      await page.setViewportSize({ width, height: 420 }); await entry.scrollIntoViewIfNeeded(); await expect(entry).toBeInViewport(); await page.setViewportSize({ width, height })
      await page.goto(`${fixture}?entry=error`); await page.getByRole('button', { name: 'Check entry again' }).click(); await expect(page.getByText('Checking your session…', { exact: true })).toBeVisible(); assert.equal(await page.evaluate(() => window.__REFERENCE_PUBLIC__.state().retries),1)
      await page.evaluate(() => window.__REFERENCE_PUBLIC__.readiness('anonymous')); await expect(page.getByRole('link', { name: 'Sign in to enter' })).toBeVisible()
      await page.evaluate(() => window.__REFERENCE_PUBLIC__.ready()); await expect(page.locator('[data-world-host]')).toBeVisible(); assert.equal(await page.evaluate(() => window.__REFERENCE_PUBLIC__.state().action),'REPLACE')
      await page.evaluate(() => window.__REFERENCE_PUBLIC__.revoke()); await expect(page.locator('[data-world-host]')).toHaveCount(0)
      await page.goto(base); await expect(page.getByText('World entry is not open yet.', { exact: true })).toBeVisible(); await expect(page.getByRole('button', { name: 'Enter the world', exact: true })).toBeDisabled()
      if(process.env.MCR_REF_REQUIRE_ART) { await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state','ready'); await expect(page.locator('.landing-crowd > span')).toHaveCount(4) }
      const production = `production-unavailable-${width}.png`; await page.screenshot({path:resolve(evidence,production)});report.screenshots.push(production)
    }
    report.cases.push({ width, height, geometry, approvedArtworkRequired:!!process.env.MCR_REF_REQUIRE_ART, sameDocumentRouteRemountRequests:remountRequests, publicAssetRequestsIncludingFullReloads:assets }); await page.close()
  }
  if(process.env.MCR_REF_REQUIRE_ART && !process.env.MCR_REF_PREVIEW_ONLY) {
    page=await browser.newPage({viewport:{width:1010,height:820}})
    await page.goto(`${base}/src/features/public/__tests__/reference-target/artwork-comparison.html`)
    await page.locator('img').evaluateAll(async images=>{await Promise.all(images.map(image=>image.decode()))})
    const file='comparison-390.png';await page.screenshot({path:resolve(evidence,file),fullPage:true});report.screenshots.push(file);await page.close()
  }
  if (!process.env.MCR_REF_PREVIEW_ONLY) {
    // Actual index.html -> main.tsx -> createAppRouter -> Landing, with only an
    // explicit event build input and a canonical disabled-capabilities HTTP fixture.
    // No production component or session implementation is replaced.
    await server.close()
    productionServer = await createServer({ root, configLoader: 'runner', cacheDir: '/tmp/mcr-public-fidelity-entry-vite', define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify('11111111-1111-4111-8111-111111111111') }, server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
    await productionServer.listen()
    const productionBase = `http://127.0.0.1:${productionServer.httpServer.address().port}`
    const capabilities = await readFile(resolve(root, '../contracts/fixtures/core/auth-capabilities-disabled.json'), 'utf8')
    report.actualEntry = []
    for (const width of [360, 390, 430]) {
      page = await browser.newPage({ viewport: { width, height: 932 }, reducedMotion: 'reduce' })
      const apiRequests = [], modules = [], assets = []
      page.on('pageerror', error => report.errors.push(error.message))
      page.on('request', request => { const path = new URL(request.url()).pathname; modules.push(path); if (path.startsWith('/world/avatars/derived-v1/')) assets.push(path) })
      await page.route('**/api/**', route => {
        const path = new URL(route.request().url()).pathname
        if (!path.startsWith('/api/')) return route.continue()
        apiRequests.push(path)
        return route.fulfill({ status: path === '/api/auth/capabilities' ? 200 : 503, contentType: 'application/json', body: path === '/api/auth/capabilities' ? capabilities : '{}' })
      })
      await page.goto(productionBase)
      const entry = page.getByRole('link', { name: 'Sign in to enter', exact: true })
      await expect(entry).toBeVisible({ timeout: 20000 }); await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state', 'ready')
      await expect(page.locator('.landing-crowd > span')).toHaveCount(4)
      await expect(page.locator('canvas, [data-world-host]')).toHaveCount(0)
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
      assert.equal(modules.some(path => /\/phaser(?:\/|\.)/.test(path)), false)
      const file = `actual-entry-anonymous-${width}.png`; await page.screenshot({ path: resolve(evidence, file) }); report.screenshots.push(file)
      await entry.click(); await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible()
      assert.equal(new URL(page.url()).pathname, '/login'); assert.equal(new URL(page.url()).search, '?returnTo=%2Fworld')
      await page.goBack(); await expect(entry).toBeVisible()
      await page.getByRole('navigation', { name: 'Public information' }).getByRole('link', { name: 'Safety', exact: true }).click()
      await expect(page.getByRole('heading', { name: 'Safety', exact: true })).toBeFocused()
      await expect(page.getByText('This document is not available yet.', { exact: true })).toBeVisible()
      assert.equal(await page.locator('.public-entry--poster').count(), 0)
      await page.getByRole('link', { name: 'Back to home', exact: true }).click(); await expect(entry).toBeVisible()
      await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state', 'ready')
      assert.equal(assets.filter(path => path.endsWith('.json')).length, 1)
      assert.equal(assets.filter(path => path.endsWith('.png')).length, 1)
      assert(apiRequests.every(path => path === '/api/auth/capabilities'))
      // Missing approved art stays absent; entry permission never depends on ornaments.
      if (width === 390) {
        await page.route('**/world/avatars/derived-v1/**', route => route.fulfill({ status: 503, body: '' }))
        await page.reload(); await expect(entry).toBeVisible()
        await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state', 'unavailable')
        await expect(page.locator('.landing-crowd > span')).toHaveCount(0)
        await expect(page.locator('canvas, [data-world-host]')).toHaveCount(0)
        await entry.click(); await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible()
      }
      report.actualEntry.push({ width, apiRequests, sameDocumentArtworkRequests: assets.slice(0, 2), missingArtworkCase: width === 390, result: 'PASS', limitation: 'Actual public entry with a configured synthetic event and intercepted disabled-capabilities response; no live provider.' })
      await page.close()
    }
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { error: String(error), text: await page?.locator('body').innerText(), html: await page?.content(), url: page?.url() }; throw error }
finally { await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n'); await browser?.close(); await productionServer?.close(); await server.close() }
