// TEST ONLY: owned ephemeral loopback WebSocket fixture server. No production backend.
import assert from 'node:assert/strict'
import { createServer as createHttpServer } from 'node:http'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { createServer, preview } from 'vite'
import { chromium } from '@playwright/test'
const evidence = process.env.MCR_FE16_EVIDENCE ?? '/Users/lhs/Desktop/mcr-build/fe16'
await mkdir(evidence,{recursive:true})
const fixture = JSON.parse(await readFile(new URL('../../../../../contracts/fixtures/realtime/movement-heartbeat.json',import.meta.url),'utf8'))
const initial = fixture.messages[1].message, ping = fixture.messages[4].message
const wire = data => {
  const payload = Buffer.from(typeof data === 'string' ? data : JSON.stringify(data)), header = payload.length < 126 ? Buffer.from([0x81,payload.length]) : Buffer.from([0x81,126,payload.length>>8,payload.length&255])
  return Buffer.concat([header,payload])
}
const root = fileURLToPath(new URL('../../../../',import.meta.url))
async function unusedPort() {
  const server=createHttpServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve))
  const port=server.address().port;await new Promise(resolve=>server.close(resolve));return port
}
const dev = await createServer({root,server:{host:'127.0.0.1',port:await unusedPort(),strictPort:true}})
let browser,production,scriptTrace,holdAdmission=false
const sockets = new Set(), accepted = [], received = [], urls = [], errors = []
try {
  await dev.listen()
  const base = `http://127.0.0.1:${dev.httpServer.address().port}`
  dev.httpServer.on('upgrade',(request,socket,head) => {
    if (!request.url.startsWith('/world-ws')) return
    assert.equal(request.url,'/world-ws'); assert.equal(request.headers.origin,base); assert.equal(request.headers['sec-websocket-protocol'],undefined)
    urls.push(request.url); sockets.add(socket); accepted.push(socket)
    const digest = createHash('sha1').update(request.headers['sec-websocket-key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${digest}\r\n\r\n`)
    const trace = scriptTrace, accessExpiresAt = new Date(Date.now()+60000).toISOString()
    let cursor = 0
    const deliver = () => { while(trace && cursor < trace.messages.length && trace.messages[cursor].direction === 'server') socket.write(wire(trace.messages[cursor++].message)) }
    let buffer = head
    socket.on('close',()=>sockets.delete(socket)); socket.on('error',()=>sockets.delete(socket))
    socket.on('data',chunk => {
      buffer = Buffer.concat([buffer,chunk]); assert(buffer.length < 8192)
      while(buffer.length>=2) {
        const opcode = buffer[0]&15, masked=!!(buffer[1]&128)
        let length=buffer[1]&127, offset=2
        if(length===126) { if(buffer.length<4)return; length=buffer.readUInt16BE(2);offset=4 }
        assert(length<=4096 && length!==127); assert(masked)
        if(buffer.length<offset+4+length)return
        const mask=buffer.subarray(offset,offset+4), payload=Buffer.from(buffer.subarray(offset+4,offset+4+length)); buffer=buffer.subarray(offset+4+length)
        for(let i=0;i<payload.length;i++) payload[i]^=mask[i%4]
        if(opcode===8) { socket.end(Buffer.from([0x88,0])); continue }
        assert.equal(opcode,1); const message=JSON.parse(payload); received.push(message)
        if (trace) {
          const row=trace.messages[cursor++];assert.equal(row.direction,'client')
          const normalize = value => { const out={...value};delete out.ticket;return out }
          assert.deepEqual(normalize(message),normalize(row.message));deliver()
        } else if(message.type==='authenticate') {
          if (!holdAdmission) { socket.write(wire({...initial,accessExpiresAt})); socket.write(wire(ping)) }
        } else if(message.type==='resync') {
          socket.write(wire({...initial,accessExpiresAt,streamId:fixture.actor_id,cause:'RESYNC',requestId:message.requestId}))
        }
      }
    })
  })
  browser = await chromium.launch({executablePath:process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
  const context = await browser.newContext(), page = await context.newPage()
  page.on('pageerror',e=>errors.push(e.message)); const logs=[]; page.on('console',m=>logs.push(m.text()))
  await page.goto(`${base}/src/world/network/__tests__/fixtures/browser.html`)
  await page.waitForFunction(()=>window.__FE16_TEST_ONLY__)
  await page.evaluate(()=>window.__FE16_TEST_ONLY__.connect())
  await page.waitForFunction(()=>window.__FE16_TEST_ONLY__.events.some(e=>e.type==='frame'&&e.frame.type==='ping'))
  await page.waitForTimeout(50)
  assert(received.some(m=>m.type==='pong')); assert.equal(accepted.length,1)
  await page.evaluate(()=>window.__FE16_TEST_ONLY__.transport.sendInput({seq:1,axisX:0.6,axisY:0.8,buttons:[]}))
  await page.waitForTimeout(50); assert(received.some(m=>m.type==='input'))
  accepted[0].write(wire({...fixture.messages[3].message,streamSeq:2,ackInputSeq:1}))
  await page.waitForFunction(()=>window.__FE16_TEST_ONLY__.transport.state==='resync')
  assert(await page.evaluate(()=>{try{window.__FE16_TEST_ONLY__.transport.sendInput({seq:2,axisX:0,axisY:0,buttons:[]});return false}catch{return true}}))
  accepted[0].write(wire({...fixture.messages[3].message,streamSeq:999,ackInputSeq:999}))
  await page.evaluate(()=>window.__FE16_TEST_ONLY__.transport.resync('00000000-0000-4000-8000-000000000050','GAP'))
  await page.waitForFunction(()=>window.__FE16_TEST_ONLY__.transport.state==='live')
  assert.equal(received.find(m=>m.type==='resync').lastStreamSeq,0)
  assert.equal(await page.evaluate(()=>window.__FE16_TEST_ONLY__.events.filter(e=>e.type==='resync-needed').length),1)
  accepted[0].write(wire({protocolVersion:'1.0.0',type:'unknown'}))
  await page.waitForFunction(()=>window.__FE16_TEST_ONLY__.transport.state==='closed')
  assert.equal(await page.evaluate(()=>window.__FE16_TEST_ONLY__.count()),1)
  await page.evaluate(()=>window.__FE16_TEST_ONLY__.connect())
  assert.equal(accepted.length,2)
  accepted[1].write(wire({protocolVersion:'1.0.0',type:'revoke',streamId:initial.streamId,reason:'CONNECTION_REPLACED'}))
  await page.waitForFunction(()=>window.__FE16_TEST_ONLY__.transport.state==='closed')
  await page.waitForTimeout(50); assert.equal(accepted.length,2)
  await page.evaluate(()=>window.__FE16_TEST_ONLY__.connect())
  await page.evaluate(()=>{ const f=window.__FE16_TEST_ONLY__;f.session.replace(f.session.read().generation,null);f.transport.dispose() })
  await page.waitForFunction(()=>window.__FE16_TEST_ONLY__.transport.state==='disposed')
  assert(await page.evaluate(()=>[...Array(localStorage.length)].map((_,i)=>localStorage.key(i)).every(k=>!localStorage.getItem(k).includes('FE16-memory-only-token'))))
  assert.equal(await page.evaluate(()=>sessionStorage.length),0)
  assert(logs.every(l=>!l.includes('FE16-memory-only-token')&&!l.includes('A'.repeat(43))))
  assert(urls.every(u=>u==='/world-ws')); assert.deepEqual(errors,[])
  await page.waitForTimeout(50); assert.equal(sockets.size,0)
  await context.close()
  const nativeTraces=[]
  for(const name of ['movement-heartbeat','resync','room-switch','epoch-replacement','revoke']) {
    scriptTrace=JSON.parse(await readFile(new URL(`../../../../../contracts/fixtures/realtime/${name}.json`,import.meta.url),'utf8'))
    const traceContext=await browser.newContext(),tracePage=await traceContext.newPage()
    tracePage.on('pageerror',e=>errors.push(e.message))
    await tracePage.goto(`${base}/src/world/network/__tests__/fixtures/browser.html`)
    await tracePage.waitForFunction(()=>window.__FE16_TEST_ONLY__)
    // The common trace driver sends the exact canonical client operations; ping replies are automatic in this test harness.
    await tracePage.evaluate(()=>window.__FE16_TEST_ONLY__.connect())
    let serverCount=0
    for(const row of scriptTrace.messages.slice(1)) {
      if(row.direction==='server') {
        serverCount++
        await tracePage.waitForFunction(count=>window.__FE16_TEST_ONLY__.events.filter(e=>e.type==='frame').length>=count,serverCount)
      } else if(row.message.type!=='pong') {
        await tracePage.evaluate(async frame=>{
          const t=window.__FE16_TEST_ONLY__.transport
          if(frame.type==='input')t.sendInput({seq:frame.seq,axisX:frame.axisX,axisY:frame.axisY,buttons:frame.buttons})
          else if(frame.type==='resync')t.resync(frame.requestId,frame.reason)
          else if(frame.type==='switchRoom')await t.roomSwitch(frame.roomId,frame.requestId)
        },row.message)
      }
    }
    await tracePage.evaluate(()=>window.__FE16_TEST_ONLY__.transport.dispose())
    await traceContext.close();nativeTraces.push(name)
  }
  scriptTrace=undefined
  const denialContext=await browser.newContext(),denialPage=await denialContext.newPage()
  await denialPage.goto(`${base}/src/world/network/__tests__/fixtures/browser.html`)
  await denialPage.waitForFunction(()=>window.__FE16_TEST_ONLY__)
  await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.connect())
  const deniedPeer=accepted.at(-1), switchCount=received.filter(m=>m.type==='switchRoom').length
  await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.denyNextTicket())
  assert.equal(await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.transport.roomSwitch('00000000-0000-4000-8000-000000000012','00000000-0000-4000-8000-000000000051').then(()=>'unexpected-success',error=>error.kind)),'unavailable')
  assert.equal(await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.transport.state),'live')
  assert.equal(await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.urls.length),1)
  assert.equal(received.filter(m=>m.type==='switchRoom').length,switchCount)
  assert(sockets.has(deniedPeer))
  await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.transport.sendInput({seq:1,axisX:0,axisY:0,buttons:[]}))
  await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.transport.roomSwitch('00000000-0000-4000-8000-000000000012','00000000-0000-4000-8000-000000000052'))
  assert.equal(await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.count()),3)
  await denialPage.waitForFunction(()=>window.__FE16_TEST_ONLY__.transport.state==='closed')
  assert.equal(received.filter(m=>m.type==='switchRoom').length,switchCount+1)
  assert.equal(await denialPage.evaluate(()=>window.__FE16_TEST_ONLY__.events.filter(e=>e.type==='error').at(-1).code),4408)
  await denialContext.close()
  holdAdmission=true
  const timeoutContext=await browser.newContext(),timeoutPage=await timeoutContext.newPage()
  await timeoutPage.goto(`${base}/src/world/network/__tests__/fixtures/browser.html`)
  await timeoutPage.waitForFunction(()=>window.__FE16_TEST_ONLY__)
  assert.equal(await timeoutPage.evaluate(()=>window.__FE16_TEST_ONLY__.connect().then(()=>'unexpected-success',error=>error.kind)),'timeout')
  assert.equal(await timeoutPage.evaluate(()=>window.__FE16_TEST_ONLY__.transport.state),'closed')
  await timeoutContext.close();holdAdmission=false
  assert.deepEqual(errors,[])
  production=await preview({root,preview:{host:'127.0.0.1',port:await unusedPort(),strictPort:true}})
  const prodPage=await browser.newPage();let productSockets=0;prodPage.on('websocket',()=>productSockets++)
  await prodPage.goto(`http://127.0.0.1:${production.httpServer.address().port}`)
  assert.equal(await prodPage.evaluate(()=>window.__FE16_TEST_ONLY__),undefined);assert.equal(productSockets,0)
  await writeFile(`${evidence}/browser-results.json`,JSON.stringify({state:'MOCK_READY',browser:browser.version(),connections:accepted.length,nativeTraces,
    scenarios:['native open/authenticate/snapshot/input/ping-pong','detected gap freezes input/queued delta discard/explicit resync','native handshake timeout','target ticket denial preserves old socket/input; explicit fresh-ticket retry; post-send timeout terminal','unknown frame closes','fresh-ticket explicit reconnect','revoke disables reconnect','canonical session cancellation','owned socket cleanup','no credential URL/storage/log','production no test entry/mock/socket'],
    real_backend:'NOT_RUN',multiuser:'NOT_RUN',BE10_BE11_IN01:'NOT_RUN'},null,2))
  console.log(`PASS FE16 installed Chrome ${browser.version()}: native lifecycle and production exclusion`)
} finally {
  for(const socket of sockets)socket.destroy()
  await browser?.close(); await dev.close()
  await new Promise(resolve=>production?production.httpServer.close(resolve):resolve())
}
