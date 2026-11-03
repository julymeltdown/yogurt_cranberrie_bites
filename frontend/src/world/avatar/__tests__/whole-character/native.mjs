import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, writeFile, rm, statfs } from 'node:fs/promises'
import { resolve, join } from 'node:path'
import { tmpdir } from 'node:os'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../../..'),out=resolve(import.meta.dirname,'evidence');const disk=await statfs(root);assert(disk.bavail*disk.bsize>=8*1024**3,'Disk floor')
await mkdir(out,{recursive:true});const cache=await mkdtemp(join(tmpdir(),'mcr-whole-world-')),hash=x=>createHash('sha256').update(x).digest('hex')
const sources=['src/world/avatar/whole-character.ts','src/world/avatar/scene.ts','src/world/scenes/MapScene.ts','src/world/runtime.ts','src/world/network/transport/contract.ts','src/world/network/transport/wire-types.ts','src/entities/avatar/whole-character/runtime.ts','src/entities/avatar/whole-character/contract.ts','src/entities/avatar/whole-character/index.ts','src/world/avatar/__tests__/whole-character/native.ts','src/world/avatar/__tests__/whole-character/native.html','src/world/avatar/__tests__/whole-character/native.mjs','../contracts/fixtures/assets/selected-static/avatar-manifest.json','../contracts/assets/v1/avatar-config.schema.json','../contracts/realtime/v1/world.schema.json','public/world/avatars/derived-v1/static-release.json','public/world/avatars/derived-v1/photo01-front-idle-64-e124c88815b22220.png']
const report={result:'FAIL',qualification:'Genuine frozen canonical whole-character configs/source PNG bytes and published shared runtime in incumbent production Phaser/map/renderer. Actual C05 decoder exercised; deterministic isolated roster/privacy owner, no live socket/Auth server, devices or multiuser acceptance.',testedParent:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),sources:Object.fromEntries(await Promise.all(sources.map(async p=>[p,hash(await readFile(resolve(root,p)))]))),cases:[],artifacts:{},errors:[]}
let browser,page,stage='start';const server=await createServer({root,configLoader:'runner',cacheDir:cache,server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'});await server.listen()
const state=()=>page.evaluate(()=>window.wholeWorld?.state()),invoke=(method,...args)=>page.evaluate(async({method,args})=>window.wholeWorld[method](...args),{method,args})
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
for(const [width,height]of[[360,800],[390,844],[430,932]]){
 stage=`${width}/ready`;page=await browser.newPage({viewport:{width,height},reducedMotion:'reduce'});page.setDefaultTimeout(10000);page.on('pageerror',e=>report.errors.push(e.message))
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/src/world/avatar/__tests__/whole-character/native.html`)
 await expect.poll(async()=>(await state())?.scene.sprites).toBe(6);await expect.poll(async()=>(await state()).status).toBe('ready')
 const initial=await state();assert.equal(initial.scene.textures,6);assert.equal(initial.mounts,1);assert.equal(initial.canvases,1);assert.equal(initial.owner.decodedAtlases,1);assert.equal(initial.requests.length,2);assert(initial.requests.filter(p=>p.endsWith('.png')).every(p=>p.includes('front-idle-64')))
 const art=await invoke('artFrames');assert.equal(art.images,1);assert.equal(new Set(art.hashes).size,6);assert.deepEqual(art.sources,[0,64,128,192,256,320])
 const contract=await invoke('sceneContract');assert.equal(contract.rejected,7);assert.equal((await state()).scene.textures,6)
 const file=`six-front-idle-${width}.jpg`;await page.screenshot({path:resolve(out,file),type:'jpeg',quality:85});report.artifacts[file]=hash(await readFile(resolve(out,file)))
 await invoke('move');await expect.poll(async()=>(await state()).scene.sprites).toBe(6);assert.equal((await state()).scene.textures,6)
 const geometry=(await state()).geometry;for(const era of[0,2,1]){await invoke('era',era);assert.equal((await state()).geometry,geometry);assert.equal((await state()).scene.textures,6);assert.equal((await state()).requests.length,2)}
 await invoke('visible',false);const hidden=await state();assert.equal(hidden.scene.sprites,0);assert.equal(hidden.scene.textures,0);assert.equal(hidden.owner.leases,0)
 await invoke('visible',true);await expect.poll(async()=>(await state()).scene.sprites).toBe(6);assert.equal((await state()).requests.length,2)
 await invoke('config');await expect.poll(async()=>(await state()).scene.textures).toBe(5);assert.equal((await state()).scene.sprites,6)
 await invoke('privacy');const revoked=await state();assert.equal(revoked.scene.sprites,0);assert.equal(revoked.scene.textures,0);assert.equal(revoked.scene.listeners,0);assert.equal(revoked.owner.leases,0)
 const cleanup=await invoke('dispose');assert.equal(cleanup.canvases,0);assert.deepEqual(cleanup.scene,{textures:0,sprites:0,listeners:0});assert.equal(cleanup.mounts,1);assert.equal(cleanup.destroys,1)
 report.cases.push({width,height,initial,art,contract,hidden,revoked,cleanup});await page.close()
}assert.deepEqual(report.errors,[]);report.result='PASS'
}catch(error){report.failure={stage,error:String(error)};throw error}
finally{await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await rm(cache,{recursive:true,force:true})}
console.log(JSON.stringify({result:report.result,cases:report.cases.length,artifacts:Object.keys(report.artifacts)}))
