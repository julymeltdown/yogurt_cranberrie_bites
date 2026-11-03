import assert from 'node:assert/strict'
import { build, preview } from 'vite'
import { chromium, expect } from '@playwright/test'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend = resolve(import.meta.dirname, '../../../..'), fixturePath = '/src/app/places-entry/__tests__/preview.html'
const outDir = '/tmp/mcr-places-entry-build', evidence = resolve(import.meta.dirname, 'evidence')
await build({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner', cacheDir: '/tmp/mcr-places-entry-cache', logLevel: 'warn',
  build: { outDir, emptyOutDir: true, rollupOptions: { input: [resolve(frontend, 'index.html'), resolve(frontend, `.${fixturePath}`)] } } })
const server = await preview({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), configLoader: 'runner', build: { outDir },
  plugins: [{ name: 'places-entry-fixture-document-only', configurePreviewServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (new URL(request.url, 'http://127.0.0.1').pathname !== '/world/map') return next()
      response.setHeader('Content-Type', 'text/html'); response.end(await readFile(resolve(outDir, `.${fixturePath}`), 'utf8'))
    })
  } }], preview: { host: '127.0.0.1', port: 0, strictPort: false } })
const report = { result: 'FAIL', scope: 'Synthetic canonical Auth/HTTP + actual routes, FE14 map loader/Phaser and FE15 attachment; no default root/live backend/venue/device qualification', browser: '', widths: [], errors: [], screenshots: [], cleanup: false }
let browser, stage = 'start'
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  const base = `http://127.0.0.1:${server.httpServer.address().port}`
  for (const [width, height] of [[360,800],[390,844],[430,932]]) {
    stage = `${width}/start`
    const context = await browser.newContext({ viewport: { width,height }, hasTouch: true, reducedMotion: 'reduce', locale: 'en-GB', timezoneId: 'Asia/Seoul' })
    const page = await context.newPage(); page.on('pageerror', e => report.errors.push(e.message))
    const state = () => page.evaluate(() => window.__PLACES_ENTRY__.state())
    const ready = async () => { await expect.poll(async () => (await state()).runtime?.phase).toBe('ready'); await expect.poll(async () => (await state()).places?.list.phase).toBe('ready') }
    const mode = value => page.evaluate(value => window.__PLACES_ENTRY__.mode(value), value)
    const shot = async name => {
      await page.locator('.shell-panel-body').evaluate(el => { el.scrollTop = 0 })
      const file = `${name}-${width}.png`; await page.screenshot({ path: resolve(evidence,file), fullPage: true }); report.screenshots.push(file)
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
      const short = await page.locator('.places-view').evaluateAll(elements => elements.flatMap(element => [...element.querySelectorAll('button,a,input,summary')].filter(el => el.checkVisibility()).map(el => ({ label: el.textContent, width: el.getBoundingClientRect().width, height: el.getBoundingClientRect().height })).filter(r => r.width < 43.9 || r.height < 43.9)))
      assert.deepEqual(short,[])
    }
    await page.goto(`${base}${fixturePath}`); await ready(); await page.getByRole('button',{name:'Expand panel'}).click()
    const first = await state(); assert.equal(first.mounts,1); assert.equal(first.ownerCount,1); assert.equal(first.canvases,1)
    await shot('loaded')
    stage = `${width}/camera`
    const foot = first.foot
    await page.locator('.places-view').getByRole('button',{name:'Zoom in',exact:true}).click()
    await expect.poll(async () => (await state()).camera?.camera?.zoom).toBe(2)
    await page.getByRole('button',{name:'Show Synthetic reviewed landmark on virtual map'}).click()
    await expect.poll(async () => (await state()).camera?.following).toBe(false)
    await page.locator('.places-view').getByRole('button',{name:'Recenter avatar',exact:true}).click()
    await expect.poll(async () => (await state()).camera?.following).toBe(true)
    assert.deepEqual((await state()).foot,foot)
    stage = `${width}/navigation`
    await page.getByRole('link',{name:'View Synthetic reviewed landmark'}).click()
    await expect(page.getByRole('heading',{name:'Synthetic reviewed landmark',exact:true})).toBeVisible()
    await shot('detail')
    await page.goBack(); await expect(page.getByRole('heading',{name:'MAP',exact:true})).toBeVisible()
    assert.equal((await state()).ownerCount,1); assert.equal((await state()).mounts,1); assert.equal((await state()).canvases,1)
    stage = `${width}/mismatch`
    await mode('map-error'); await expect(page.getByRole('heading',{name:'Your map and places do not match'})).toBeVisible()
    await mode('ok'); await expect.poll(async () => (await state()).places?.list.phase).toBe('ready')
    assert.equal((await state()).places.mapMismatch,true)
    await shot('mismatch')
    stage = `${width}/reload`
    const previousDocument = (await state()).documentId
    await Promise.all([page.waitForEvent('load'),page.getByRole('button',{name:'Reload event',exact:true}).click()])
    await page.waitForFunction(() => Boolean(window.__PLACES_ENTRY__)); await ready()
    assert.notEqual((await state()).documentId,previousDocument); assert.equal((await state()).places.mapMismatch,false)
    assert.equal((await state()).ownerCount,1); assert.equal((await state()).canvases,1)
    await page.getByRole('button',{name:'Expand panel'}).click(); await shot('recovered')
    stage = `${width}/unavailable`
    await mode('unavailable'); await expect(page.getByText(/Places are unavailable until/)).toBeVisible()
    assert.equal((await state()).places,null); assert.equal((await state()).canvases,1)
    await shot('unavailable')
    await page.evaluate(() => window.__PLACES_ENTRY__.dispose())
    const end = await state(); assert.equal(end.canvases,0); assert.equal(end.controls,0); assert.equal(end.disposals,1)
    stage = `${width}/direct-detail`
    await page.goto(`${base}${fixturePath}?detail=1`); await ready()
    await expect(page.getByRole('heading',{name:'Synthetic reviewed landmark',exact:true})).toBeVisible()
    await page.getByRole('button',{name:'Expand panel'}).click(); await shot('direct-detail')
    await page.getByRole('button',{name:'Back to previous panel'}).click()
    await expect(page.getByRole('navigation',{name:'World tools'})).toBeVisible()
    assert.equal(new URL(page.url()).pathname,'/world')
    assert.equal((await state()).mounts,1); assert.equal((await state()).ownerCount,1); assert.equal((await state()).canvases,1)
    await page.evaluate(() => window.__PLACES_ENTRY__.dispose())
    assert.equal((await state()).canvases,0); assert.equal((await state()).controls,0)
    report.widths.push({ width,height, result:'PASS', directDetailAndHeaderBack:true, minFeatureTouchTarget:44, ownerCountBeforeNavigation:1, sameOwnerAfterNavigation:true, mapReloadRecovered:true, canvasDisposed:true })
    await context.close()
  }
  assert.deepEqual(report.errors,[]); report.result='PASS'; report.cleanup=true
} catch (error) { report.errors.push(`${stage}: ${error.stack ?? error}`); throw error }
finally { await browser?.close(); await server.close(); await writeFile(resolve(evidence,'native.json'),JSON.stringify(report,null,2)+'\n') }
console.log(JSON.stringify(report))
