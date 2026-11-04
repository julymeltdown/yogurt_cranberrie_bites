// Developer-owned CSS evidence. Existing production-root fixture; no product DOM replacement.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { mkdir, readFile, writeFile, rm, statfs, readdir, stat } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
const require = createRequire('/Users/lhs/Desktop/mcr/frontend/package.json')
const { createServer } = await import(require.resolve('vite'))
const { chromium, expect } = require('@playwright/test')
const { default: react } = await import(require.resolve('@vitejs/plugin-react'))
const { default: tailwindcss } = await import(require.resolve('@tailwindcss/vite'))
const root = resolve(import.meta.dirname, '../../../..')
const tag = process.argv[2]
assert(/^[a-z0-9-]+$/.test(tag ?? ''), 'Supply a new evidence round name')
const output = resolve(import.meta.dirname, tag)
const temporary = process.env.TMPDIR
assert(temporary?.startsWith('/private/tmp/mcr-css-meet-'), 'Owned temporary directory required')
await mkdir(output)
const size = async path => { let bytes=0;for(const e of await readdir(path,{withFileTypes:true}).catch(()=>[])){const p=resolve(path,e.name);bytes+=e.isDirectory()?await size(p):e.isFile()?(await stat(p)).size:0}return bytes }
const resource={minFree:Infinity,peaks:{temporary:0,durable:0,whole:0},caps:{temporary:128*1024**2,durable:32*1024**2,whole:160*1024**2}}
const floor = async () => { const s = await statfs(root),free=s.bavail*s.bsize;resource.minFree=Math.min(resource.minFree,free);assert(free>=8*1024**3,'8GiB disk floor');const temporaryBytes=await size(temporary),durable=await size(import.meta.dirname);for(const [key,value]of Object.entries({temporary:temporaryBytes,durable,whole:temporaryBytes+durable})){resource.peaks[key]=Math.max(resource.peaks[key],value);assert(value<=resource.caps[key],`${key} byte cap`)} }
await floor()
const cssPath = 'frontend/src/features/party-discovery/ui/party-discovery.css'
const baseline = process.env.MCR_MEET_BASELINE === '1'
const css = baseline ? execFileSync('git',['show',`91058462:${cssPath}`],{cwd:root}) : await readFile(resolve(root, cssPath))
const report = { kind: 'CSS-PARITY-MEET', fixture: 'src/app/__tests__/cozy-backgrounds-v1/preview.html', qualification: 'MOCK_READY_ONLY',
  candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  source: cssPath, baselineRootInput: baseline ? '91058462' : null, cssSha256: createHash('sha256').update(css).digest('hex'), node: process.version,
  reference: { photo: 'AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg', crop: [260,450,501,848] },
  liveServices: 'NOT_RUN', devices: 'NOT_RUN', fullVisualAcceptance: 'OPEN', resource, cases: [] }
const server = await createServer({ root: resolve(root, 'frontend'), configFile: false, plugins: [react(), tailwindcss(),
  ...(baseline ? [{name:'qualified-root-focus-baseline',enforce:'pre',load(id){if(id.split('?')[0]===resolve(root,cssPath))return css.toString()}}] : [])], logLevel: 'error',
  cacheDir: resolve(output, 'vite-cache'), server: { host: '127.0.0.1', port: 5298, strictPort: true } })
let browser
let resourceFailure
const watcher=setInterval(()=>{void floor().catch(error=>{resourceFailure=String(error);void browser?.close()})},2000)
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  for (const width of baseline ? [390] : [390, 360, 430]) {
    await floor()
    const context = await browser.newContext({ viewport: { width, height: Math.round(width * 398 / 241) }, hasTouch: true, reducedMotion: 'reduce', serviceWorkers: 'block' })
    const page = await context.newPage(), errors = [], result = { width, errors }
    page.on('pageerror', error => errors.push(error.message))
    try {
      await page.clock.setFixedTime(new Date('2026-11-07T07:00:00.000Z'))
      await page.goto('http://127.0.0.1:5298/src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=%2Fmeet&era=PARADE')
      await expect(page.locator('.meet-party')).toHaveCount(3, { timeout: 30000 })
      await expect(page.locator('.meet-party__portrait canvas')).toHaveCount(3, { timeout: 15000 })
      await page.evaluate(() => document.fonts.ready)
      await page.waitForLoadState('networkidle')
      const metrics = () => page.evaluate(() => {
        const box = el => { const r = el.getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height } }
        const view = document.querySelector('.party-browse')
        return { overflow: document.documentElement.scrollWidth > innerWidth, viewport: { width:innerWidth, height:innerHeight },
          rows:[...view.querySelectorAll('.meet-party')].map(box),
          foreground:[...view.querySelectorAll('.party-tabs,.meet-party__portrait,.meet-party h3,.meet-party__where,.meet-party__members-unavailable,.meet-party__capacity,.meet-party__join,.meet-create')].map(el => ({ selector:el.className || el.tagName, text:el.textContent, rect:box(el), font:getComputedStyle(el).font })),
          controls:[...view.querySelectorAll('button,a,summary')].filter(el => el.checkVisibility()).map(el => ({ name:el.getAttribute('aria-label') || el.textContent, disabled:!!el.disabled, rect:box(el) })) }
      })
      result.initial = await metrics()
      result.characters=await page.locator('.meet-party__portrait canvas').evaluateAll(async els=>Promise.all(els.map(async el=>{const s=getComputedStyle(el),r=el.getBoundingClientRect(),pixels=el.getContext('2d').getImageData(0,0,el.width,el.height).data;return {width:el.width,height:el.height,cssWidth:r.width,cssHeight:r.height,objectFit:s.objectFit,imageRendering:s.imageRendering,rgba:[...new Uint8Array(await crypto.subtle.digest('SHA-256',pixels))].map(n=>n.toString(16).padStart(2,'0')).join('')}})))
      assert.equal(result.initial.overflow, false)
      await page.screenshot({ path: resolve(output, `meet-${width}.png`) })
      if(width===390){await page.setViewportSize({width,height:844});await page.screenshot({path:resolve(output,'meet-390x844.png')});await page.setViewportSize({width,height:Math.round(width*398/241)})}
      const joins = page.locator('.meet-party__join')
      result.joinReachability = []
      for (const join of await joins.all()) {
        await join.scrollIntoViewIfNeeded(); await join.focus()
        const reached = await join.evaluate(el => {
          const r=el.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2),s=getComputedStyle(el)
          return { width:r.width,height:r.height,hit:hit===el||el.contains(hit),focus:el===document.activeElement,outline:s.outlineStyle,outlineColor:s.outlineColor }
        })
        assert(reached.width>=44 && reached.height>=44 && reached.hit && reached.focus && reached.outline!=='none')
        result.joinReachability.push(reached)
      }
      await joins.first().scrollIntoViewIfNeeded(); await joins.first().focus()
      await page.screenshot({path:resolve(output, `focus-${width}.png`)})
      const state = () => page.evaluate(() => window.__COZY_PANEL__.state())
      const before = (await state()).requests.filter(r=>r.method!=='GET').length
      await joins.first().press('Enter')
      await expect.poll(async()=>(await state()).path).toMatch(/^\/meet\/parties\//)
      assert.equal((await state()).requests.filter(r=>r.method!=='GET').length,before)
      await page.goBack()
      await expect(page.locator('.meet-party')).toHaveCount(3)
      result.detailReadNoMutation = true
      const plan = page.getByRole('button',{name:'MY PLAN',exact:true})
      await plan.focus(); await plan.press('Enter'); await expect(plan).toHaveAttribute('aria-pressed','true')
      await page.getByRole('button',{name:'Parties',exact:true}).click()
      const filters = page.locator('.party-filters summary')
      await filters.focus(); await filters.press('Enter'); await expect(page.locator('.party-filters')).toHaveAttribute('open','')
      await expect(page.getByRole('button',{name:'Show parties',exact:true})).toBeVisible()
      await page.screenshot({path:resolve(output,`filters-${width}.png`)})
      await filters.press('Enter'); await expect(page.locator('.party-filters')).not.toHaveAttribute('open','')
      result.tabsAndFilters = true
      await page.emulateMedia({forcedColors:'active'})
      await filters.focus()
      result.forcedFocus=await filters.evaluate(el=>{const s=getComputedStyle(el);return {outline:s.outlineStyle,width:s.outlineWidth,offset:s.outlineOffset,adjust:s.forcedColorAdjust}})
      assert.notEqual(result.forcedFocus.outline,'none');assert.equal(result.forcedFocus.width,'3px');assert.equal(result.forcedFocus.offset,'-3px')
      await page.screenshot({path:resolve(output,`forced-focus-${width}.png`)})
      await page.emulateMedia({forcedColors:'none'})
      await page.locator('.meet-party h3 a').first().evaluate(el=>{el.textContent='TEST ONLY long party title with enough words to wrap naturally across several lines'})
      result.longTitle = await metrics()
      assert.equal(result.longTitle.overflow,false)
      await page.locator('.meet-party').first().scrollIntoViewIfNeeded()
      await page.screenshot({path:resolve(output,`long-title-${width}.png`)})
      assert.equal(resourceFailure,undefined)
      result.status='PASS'
      assert.equal(errors.length,0)
    } catch (error) { result.status='FAIL';result.error=String(error.stack);await page.screenshot({path:resolve(output,`failure-${width}.png`)}).catch(()=>{}) }
    finally { report.cases.push(result);await page.evaluate(()=>window.__COZY_PANEL__?.dispose()).catch(()=>{});await context.close() }
  }
} finally {
  clearInterval(watcher)
  await browser?.close();await server.close();await rm(resolve(output,'vite-cache'),{recursive:true,force:true})
  await floor()
  await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n')
}
console.log(JSON.stringify(report.cases.map(({width,status,error})=>({width,status,error}))))
if(report.cases.length!==(baseline?1:3) || report.cases.some(c=>c.status!=='PASS'))process.exitCode=1
