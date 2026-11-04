import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile, mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { createServer } from 'vite'
const root=resolve(import.meta.dirname,'../../../..')
test('genuine selected C06 Bootstrap and six-player C05 snapshot/delta pass actual published admission without avatar rewrites',async()=>{
 const cache=await mkdtemp(resolve(tmpdir(),'mcr-whole-data-')),server=await createServer({root:resolve(root,'frontend'),configLoader:'runner',logLevel:'error',cacheDir:cache,server:{middlewareMode:true}})
 try{
  const data=await server.ssrLoadModule('/e2e/reference-integrated/whole/data.ts'),auth=await server.ssrLoadModule('/src/features/auth/model/bootstrap.ts'),me=await server.ssrLoadModule('/src/features/me/model/contract.ts')
  const fixture=await data.wholeData(async path=>JSON.parse(await readFile(resolve(root,'frontend/public',path),'utf8'))),manifest=JSON.parse(await readFile(resolve(root,'contracts/fixtures/assets/selected-static/avatar-manifest.json'),'utf8'))
  assert.deepEqual(fixture.bootstrap.avatar_manifest,manifest);assert.equal('provenance' in fixture.bootstrap.avatar_manifest,false)
  const scope={generation:1,userId:data.userId,eventId:data.eventId},checked=await auth.decodeBootstrap(fixture.bootstrap,scope)
  assert.deepEqual(checked.document.avatar_manifest,manifest);assert.equal(checked.document.avatar.config.contract_version,'1.1.0')
  assert.deepEqual(me.decodeParticipant(data.participant(fixture,fixture.avatar),scope).profile.avatar,fixture.avatar)
  const snapshot=data.frame(fixture,'00000000-0000-4000-8000-000000000003',fixture.avatar)
  assert.equal(snapshot.players.length,6);assert.equal(new Set(snapshot.players.map(p=>p.avatar.config.preset_id)).size,6)
  const saved={revision:8,config:data.presets[5]},delta=data.changedSelf(snapshot,saved)
  assert.equal(delta.upserts[0].userId,data.userId);assert.deepEqual(delta.upserts[0].avatar,saved);assert.equal(delta.streamSeq,1)
 }finally{await server.close();await rm(cache,{recursive:true,force:true})}
})
