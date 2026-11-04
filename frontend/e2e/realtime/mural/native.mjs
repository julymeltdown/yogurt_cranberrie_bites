import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {execFileSync} from 'node:child_process'
import {readFile,writeFile,mkdir,mkdtemp,rm,statfs} from 'node:fs/promises'
import {resolve,join} from 'node:path'
import {tmpdir} from 'node:os'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
import {nativeBrowser} from '../world/qa05/browser.mjs'
import {muralPeer} from './peer.mjs'

const frontend=resolve(import.meta.dirname,'../../..'),repo=resolve(frontend,'..'),out=resolve(repo,'docs/mcr/evidence/realtime/mural')
const git=(...args)=>execFileSync('git',args,{cwd:repo,encoding:'utf8'}).trim(),hash=b=>createHash('sha256').update(b).digest('hex')
const development=process.argv.includes('--development'),candidate=git('rev-parse','HEAD'),tree=git('rev-parse','HEAD^{tree}')
assert.equal(process.version,'v24.22.2')
if(!development){assert.equal(process.argv[process.argv.indexOf('--candidate')+1],candidate,'Explicit exact candidate required');assert.equal(git('diff','--name-only','HEAD'),'','Tracked source dirty')}
const disk=await statfs(repo);assert.ok(disk.bavail*disk.bsize>8*1024**3,'8GiB disk floor')
const paths=git('ls-files','frontend/src','contracts','frontend/e2e/realtime/world/qa05/browser.mjs','frontend/e2e/realtime/mural','frontend/package.json','frontend/pnpm-lock.yaml').split('\n').filter(p=>/\.(?:[cm]?[jt]sx?|json|yaml|css|html)$/.test(p)&&!p.includes('/evidence/')&&!p.includes('/public/'))
for(const p of ['peer.mjs','native.mjs','data.ts','preview.tsx','preview.html','check-types.mjs','peer.node.mjs'])if(!paths.includes('frontend/e2e/realtime/mural/'+p))paths.push('frontend/e2e/realtime/mural/'+p)
const pins={}
for(const p of paths){const bytes=await readFile(join(repo,p));if(!development)assert.equal(hash(execFileSync('git',['show',candidate+':'+p],{cwd:repo,maxBuffer:32*1024**2})),hash(bytes),p);pins[p]=hash(bytes)}
await mkdir(out,{recursive:true})
const temp=await mkdtemp(join(tmpdir(),'mcr-qa05-mural-'))
const original=JSON.parse(await readFile(join(repo,'contracts/fixtures/core/bootstrap-open.json'),'utf8'))
let peer=muralPeer(original.event.event_id,original.participant.user_id)
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:join(temp,'vite-cache'),server:{host:'127.0.0.1',port:0,strictPort:false},plugins:[{name:'qa05-synthetic-mural-peer',configureServer(s){s.middlewares.use((req,res,next)=>{peer.middleware(req,res,next).catch(error=>{res.statusCode=500;res.end(String(error))})})}}]})
let browser,native
let activeCase=null
const cases=[],errors=[],started=new Date().toISOString()
const read=p=>p.evaluate(()=>window.__MURAL_FAILURE__.read()),inspect=p=>p.evaluate(()=>window.__MURAL_FAILURE__.inspect())
const action=(p,method,...args)=>p.evaluate(({method,args})=>window.__MURAL_FAILURE__[method](...args),{method,args})
const ready=async(p,revision)=>{await expect.poll(async()=> (await read(p)).sync?.phase).toBe('ready');await expect.poll(async()=> (await read(p)).confirmed?.revision).toBe(revision)}
const digest=p=>p.evaluate(()=>window.__MURAL_FAILURE__.digest())
const expectedPixels=()=>hash(Buffer.from(JSON.stringify(peer.snapshot().pixels)))
const expectedRGBA=()=>{const s=peer.snapshot(),b=Buffer.alloc(65536);s.pixels.forEach((n,i)=>{const hex=s.palette[n];b[i*4]=parseInt(hex.slice(1,3),16);b[i*4+1]=parseInt(hex.slice(3,5),16);b[i*4+2]=parseInt(hex.slice(5,7),16);b[i*4+3]=255});return hash(b)}
async function converged(pages,revision){for(const p of pages){await ready(p,revision);await expect.poll(()=>digest(p)).toBe(expectedPixels());await expect.poll(async()=> (await inspect(p)).rgbaHash).toBe(expectedRGBA());const v=await read(p);assert.equal(v.created,1);assert.equal(v.peak,1);assert.equal(v.sockets,1);assert.ok(v.peakInFlight<=2);assert.ok(v.maxQueue<=512);assert.equal((await inspect(p)).objects,1)}}
let base
async function open(context,client,width,omitted=false){const p=await context.newPage();await p.setViewportSize({width,height:844});p.on('pageerror',e=>errors.push({client,message:e.message}));await p.goto(`${base}/e2e/realtime/mural/preview.html?client=${client}${omitted?'&omitted':''}`);await expect.poll(async()=> (await read(p)).phase).toBe('live');return p}
try{
 await server.listen();base=`http://127.0.0.1:${server.httpServer.address().port}`
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 for(const width of [360,390,430]){
  peer.dispose();peer=muralPeer(original.event.event_id,original.participant.user_id)
  const contexts=await Promise.all([browser.newContext(),browser.newContext()]),pages=await Promise.all(contexts.map((c,i)=>open(c,i?'b':'a',width)))
  const [a,b]=pages,steps=[];activeCase={width,steps}
  const record=async name=>steps.push({name,clients:await Promise.all(pages.map(read)),rasters:await Promise.all(pages.map(inspect)),peer:peer.stats()})
  await converged(pages,10);await record('initial-two-independent-shells')
  // Leased in-process delivery is the real consumer seam, not a fabricated mural WS message.
  const pair=peer.paint(2),gap=await action(a,'deliver',peer.delta(11,[pair[1]]));assert.equal(gap.state.phase,'gap');assert.equal(gap.state.queued,1)
  const filled=await action(a,'deliver',peer.delta(10,[pair[0]]));assert.equal(filled.state.revision,12);assert.equal(filled.state.queued,0)
  await action(b,'deliver',peer.delta(10,pair));await action(a,'deliver',peer.delta(10,pair));await converged(pages,12);await record('out-of-order-gap-filled-and-duplicate-idempotent')
  const stale=peer.delta(11,[pair[1]]);await action(a,'saveLease')
  // A lost gap times out into a real decoded HTTP snapshot, rather than applying unseen revision13.
  const missing=peer.paint(2),lostGap=await action(a,'deliver',peer.delta(13,[missing[1]]));assert.equal(lostGap.state.phase,'gap')
  await converged(pages,14);await record('missing-packet-gap-deadline-fresh-http-snapshot')
  // Test the published 512 operation bound, then one extra future operation forces a reset.
  const many=peer.paint(514),queued=await action(a,'deliver',peer.delta(15,many.slice(1,513)));assert.equal(queued.state.queued,512)
  const overflow=await action(a,'deliver',peer.delta(527,[many[513]]));assert.equal(overflow.accepted,false);assert.equal(overflow.state.queued,0)
  assert.ok(['loading','ready'].includes(overflow.state.phase));await converged(pages,528);await record('queue512-then-overflow-resnapshot-not-unbounded-buffer')
  // Exercise actual DOM preview/Confirm and C04 definitive rejection.
  peer.mode('rejected');const before=await digest(a),revision=peer.snapshot().revision
  await a.getByRole('group',{name:'Mural pixel preview',exact:true}).focus();await a.keyboard.press('ArrowRight')
  await expect(a.getByLabel('Selection preview',{exact:true})).toBeVisible();await a.getByRole('button',{name:'Confirm pixel',exact:true}).click()
  await expect.poll(async()=> (await read(a)).editor).toBe('rejected');assert.equal((await read(a)).selection,null);assert.equal(await digest(a),before);assert.equal((await read(a)).confirmed.revision,revision);await record('rejected-dom-preview-keeps-confirmed-raster')
  // Lost acknowledgement is not a reject and is never automatically replayed.
  peer.mode('lost');await a.getByRole('group',{name:'Mural pixel preview',exact:true}).focus();await a.keyboard.press('ArrowRight');await a.getByRole('button',{name:'Confirm pixel',exact:true}).click()
  await expect.poll(async()=> (await read(a)).editor).toBe('unknown');const unknown=await read(a),posts=peer.stats().posts;assert.ok(unknown.command)
  await action(a,'saveLease');await action(a,'withdraw');await expect.poll(async()=> (await read(a)).available).toBe(false);assert.equal((await inspect(a)).texture,false);assert.equal((await read(a)).command,unknown.command)
  const obsolete=await action(a,'deliver',stale,true);assert.equal(obsolete.accepted,false)
  peer.restart();peer.mode('ack');assert.equal(await action(a,'qualify'),true);await ready(a,529)
  assert.equal((await read(a)).command,unknown.command);await a.getByRole('button',{name:'Check original command',exact:true}).click();await expect.poll(async()=> (await read(a)).editor).toBe('confirmed')
  assert.equal(peer.stats().posts,posts);assert.equal(peer.stats().replayedPosts,0);await converged(pages,529);await record('fixture-peer-restart-fresh-qualification-original-result-no-replay')
  // Captured pre-restore delivery must not reanimate the old raster or editing state.
  const paused=peer.pause();await Promise.all(pages.map(p=>action(p,'refresh')));await converged(pages,paused.revision);await record('pause-control-boundary-fresh-snapshot-read-only')
  await action(a,'saveLease');await action(a,'withdraw');const restored=peer.restore();assert.equal(await action(a,'qualify'),true)
  await action(b,'refresh');await converged(pages,restored.revision)
  assert.equal((await read(a)).confirmed.state,'PAUSED');assert.equal((await read(a)).selection,null)
  assert.equal((await action(a,'deliver',stale,true)).accepted,false)
  await expect(a.getByRole('button',{name:'Pencil preview',exact:true})).toBeDisabled();await record('restore-paused-obsolete-lease-discarded')
  peer.resume();await Promise.all(pages.map(p=>action(p,'refresh')));await converged(pages,peer.snapshot().revision);await record('resume-control-revision-requires-snapshot')
  // Genuine slow HTTP read on the 390 case hits the consumer's real8s deadline.
  if(width===390){peer.hold('a','delta');await expect.poll(()=>peer.stats().pending,{timeout:5000}).toBe(1);const heldAt=Date.now(),held=await read(a);assert.equal(held.inFlight,1)
   await expect.poll(async()=> (await read(a)).sync?.problem,{timeout:12000}).toBe('timeout');assert.ok(Date.now()-heldAt>=6000);assert.equal((await inspect(a)).texture,false)
   await expect.poll(()=>peer.stats().pending).toBe(0);await action(a,'refresh');await converged(pages,peer.snapshot().revision);await record('slow-http-client-bounded-single-flight-timeout-clears-raster')}
  // Pending snapshot cancellation tests real Fetch AbortSignal teardown.
  peer.hold('a','snapshot');await action(a,'refresh');await expect.poll(()=>peer.stats().pending).toBe(1);await action(a,'withdraw')
  await expect.poll(()=>peer.stats().pending).toBe(0);assert.equal((await inspect(a)).objects,0);assert.equal((await read(a)).inFlight,0)
  peer.release('a','snapshot');assert.equal(await action(a,'qualify'),true);await converged(pages,peer.snapshot().revision);await record('withdraw-cancels-held-http-read-no-ghost-raster')
  const screenshot=`recovered-${width}.png`;await a.screenshot({path:join(out,screenshot),fullPage:true})
  await Promise.all(pages.map(p=>action(p,'privacy')))
  for(const p of pages){await expect.poll(async()=> (await read(p)).active).toBe(0);const v=await read(p);assert.equal(v.sockets,0);assert.equal(v.socketListeners,0);assert.equal(v.sourceListeners,0);assert.equal(v.command,null);assert.equal(v.inFlight,0);assert.equal(v.sync.phase,'disposed');assert.equal((await inspect(p)).texture,false);await action(p,'dispose');await expect(p.locator('#world canvas')).toHaveCount(0)}
  await expect.poll(()=>peer.stats().active).toBe(0);await record('privacy-and-dispose-release-all-owned-leases')
  cases.push({width,steps,screenshot,sha256:hash(await readFile(join(out,screenshot)))})
  await Promise.all(contexts.map(c=>c.close()))
 }
 // Ordinary Playwright contexts force visibility. Use noDefaults CDP for a real tab transition.
 peer.dispose();peer=muralPeer(original.event.event_id,original.participant.user_id);native=await nativeBrowser()
 const page=await open(native.context,'visibility',390);await page.bringToFront();await ready(page,10);await action(page,'saveLease')
 const front=await native.context.newPage();await front.goto('about:blank');await front.bringToFront()
 await expect.poll(async()=> (await read(page)).visible).toBe('hidden');await expect.poll(async()=> (await read(page)).available).toBe(false)
 const hidden=await read(page);assert.equal((await inspect(page)).texture,false)
 peer.paint();await page.bringToFront();await expect.poll(async()=> (await read(page)).visible).toBe('visible');await expect.poll(async()=> (await read(page)).phase).toBe('live')
 assert.equal((await read(page)).available,false);assert.equal(await action(page,'qualify'),true);await ready(page,11)
 assert.equal((await action(page,'deliver',peer.delta(10,[]),true)).accepted,false)
 const foreground=await read(page);assert.equal(foreground.created,1);assert.equal(foreground.sockets,1)
 await action(page,'privacy');await expect.poll(async()=> (await read(page)).active).toBe(0);await action(page,'dispose');await front.close();await page.close()
 const omitted=await open(native.context,'visibility',390,true);await omitted.bringToFront();await expect.poll(async()=> (await read(omitted)).phase).toBe('live')
 const closed=await read(omitted);assert.equal(closed.available,false);assert.equal(closed.factoryCalls,0);assert.equal(closed.inFlight,0);assert.equal(closed.requestCount,0);await action(omitted,'dispose');await omitted.close()
 assert.deepEqual(errors,[])
 if(!development){assert.equal(git('rev-parse','HEAD'),candidate);assert.equal(git('diff','--name-only','HEAD'),'');for(const[p,h]of Object.entries(pins))assert.equal(hash(await readFile(join(repo,p))),h,p)}
 await writeFile(join(out,development?'development.json':'native.json'),JSON.stringify({status:'PASS',scope:'QA05 mural fixture contribution; no whole QA05/live/device/IN04 acceptance',synthetic:true,development,candidate,tree,started,finished:new Date().toISOString(),browser:browser.version(),visibilityBrowser:native.browser.version(),sourcePins:pins,cases,visibility:{hidden,foreground,omitted:closed},errors},null,2)+'\n')
 console.log(JSON.stringify({status:'PASS',candidate,development,widths:cases.length,vitestSubstitute:false,visibility:'native hidden/visible',scope:'fixture-only'}))
}catch(error){await writeFile(join(out,development?'development-failure.json':'failure.json'),JSON.stringify({status:'FAIL',candidate,tree,development,error:String(error.stack),cases,activeCase,errors,peer:peer.stats()},null,2)+'\n');throw error}
finally{peer.dispose();await native?.close();await browser?.close();await server.close();await rm(temp,{recursive:true,force:true})}
