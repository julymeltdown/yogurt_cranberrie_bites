// Bounded focused production-component captures, not the king's all-ten-route gate.
import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { mkdir, writeFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
const root = resolve(import.meta.dirname, '../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const disk = await statfs(root)
assert(disk.bavail * disk.bsize > 8 * 1024 ** 3, 'At least 8GiB free disk required before browser validation')
await mkdir(evidence, { recursive: true })
const server = await createServer({ root, configLoader: 'runner', cacheDir: '/tmp/mcr-ref-public-vite', server: { host: '127.0.0.1', port: 0, strictPort: false }, logLevel: 'warn' })
await server.listen()
let browser, page
const report = { result: 'FAIL', qualification: 'Actual Landing/PublicFrame and createAppRoutes, explicit synthetic MemorySession/document fixtures; actual production unavailable route also checked. No live auth/world/physical-device or all-ten-screen acceptance.', cases: [], screenshots: [], errors: [] }
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
    const geometry = await page.evaluate(() => {
      const rect = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
      return { poster: rect('.public-poster'), wordmark: rect('.landing-wordmark'), scene: rect('.landing-scene'), crowd: rect('.landing-crowd'), copy: rect('.landing-poster-copy'), cta: rect('.public-primary'), overflow: document.documentElement.scrollWidth > innerWidth }
    })
    assert.equal(geometry.overflow, false)
    assert(geometry.cta.height >= 44)
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
  if(process.env.MCR_REF_REQUIRE_ART) {
    page=await browser.newPage({viewport:{width:760,height:820}})
    await page.goto(`${base}/src/features/public/__tests__/reference-target/comparison.html`)
    await page.locator('img').evaluateAll(async images=>{await Promise.all(images.map(image=>image.decode()))})
    const file='comparison-390.png';await page.screenshot({path:resolve(evidence,file),fullPage:true});report.screenshots.push(file);await page.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { error: String(error), text: await page?.locator('body').innerText() }; throw error }
finally { await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n'); await browser?.close(); await server.close() }
