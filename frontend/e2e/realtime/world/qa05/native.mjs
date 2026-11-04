import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {readFile,writeFile,mkdir,mkdtemp,rm,statfs} from 'node:fs/promises'
import {resolve} from 'node:path'
import {tmpdir} from 'node:os'
import {execFileSync} from 'node:child_process'
import {createServer} from 'vite'
import {expect} from '@playwright/test'
import {faultAuthority} from './peer.mjs'
import {nativeBrowser} from './browser.mjs'

const frontend=resolve(import.meta.dirname,'../../../..'),repo=resolve(frontend,'..'),out=resolve(repo,'docs/mcr/evidence/realtime/world-client'),captures=resolve(import.meta.dirname,'captures')
const disk=await statfs(repo);assert(disk.bavail*disk.bsize>=8*1024**3,'8GiB disk floor')
const cache=await mkdtemp(resolve(tmpdir(),'mcr-qa05-client-')),authority=faultAuthority(),hash=b=>createHash('sha256').update(b).digest('hex')
await mkdir(out,{recursive:true});await mkdir(captures,{recursive:true})
const report={status:'FAIL',candidate:execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim(),qualification:'Actual application/canonical Auth/production WebSocket transport/default Phaser + local explicit C05 fault peer. No Rust service, real server restart/replay enforcement, two-principal, device or IN acceptance.',pins:{},cases:[],errors:[]}
const paths=execFileSync('git',['ls-files','frontend/src','contracts'],{cwd:repo,encoding:'utf8'}).trim().split('\n').filter(p=>/\.(tsx?|css|json)$/.test(p)&&!p.includes('/evidence/'))
paths.push(...['preview.tsx','preview.html','peer.mjs','browser.mjs','native.mjs','check-types.mjs'].map(p=>'frontend/e2e/realtime/world/qa05/'+p))
for(const p of paths)report.pins[p]=hash(await readFile(resolve(repo,p)))
const server=await createServer({root:frontend,cacheDir:cache,logLevel:'warn',server:{host:'127.0.0.1',port:0,strictPort:false},plugins:[{name:'qa05-local-peer',configureServer(vite){vite.middlewares.use(authority.middleware)}}]})
let browser,native,page,stage='startup'
const read=()=>page.evaluate(()=>window.__QA05_CLIENT__.inputState()),world=()=>page.evaluate(()=>window.__QA05_CLIENT__.inspectWorld())
const api=(name,arg)=>page.evaluate(({name,arg})=>window.__QA05_CLIENT__[name](arg),{name,arg})
try{
 await server.listen();authority.attach(server.httpServer)
 native=await nativeBrowser();browser=native.browser;report.browser=browser.version()
 for(const width of(process.env.MCR_QA05_WIDTH?[+process.env.MCR_QA05_WIDTH]:[360,390,430])){
  const id='width-'+width,context=native.context
  page=await context.newPage();await page.bringToFront();await page.setViewportSize({width,height:844});page.on('pageerror',e=>report.errors.push(e.message));const cdp=await context.newCDPSession(page)
  await cdp.send('Emulation.setDeviceMetricsOverride',{width,height:844,deviceScaleFactor:1,mobile:true});await cdp.send('Emulation.setTouchEmulationEnabled',{enabled:true,maxTouchPoints:1})
  await page.addInitScript(()=>{window.qa05Visibility=[];document.addEventListener('visibilitychange',e=>window.qa05Visibility.push({hidden:document.hidden,trusted:e.isTrusted}))})
  const result={width,steps:[]};report.cases.push(result)
  const sample=async name=>{const row={name,input:await read(),world:await world(),peer:authority.inspect(id)};result.steps.push(row);return row}
  const live=async()=>{await expect.poll(async()=>(await read()).root.phase,{timeout:15000}).toBe('live');await expect.poll(async()=>(await world()).sprites).toBe(authority.inspect(id).roster.length)}
  const shot=async name=>{const file=`${name}-${width}.png`;await page.screenshot({path:resolve(captures,file)});return {file:'frontend/e2e/realtime/world/qa05/captures/'+file,sha256:hash(await readFile(resolve(captures,file)))}}
  const canvas=page.locator('.shell-world-host canvas'),stick=page.locator('.world-joystick')
  stage=`${width}/boot`;await page.goto(server.resolvedUrls.local[0]+`e2e/realtime/world/qa05/preview.html?source=derived&peer=${id}`)
  await expect.poll(()=>page.evaluate(()=>Boolean(window.__QA05_CLIENT__)),{timeout:20000}).toBe(true);await live();await expect(stick).toBeEnabled();await sample('real-root-native-websocket-ready')

  // The original gap criterion, observed in the real render sink and Nearby DOM.
  stage=`${width}/gap`;await api('navigate','/world/nearby');await expect(page.getByRole('region',{name:'Other people in this world'})).toBeVisible()
  authority.fault(id,'gap');await expect.poll(async()=>(await read()).root.phase).toBe('resync');await expect.poll(async()=>(await world()).sprites).toBe(0)
  await expect(page.getByText('Updating who is visible. The previous list has been cleared.',{exact:true})).toBeVisible()
  assert.equal(authority.inspect(id).resyncs.length,1);const baseline=(await read()).projection.baseline.streamSeq
  authority.fault(id,'old');await page.waitForTimeout(120);assert.equal((await read()).projection.baseline.streamSeq,baseline);assert.equal((await world()).sprites,0);assert.equal(authority.inspect(id).resyncs.length,1)
  const gap=await sample('gap-clears-renderer-and-nearby');gap.capture=await shot('gap')
  authority.fault(id,'release');await live();await expect(page.getByRole('link',{name:/Recovered peer/})).toHaveCount(1);await expect(page.getByRole('link',{name:/Scene visitor/})).toHaveCount(0)
  const restored=await sample('complete-authoritative-replacement');assert.deepEqual(restored.input.projection.entities.map(e=>e.player.userId).sort(),restored.peer.roster.map(e=>e.userId).sort())
  for(const entity of restored.input.projection.entities)assert(restored.world.rendered.some(s=>s.x===entity.position.x&&s.y===entity.position.y))

  stage=`${width}/epoch`;authority.fault(id,'epoch');await live();await expect(page.getByRole('link',{name:/Recovered peer/})).toHaveCount(2);await sample('epoch-reset-removes-old-roster')
  await api('navigate','/world');await expect(stick).toBeEnabled()
  stage=`${width}/interpolation`;const frames=page.evaluate(async()=>{const values=[],start=performance.now();while(performance.now()-start<300&&values.length<1000){await new Promise(requestAnimationFrame);values.push(await window.__QA05_CLIENT__.inspectWorld())}return values});authority.fault(id,'remoteMove');const rendered=await frames,ys=rendered.map(s=>Math.max(...s.rendered.map(p=>p.y)));assert(ys.some(y=>y>456&&y<500));assert.equal(ys.at(-1),500);assert(ys.every(y=>y>=456&&y<=500));const interpolated=await sample('native-remote-interpolation-without-extrapolation');interpolated.renderedY=ys
  await api('navigate','/world');await expect(stick).toBeEnabled()

  // Delay a single ACK first, then withhold ACKs under actual held keyboard input.
  stage=`${width}/delay`;authority.fault(id,'hold');const beforeDelay=await read(),ack=beforeDelay.projection.ackInputSeq
  await canvas.focus();await page.keyboard.down('ArrowRight');await expect.poll(async()=>(await read()).projection.pendingInputs).toBeGreaterThan(0);await page.keyboard.up('ArrowRight')
  const delayed=await sample('packet-delay-keeps-authoritative-coordinates');assert.equal(delayed.input.projection.ackInputSeq,ack);assert.deepEqual(delayed.input.self,beforeDelay.self)
  authority.fault(id,'deliver');await expect.poll(async()=>(await read()).projection.pendingInputs).toBe(0)
  stage=`${width}/slow`;authority.fault(id,'hold');await canvas.focus();await page.keyboard.down('ArrowRight')
  const pending=[];for(let i=0;i<42;i++){const s=await read();pending.push(s.projection.pendingInputs);if(s.root.phase==='resync')break;await page.waitForTimeout(100)}
  await expect.poll(async()=>(await read()).root.phase).toBe('resync');await page.keyboard.up('ArrowRight');assert(Math.max(...pending)<=64);assert(pending.some(n=>n>=60))
  assert.equal(authority.inspect(id).resyncs.at(-1).reason,'PROTOCOL_RECOVERY');assert.equal((await world()).sprites,0);await expect(stick).toBeDisabled()
  const slow=await sample('bounded-unacknowledged-input-with-native-key');slow.pendingSamples=pending;authority.fault(id,'release');await live();await expect(stick).toBeEnabled()

  // A real WS close is delivered by the local peer; retry remains a native UI action.
  stage=`${width}/restart`;const connections=authority.inspect(id).connections;authority.fault(id,'restart')
  await expect.poll(async()=>(await read()).root.phase).toBe('closed');await expect.poll(async()=>(await world()).sprites).toBe(0);await page.waitForTimeout(150);assert.equal(authority.inspect(id).connections,connections)
  await page.getByRole('button',{name:'Reconnect world',exact:true}).click();await live();assert.equal(authority.inspect(id).connections,connections+1);await sample('restart-close-retry-fresh-baseline')

  stage=`${width}/rooms`;await api('navigate','/world/rooms');const change=page.getByRole('button',{name:'Switch to SECOND ROOM',exact:true});await expect(change).toBeEnabled()
  const oldRoom=authority.inspect(id).roomId,oldConnections=authority.inspect(id).connections;authority.fault(id,'rejectSwitch');await change.click();await expect(page.getByRole('alert')).toContainText('The switch was not accepted')
  assert.equal(authority.inspect(id).roomId,oldRoom);await sample('room-rejection-retains-membership');await change.click();await expect(page.getByText('Room switch confirmed.',{exact:true})).toBeVisible();await live();assert.notEqual(authority.inspect(id).roomId,oldRoom);assert.equal(authority.inspect(id).connections,oldConnections);await sample('room-switch-atomic-baseline')
  await api('navigate','/world');await expect(stick).toBeEnabled()

  stage=`${width}/background`;await canvas.focus();await page.keyboard.press('e');const beforeHeld=authority.inspect(id).trace.length;await page.keyboard.down('ArrowRight');await expect.poll(()=>authority.inspect(id).trace.slice(beforeHeld).some(m=>m.type==='input'&&m.axisX===1)).toBe(true)
  const beforeHidden=authority.inspect(id).connections,other=await context.newPage();await other.goto('about:blank');await other.bringToFront()
  await expect.poll(()=>page.evaluate(()=>document.hidden)).toBe(true);await expect.poll(async()=>(await read()).root.phase).toBe('suspended');await page.keyboard.up('ArrowRight')
  await expect.poll(()=>authority.inspect(id).active).toBe(0);assert.equal((await world()).sprites,0);await sample('native-tab-background-withdrawal')
  await other.close();await page.bringToFront();await expect.poll(()=>page.evaluate(()=>document.hidden)).toBe(false);await live()
  assert.equal(authority.inspect(id).connections,beforeHidden+1);await expect(stick).toBeEnabled();await sample('native-foreground-fresh-ticket-no-held-replay')
  await expect.poll(()=>authority.inspect(id).trace.filter(m=>m.direction==='client'&&m.connection===beforeHidden+1&&m.type==='input').length).toBeGreaterThan(0);const resumed=authority.inspect(id).trace.filter(m=>m.direction==='client'&&m.connection===beforeHidden+1&&m.type==='input');assert.equal(resumed[0].seq,1);assert(resumed.every(m=>m.axisX===0&&m.axisY===0&&m.buttons.length===0));result.visibility=await page.evaluate(()=>window.qa05Visibility);assert(result.visibility.some(e=>e.hidden&&e.trusted))

  stage=`${width}/ticket-replay-client`;authority.fault(id,'reuseTicket');authority.fault(id,'drop');await expect.poll(async()=>(await read()).root.phase).toBe('closed')
  const beforeReplay=authority.inspect(id).connections;await page.getByRole('button',{name:'Reconnect world',exact:true}).click();await expect.poll(async()=>(await read()).root.phase).toBe('closed');assert.equal(authority.inspect(id).connections,beforeReplay)
  await sample('client-rejects-reissued-used-ticket');await page.getByRole('button',{name:'Reconnect world',exact:true}).click();await live()
  const final=await sample('fresh-retry-after-replay-denial');final.capture=await shot('recovered')
  for(const row of result.steps){assert.equal(row.world.totalGames,1);assert.equal(row.world.peakGames,1);assert.equal(row.peer.peak,1);assert.equal(row.input.root.sameCamera,true);assert.equal(row.input.root.factoryCalls,1);assert.equal(row.input.root.canvasPeak,1)}
  for(const req of final.peer.trace.filter(m=>m.type==='authenticate'))assert(!('ticket'in req),'no credential traces')
  stage=`${width}/privacy`;await api('suspend');await expect.poll(async()=>(await world()).games).toBe(0);await expect.poll(()=>authority.inspect(id).active).toBe(0);assert.equal((await read()).controls,0)
  await api('dispose');const cleaned=await read();assert(cleaned.root.sourceCaches.every(c=>c.disposed&&c.residentBytes===0));assert.deepEqual(new Set(cleaned.root.urls.created),new Set(cleaned.root.urls.revoked));result.cleanup={input:cleaned,world:await world(),peer:authority.inspect(id)};result.status='PASS';await page.close()
 }
 assert.deepEqual(report.errors,[]);assert.deepEqual(authority.errors,[]);report.status='PASS'
}catch(e){report.failure={stage,error:String(e.stack),input:await read().catch(()=>null),world:await world().catch(()=>null),authorityErrors:authority.errors};await page?.screenshot({path:resolve(captures,'failure.png')}).catch(()=>{});throw e}
finally{await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2)+'\n');await page?.evaluate(()=>window.__QA05_CLIENT__?.dispose()).catch(()=>{});await native?.close();authority.close();await server.close();await rm(cache,{recursive:true,force:true})}
console.log(JSON.stringify({status:report.status,widths:report.cases.map(c=>c.width)}))
