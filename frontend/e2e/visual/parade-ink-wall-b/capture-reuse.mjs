// Retained captures are labeled and hashed; the current browser still executes its checks.
import assert from 'node:assert/strict'
import {readFile} from 'node:fs/promises'
import {createHash} from 'node:crypto'
import {dirname,resolve,relative} from 'node:path'
export async function reuseCapture(name,output,source){
 assert(source.endsWith('/pi09-wallb-remainder-native14-0145/commands/report.json'))
 if(name==='wall-parade-ink-390-sync-degraded.png'){
  const folder=resolve(dirname(source),'../../wall-native36-0343'),closed=JSON.parse(await readFile(resolve(folder,'FULL.json'),'utf8'))
  assert(closed.FULL&&closed.temporaryAbsent&&closed.remainingOwned.length===0&&closed.status==='FAIL')
  assert.equal(Object.entries(closed.config.pins).find(([path])=>path.endsWith('/parade-ink-wall-b/app.mjs'))?.[1],'b0ccabf019c52af20d310ead53854a5883a8535efcbef519874cd6bd7d5d00c5')
  const path=resolve(folder,'commands',name),bytes=await readFile(path),sha256='e081a9d5e9c55afbec96b278427774cbf3119e1ab822d4cc41a9af36a7dbbd43'
  assert.equal(createHash('sha256').update(bytes).digest('hex'),sha256)
  return {path:relative(output,path),sha256,bytes:bytes.length,qualification:'RETAINED_NATIVE36_CAPTURE_NOT_NEW_SCREENSHOT',sourceCapture:path}
 }
 if(name==='wall-parade-ink-390-unavailable.png'){
  const path=resolve(dirname(source),'../../pi09-wallb-remainder-native18-0202/commands',name),bytes=await readFile(path)
  const sha256='8dd9f189f9a0134cf931a2bce4498edc6f929b73ffe4f42978ebfc42437ed705'
  assert.equal(createHash('sha256').update(bytes).digest('hex'),sha256)
  return {path:relative(output,path),sha256,bytes:bytes.length,qualification:'RETAINED_NATIVE18_CAPTURE_NOT_NEW_SCREENSHOT',sourceCapture:path}
 }
 const report=JSON.parse(await readFile(source,'utf8')),c=report.cases[0]
 assert.equal(c.width,390)
 const capture=['screenshot','reducedScreenshot','commandPreview','unknownScreenshot'].map(key=>c[key]).find(row=>row?.path===name)
 if(!capture)return null
 const path=resolve(dirname(source),capture.path),bytes=await readFile(path)
 assert.equal(createHash('sha256').update(bytes).digest('hex'),capture.sha256)
 return {...capture,path:relative(output,path),qualification:'RETAINED_NATIVE14_CAPTURE_NOT_NEW_SCREENSHOT',sourceReport:source}
}
