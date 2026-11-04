// Read-only exercise of the retained fixture; all new artifacts stay in this assigned directory.
import assert from 'node:assert/strict'
import { mkdir, mkdtemp, readFile, writeFile, rm, statfs } from 'node:fs/promises'
import { resolve, join } from 'node:path'
import { tmpdir } from 'node:os'
import { createHash } from 'node:crypto'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../..'),out=resolve(import.meta.dirname,'evidence')
const disk=await statfs(root);assert(disk.bavail*disk.bsize>=8*1024**3,'Disk floor');await mkdir(out,{recursive:true})
const cache=await mkdtemp(join(tmpdir(),'mcr-reference-legacy-')),server=await createServer({root,configLoader:'runner',cacheDir:cache,server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'});await server.listen()
const report={result:'FAIL',qualification:'Retained isolated real-art whole-character/illustrated-scene fixture with changed MapScene dispatch; not default product bootstrap/live acceptance.',cases:[],artifacts:{},errors:[],mapSceneSha256:createHash('sha256').update(await readFile(resolve(root,'src/world/scenes/MapScene.ts'))).digest('hex')}
let browser
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
for(const [width,height]of[[360,800],[390,844],[430,932]]){
 const page=await browser.newPage({viewport:{width,height}});page.on('pageerror',e=>report.errors.push(e.message))
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/world/avatar/__tests__/whole-character/native.html`)
 const invoke=(method,...args)=>page.evaluate(async({method,args})=>window.wholeWorld?.[method](...args),{method,args})
 await expect.poll(async()=>(await invoke('state'))?.scene.sprites).toBe(6)
 const initial=await invoke('state');assert.equal(initial.mounts,1);assert.equal(initial.canvases,1);assert.equal(initial.status,'ready')
 const contract=await invoke('sceneContract');assert.equal(contract.rejected,7)
 for(const era of[0,2,1]){await invoke('era',era);assert.equal((await invoke('state')).geometry,initial.geometry);assert.equal((await invoke('state')).scene.sprites,6)}
 const file=`retained-${width}.jpg`;await page.screenshot({path:resolve(out,file),type:'jpeg',quality:70});report.artifacts[file]=createHash('sha256').update(await readFile(resolve(out,file))).digest('hex')
 const cleanup=await invoke('dispose');assert.equal(cleanup.canvases,0);assert.deepEqual(cleanup.scene,{textures:0,sprites:0,listeners:0});assert.equal(cleanup.mounts,1);assert.equal(cleanup.destroys,1)
 report.cases.push({width,height,initial,contract,cleanup});await page.close()
}assert.deepEqual(report.errors,[]);report.result='PASS'
}finally{await writeFile(resolve(out,'retained.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await rm(cache,{recursive:true,force:true})}
console.log(JSON.stringify({result:report.result,cases:report.cases.length}))
