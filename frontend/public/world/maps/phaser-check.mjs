// Bounded FE14 consumer proof on real AR02 geometry/AR04 pixels, not product admission.
import { createRequire } from 'node:module'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve, dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, '../../../..')
const modules = process.env.MCR_NODE_MODULE_ROOT ?? '/Users/lhs/Desktop/mcr/frontend/node_modules'
const require = createRequire(resolve(modules, '../package.json'))
const { chromium } = require('@playwright/test'), evidence = join(root, 'docs/mcr/art/evidence/maps')
const sourceHashes = {}
for (const path of ['frontend/src/world/map/resources.ts', 'frontend/src/world/map/manifest.ts', 'frontend/src/world/map/tiled.ts',
  'frontend/src/world/map/json.ts', 'frontend/src/world/scenes/MapScene.ts', 'frontend/src/world/theme/compatibility.ts',
  'frontend/public/world/maps/geometry.json', 'frontend/public/world/maps/courtyard-tiled.json']) {
  sourceHashes[path] = 'sha256:' + createHash('sha256').update(await readFile(join(root,path))).digest('hex')
}
const phaserPath = require.resolve('phaser/dist/phaser.js')
const browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
const results = [], errors = [], requests = [], teardown = []
const pointIds = ['studio_info','park_info','stage_info','gifts_info','wall_info']
try {
  for (const width of [360,390,430]) {
    const page = await browser.newPage({ viewport: { width, height: 844 }, deviceScaleFactor: 1 })
    page.on('pageerror',e=>errors.push(e.message)); page.on('request',r=>requests.push(new URL(r.url()).pathname))
    await page.goto('http://127.0.0.1:8874/world/maps/phaser-review.html')
    await page.waitForFunction(()=>window.ar02Phaser?.ready,null,{timeout:20000})
    const firstCanvas = await page.locator('#world canvas').elementHandle(), signatures = new Set()
    for (const theme of ['REVENGE','PARADE','DANGER']) {
      await page.selectOption('#theme',theme)
      await page.waitForFunction(t=>window.ar02Phaser?.ready&&window.ar02Phaser.theme===t,theme)
      const pointResults=[]
      for (const [i,id] of pointIds.entries()) {
        await page.selectOption('#target',String(i))
        await page.waitForFunction(id=>window.ar02Phaser?.ready&&window.ar02Phaser.point===id,id)
        const parity=await page.evaluate(()=>window.ar02PhaserParity())
        assert.equal(parity.differentChannels,0,`${width}/${theme}/${id}: Phaser pixels differ from normalized terrain/overdraw`)
        assert.equal(parity.camera.zoom,1)
        pointResults.push({interaction_id:id,...parity})
      }
      await page.selectOption('#target','1')
      await page.waitForFunction(()=>window.ar02Phaser?.ready&&window.ar02Phaser.point==='park_info')
      assert.equal(await page.evaluate(node=>node===document.querySelector('#world canvas'),firstCanvas),true,'skin change replaced persistent Phaser canvas')
      const state=await page.evaluate(()=>window.ar02Phaser)
      assert.equal(state.gameCanvases,1);signatures.add(state.pixelSignature)
      const metrics=await page.evaluate(()=>({width:innerWidth,pageWidth:document.documentElement.scrollWidth,fontSize:parseFloat(getComputedStyle(document.querySelector('#current')).fontSize),
        controls:[...document.querySelectorAll('select')].map(e=>({height:e.getBoundingClientRect().height,width:e.getBoundingClientRect().width})),labels:document.querySelectorAll('#legend li').length}))
      assert.equal(metrics.pageWidth,width);assert.ok(metrics.fontSize>=16&&metrics.controls.every(e=>e.height>=44&&e.width<=width));assert.equal(metrics.labels,5)
      const screenshot=`phaser-${width}-${theme.toLowerCase()}.png`
      await page.screenshot({path:join(evidence,screenshot),fullPage:true})
      results.push({width,theme,state,metrics,points:pointResults,screenshot,persistentCanvas:'PASS'})
    }
    assert.equal(signatures.size,3,'skins must change rendered pixels')
    const negative=await page.evaluate(()=>window.ar02PhaserNegatives())
    assert.deepEqual(negative.map(x=>x.kind),['tiled-digest','png-digest','png-dimensions','tiled-grid'])
    assert.ok(negative.every(x=>x.reason==='invalid'))
    const disposed=await page.evaluate(()=>window.ar02PhaserDispose());assert.equal(disposed.canvases,0)
    teardown.push({width,negative,disposed})
    await page.close()
  }
  assert.deepEqual(errors,[])
  assert.ok(!requests.some(p=>p.endsWith('/mcr-theme.png')),'inert texture marker was fetched')
  assert.ok(!requests.some(p=>p.startsWith('/api/')||p.includes('world-ws')),'review opened product APIs/socket')
  const report={status:'MOCK_READY',scope:'Actual FE14 loadMap/resource hashes/PNG decode/MapScene Phaser3.90.0 on original AR02 map and committed AR04 textures; terrain-only candidate manifests in memory',
    browser:await browser.version(),node:process.version,playwright:require('@playwright/test/package.json').version,vite:require('vite/package.json').version,
    phaser:{version:require('phaser/package.json').version,distribution:'phaser/dist/phaser.js',sha256:'sha256:'+createHash('sha256').update(await readFile(phaserPath)).digest('hex'),harnessResolution:'Existing UMD browser build through Vite optimizer; app source and dependencies unchanged'},
    sourceHashes,results,teardown,pageErrors:errors,inertMarkerFetch:'NONE',productApiOrSocket:'NONE',
    summary:{renderCases:results.length,landmarkPixelComparisons:results.reduce((n,r)=>n+r.points.length,0),differentChannels:0,persistentCanvasAcrossSkins:'PASS',corruptResourceRejections:teardown.length*4,canvasCleanup:'PASS'},
    limits:['In-memory review candidate pins only; required avatar identity is explicit unbound sentinel, no avatar art/manifest loaded','No authenticated product shell or final release manifests','No Rust runtime/server collision execution','No physical venue operational qualification','No real device/QA06, multiuser/IN01 or release acceptance']}
  await writeFile(join(evidence,'phaser-validation.json'),JSON.stringify(report,null,2)+'\n')
  console.log(JSON.stringify({status:report.status,browser:report.browser,summary:report.summary,pageErrors:errors},null,2))
} finally {await browser.close()}
