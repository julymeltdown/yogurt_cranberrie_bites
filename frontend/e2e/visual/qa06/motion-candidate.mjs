// Independent, read-only candidate-component capture. Never activates public motion admission.
import assert from 'node:assert/strict'
import {execFileSync} from 'node:child_process'
import {readFile,writeFile,mkdir} from 'node:fs/promises'
import {createHash} from 'node:crypto'
import {resolve,extname} from 'node:path'
import {pathToFileURL} from 'node:url'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
import {requireCaptureSpace,reservePort,temporaryCache,clearTemporaryCache} from './resources.mjs'
const qa=resolve(import.meta.dirname,'../../../..'),repo=process.env.QA06_MOTION_REPO,out=process.env.QA06_MOTION_OUTPUT
assert(repo&&out?.startsWith(resolve(qa,'docs/mcr/evidence/visual/qa06/')+'/'))
const pin='228e1cd49e174f31795e648284b96a0f4ff0c44c',git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:64*1024*1024}),hash=b=>createHash('sha256').update(b).digest('hex')
assert.equal(git('diff','--name-only','--','frontend/src').toString(),'','Candidate source must be clean')
const paths=git('ls-tree','-r','--name-only',pin,'frontend/src').toString().trim().split('\n'),sourceHashes={}
for(const p of paths){const actual=await readFile(resolve(repo,p));sourceHashes[p]=hash(actual);assert.equal(sourceHashes[p],hash(git('show',pin+':'+p)),p)}
process.env.MCR_FRONTEND_DEPENDENCIES='/Users/lhs/Desktop/mcr/frontend'
const helper=resolve(repo,'frontend/src/world/avatar/__tests__/motion-adoption')
const {aliases}=await import(pathToFileURL(resolve(helper,'vitest.config.mjs')))
const {producerInputs,readPinnedProducer}=await import(pathToFileURL(resolve(helper,'producer-inputs.ts')))
const manifest=readPinnedProducer('backend/releases/selected-character-motion-v2/manifest.json'),preset=JSON.parse(new TextDecoder().decode(readPinnedProducer('contracts/fixtures/assets/motion-v2/valid/preset-01.json')))
await requireCaptureSpace(qa);await mkdir(out,{mode:0o700})
const tracked=new Set(git('ls-tree','-r','--name-only',pin,'frontend/public').toString().trim().split('\n')),bytes=new Map(),cacheDir=temporaryCache('motion'),port=await reservePort()
let writes=0,browser
const report={task:'QA06',status:'IN_PROGRESS',qualification:'TEST_ONLY_CANDIDATE_COMPONENT_NOT_PUBLIC_ROOT_OR_DEVICE_ACCEPTANCE',sourceCommit:pin,checkoutHead:git('rev-parse','HEAD').toString().trim(),sourceHashes,harnessSha256:hash(await readFile(new URL(import.meta.url))),producer:producerInputs,cases:[]}
const server=await createServer({configFile:false,root:resolve(repo,'frontend'),cacheDir,logLevel:'error',resolve:{alias:Object.entries(aliases).sort(([a],[b])=>b.length-a.length).map(([find,replacement])=>({find,replacement}))},server:{host:'127.0.0.1',port,strictPort:true,hmr:false,fs:{allow:[repo,process.env.MCR_FRONTEND_DEPENDENCIES]}},plugins:[{name:'qa06-readonly-motion',configureServer(s){s.middlewares.use((req,res,next)=>{
 const path=new URL(req.url,'http://fixture.test').pathname
 if(path.startsWith('/__motion/')){if(req.method!=='GET'){writes++;res.writeHead(405);res.end();return}res.writeHead(200,{'Content-Type':'application/json','Cache-Control':'no-store'});res.end(path==='/__motion/candidate'?manifest:JSON.stringify({revision:1,config:preset}));return}
 const source='frontend/public'+decodeURIComponent(path)
 if(!tracked.has(source)||!['.png','.ttf'].includes(extname(source)))return next()
 try{if(!bytes.has(source))bytes.set(source,git('show',pin+':'+source));res.setHeader('Content-Type',extname(source)==='.png'?'image/png':'font/ttf');res.end(bytes.get(source))}catch(e){next(e)}
 })}}]})
try{await server.listen();browser=await chromium.launch({channel:'chrome',headless:true});report.browser=browser.version()
 for(const width of [360,390,430]){await requireCaptureSpace(qa);const context=await browser.newContext({viewport:{width,height:844},isMobile:true,hasTouch:true,reducedMotion:'reduce'}),page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));page.setDefaultTimeout(20000)
  try{await page.goto(`http://127.0.0.1:${port}/src/app/character-entry/__tests__/motion-adoption/fixture.html`);await expect.poll(()=>page.evaluate(()=>window.motionJourney?.state().scene.sprites)).toBe(2)
   await page.getByRole('button',{name:'CUSTOMIZE',exact:true}).click();await page.getByRole('button',{name:'Hair',exact:true}).click();await page.getByLabel('Choose hair',{exact:true}).selectOption('hair-5');await page.getByLabel('View',{exact:true}).selectOption('down')
   const preview=page.getByRole('img',{name:'Draft outfit preview',exact:true}),rail=page.getByRole('group',{name:'Clothing categories',exact:true}),panel=page.getByRole('region',{name:'Avatar editor',exact:true})
   await expect(preview).toHaveAttribute('data-frame','0');await expect(page.getByRole('button',{name:'Choose hair 6',exact:true})).toHaveAttribute('aria-pressed','true');await page.evaluate(()=>document.fonts.ready)
   const categories=await rail.getByRole('button').evaluateAll(nodes=>nodes.map(n=>{const r=n.getBoundingClientRect();return {label:n.getAttribute('aria-label'),x:r.x,y:r.y,width:r.width,height:r.height}})),p=await preview.boundingBox(),r=await rail.boundingBox()
   assert.equal(categories.length,5);assert(categories.every((c,i)=>c.width>=44&&c.height>=44&&(!i||c.y>categories[i-1].y)));assert(r.x>=p.x+p.width)
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);assert.deepEqual(errors,[])
   const state=await page.evaluate(()=>window.motionJourney.state());assert.equal(state.mounts,1);assert.equal(state.gameCanvases,1);assert.equal(state.sockets,0);assert.equal(writes,0)
   const screenshot=`customize-component-${width}.png`;await panel.screenshot({path:resolve(out,screenshot)});report.cases.push({width,status:'CAPTURED',categories,state,errors,screenshot,screenshotSha256:hash(await readFile(resolve(out,screenshot)))})
  }finally{await page.evaluate(()=>window.motionJourney?.dispose()).catch(()=>{});await context.close()}
 }
 report.status='CANDIDATE_COMPONENT_CAPTURED';report.writes=writes
}catch(e){report.status='FAIL';report.error=e.message;process.exitCode=1}
finally{await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();await server.close();await clearTemporaryCache(cacheDir)}
console.log(JSON.stringify({status:report.status,cases:report.cases.length,error:report.error}))
