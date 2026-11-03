import assert from 'node:assert/strict'
import { build, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
import { writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend = resolve(import.meta.dirname, '../../../..'), fixture = '/src/app/shell-chrome/__tests__/preview.html'
const evidence = resolve(import.meta.dirname, 'evidence'), outDir = '/tmp/mcr-shell-chrome-build'
await build({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner', cacheDir: '/tmp/mcr-shell-chrome-cache', logLevel: 'warn',
  build: { outDir, emptyOutDir: true, rollupOptions: { input: [resolve(frontend, 'index.html'), resolve(frontend, `.${fixture}`)] } } })
const server = await preview({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner', build: { outDir }, preview: { host: '127.0.0.1', port: 0, strictPort: false } })
const report = { result: 'FAIL', scope: 'Module composition only; default WorldShell unchanged. Actual Phaser/ThemeController/FE17 with synthetic qualified map resources and MockWorldTransport; no approved avatar art/live service.', browser: '', widths: [], screenshots: [], errors: [] }
let browser, stage = 'start'
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  const base = `http://127.0.0.1:${server.httpServer.address().port}`
  for (const [width, height] of [[360,800], [390,844], [430,932]]) {
    const context = await browser.newContext({ viewport: { width,height }, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    const page = await context.newPage(); page.on('pageerror', error => report.errors.push(error.message))
    const state = () => page.evaluate(() => window.__CHROME__.state())
    const mode = value => page.evaluate(value => window.__CHROME__.mode(value), value)
    const selected = async value => { await expect.poll(async () => (await state()).theme.phase).toBe('ready'); await expect.poll(async () => (await state()).skin).toBe(value) }
    const open = async query => {
      await page.goto(`${base}${fixture}${query ?? ''}`)
      await expect.poll(async () => (await state()).phase).toBe('ready'); await expect.poll(async () => (await state()).published).toBe(true)
      await expect.poll(async () => (await state()).activeSockets).toBe(1); await page.evaluate(() => window.__CHROME__.admit())
      await expect.poll(async () => (await state()).room).toBeTruthy()
    }
    const measure = () => page.evaluate(() => {
      const height = selector => document.querySelector(selector).getBoundingClientRect().height
      const h = height('.mcr-chrome-header'), era = height('.mcr-chrome-eras'), footer = height('.mcr-chrome-bottom'), world = height('.chrome-fixture-stage')
      return { viewport: innerHeight, header: h, era, footer, chrome: h + era + footer, world, worldFraction: world / innerHeight, qualification: height('.chrome-fixture-qualification'), overflow: document.documentElement.scrollWidth > innerWidth,
        labels: [...document.querySelectorAll('.mcr-chrome-bottom span')].map(el => ({ label: el.textContent, width: el.getBoundingClientRect().width, parent: el.parentElement.getBoundingClientRect().width })),
        reducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches }
    })
    const colors = () => page.locator('canvas').evaluate(async canvas => {
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      const pixels = canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data, colors = new Set()
      for (let i=0;i<pixels.length;i+=4) colors.add(`${pixels[i]},${pixels[i+1]},${pixels[i+2]}`)
      return [...colors]
    })
    const shot = async name => {
      await colors()
      const file = `${name}-${width}.png`; await page.screenshot({ path: resolve(evidence,file), fullPage: true }); report.screenshots.push(file)
      const layout = await measure(); assert.equal(layout.overflow, false)
      for (const l of layout.labels) assert(l.width <= l.parent)
      const small = await page.locator('.mcr-chrome-header,.mcr-chrome-eras,.mcr-chrome-bottom,.mcr-chrome-panel-header').evaluateAll(els => els.flatMap(el => [...el.querySelectorAll('button,a')]).filter(el => el.checkVisibility()).map(el => ({ text: el.textContent, w: el.getBoundingClientRect().width, h: el.getBoundingClientRect().height })).filter(r => r.w < 43.9 || r.h < 43.9))
      assert.deepEqual(small, [])
    }
    const stable = async before => {
      const current = await state(); assert.equal(current.documentId, before.documentId); assert.equal(current.mounts,1); assert.equal(current.games,1); assert.equal(current.peakGames,1)
      assert.equal(current.sockets,1); assert.equal(current.activeSockets,1); assert.equal(current.sameCamera,true); assert.equal(current.canvases,1); assert.equal(current.themeSubscriptions,1)
      assert.equal(current.room,before.room); assert.deepEqual(current.outfit,before.outfit)
      return current
    }
    stage = `${width}/ready`; await open(); const initial = await state(); await shot('world-revenge')
    const initialColors = await colors()
    const readyLayout = await measure(); assert(readyLayout.worldFraction >= .70); assert(readyLayout.chrome <= 174); assert.equal(readyLayout.reducedMotion,true)
    stage = `${width}/pending`; await mode('pending')
    await page.getByRole('button', { name: 'Parade', exact: true }).focus(); await page.keyboard.press('Enter')
    await expect.poll(async () => (await state()).theme.phase).toBe('loading'); await expect.poll(async () => (await state()).gates).toBe(1)
    assert.equal((await state()).skin,'REVENGE'); assert.equal((await state()).writes,0)
    await expect(page.getByRole('button', { name: 'Revenge', exact: true })).toHaveAttribute('aria-pressed','true')
    await expect(page.getByRole('button', { name: 'Parade, loading', exact: true })).toHaveAttribute('aria-pressed','false'); await shot('theme-pending')
    await page.evaluate(() => window.__CHROME__.release()); await selected('PARADE'); await shot('world-parade')
    const paradeColors = await colors(); assert.notDeepEqual(paradeColors,initialColors); assert(paradeColors.includes('89,92,103'))
    const afterTheme = await stable(initial); assert.deepEqual(afterTheme.camera,initial.camera); assert.equal(afterTheme.writes,1)
    stage = `${width}/failure`; await mode('fail'); await page.getByRole('button', { name: 'Danger', exact: true }).tap()
    await expect.poll(async () => (await state()).theme.phase).toBe('error'); assert.equal((await state()).skin,'PARADE')
    assert.deepEqual(await colors(),paradeColors)
    await expect(page.getByRole('status')).toContainText('could not be loaded or saved'); await shot('theme-error')
    await mode('valid'); await page.getByRole('button', { name: 'Danger', exact: true }).tap(); await selected('DANGER'); await shot('world-danger'); await stable(initial)
    assert((await colors()).includes('36,121,135'))
    stage = `${width}/navigation`
    for (const [label, path] of [['Feed','/feed'],['Meet','/meet'],['Gifts','/gifts'],['Wall','/wall'],['Me','/me']]) {
      await page.getByRole('navigation', { name:'Main navigation' }).getByRole('link', { name:label, exact:true }).click()
      await expect(page.getByRole('heading',{name:label,exact:true})).toBeVisible(); assert.equal(new URL(page.url()).pathname,path)
      await expect(page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:label,exact:true})).toHaveAttribute('aria-current','page'); await stable(initial)
    }
    await shot('me-panel')
    await page.getByRole('button', { name:'Expand panel' }).click(); await shot('panel-expanded')
    await page.getByRole('button', { name:'Reduce panel' }).click()
    await page.getByRole('button', { name:'Minimize panel' }).click(); await expect(page.getByRole('button', { name:'Show panel content' })).toBeVisible()
    await page.getByRole('button', { name:'Show panel content' }).click(); await expect(page.getByRole('button', { name:'Expand panel' })).toBeVisible()
    await page.getByRole('link', { name:'Settings', exact:true }).click(); await expect(page.getByRole('heading', { name:'Settings',exact:true })).toBeVisible(); await stable(initial)
    await page.getByRole('navigation', { name:'Main navigation' }).getByRole('link', { name:'World',exact:true }).click()
    await expect(page.locator('.chrome-fixture-panel')).toHaveCount(0)
    await page.getByRole('link', { name:'Inbox',exact:true }).click(); await expect(page.getByRole('heading', { name:'Inbox',exact:true })).toBeVisible()
    await page.getByRole('button', { name:'Back to previous panel' }).click(); await expect(page.locator('.chrome-fixture-panel')).toHaveCount(0)
    await expect(page.getByRole('link', { name:'Inbox',exact:true })).toBeFocused()
    await page.getByRole('navigation', { name:'Main navigation' }).getByRole('link', { name:'Gifts',exact:true }).click()
    await expect(page.getByRole('heading',{name:'Gifts',exact:true})).toBeVisible()
    await page.getByRole('link',{name:'Open fixture giveaway'}).click(); await expect(page.getByRole('heading',{name:'Giveaway',exact:true})).toBeVisible()
    await page.getByRole('button',{name:'Back to previous panel'}).click(); await expect(page.getByRole('heading',{name:'Gifts',exact:true})).toBeVisible()
    await expect(page.getByRole('link',{name:'Open fixture giveaway'})).toBeFocused(); await stable(initial)
    stage = `${width}/short-viewport`; await page.setViewportSize({ width,height:430 }); await page.getByRole('textbox',{name:'Fixture draft'}).focus()
    await shot('short-viewport'); const shortLayout = await measure()
    await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport(); await expect(page.getByRole('textbox',{name:'Fixture draft'})).toBeInViewport()
    await page.setViewportSize({width,height}); await page.getByRole('navigation',{name:'Main navigation'}).getByRole('link',{name:'World',exact:true}).click()
    await expect(page.locator('.chrome-fixture-panel')).toHaveCount(0)
    stage = `${width}/withdrawal`; await mode('pending'); await page.getByRole('button',{name:'Revenge',exact:true}).click()
    await expect.poll(async () => (await state()).gates).toBe(1); const writes = (await state()).writes
    await page.evaluate(() => window.__CHROME__.withdraw()); await expect(page.getByRole('status')).toHaveText('Eras unavailable.')
    await expect(page.locator('.mcr-chrome-eras [aria-pressed="true"]')).toHaveCount(0); assert.equal((await state()).themeSubscriptions,0)
    await page.evaluate(() => window.__CHROME__.release()); await shot('theme-unavailable')
    assert.equal((await state()).writes,writes); assert.equal((await state()).theme.phase,'disposed')
    await page.evaluate(() => window.__CHROME__.dispose()); assert.equal((await state()).games,0); assert.equal((await state()).canvases,0); assert.equal((await state()).activeSockets,0)
    stage = `${width}/deep-link`; await open('?start=detail'); await expect(page.getByRole('heading',{name:'Giveaway',exact:true})).toBeVisible(); await shot('direct-detail')
    await page.getByRole('button',{name:'Back to previous panel'}).click(); await expect(page.getByRole('heading',{name:'Gifts',exact:true})).toBeVisible(); assert.equal(new URL(page.url()).pathname,'/gifts')
    assert.equal((await state()).mounts,1); await page.evaluate(() => window.__CHROME__.dispose())
    report.widths.push({width,height,result:'PASS',readyLayout,shortLayout,oneActualPhaser:true,oneMockSocket:true,sameCameraRoomOutfit:true,noInternalDocumentReload:true,backFocus:true,explicitThemeCommitAndRollback:true,renderedPaletteVerified:true,touchThemeSelection:true,withdrawalFenced:true,cleanup:true})
    await context.close()
  }
  assert.deepEqual(report.errors,[]); report.result='PASS'
} catch (error) { report.errors.push(`${stage}: ${error.stack ?? error}`); throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n') }
console.log(JSON.stringify(report))
