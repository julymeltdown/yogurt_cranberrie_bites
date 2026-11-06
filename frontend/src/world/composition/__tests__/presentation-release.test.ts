// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { waitFor } from '@testing-library/react'
import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../features/auth/__tests__/fixtures/data'
import { withAdmission } from '../../../features/auth/__tests__/fixtures/admission'
import { createWorldComposition } from '../index'
import { currentScope, eventId, project, resolverFor, testData } from './fixtures/data'
import type { WorldCamera } from '../../camera/port'
import type { EngineFactory } from '../../runtime'
vi.mock('../../map/resources', async original => {
 const m = await original<typeof import('../../map/resources')>()
 return {...m, loadMap: (input: Parameters<typeof m.loadMap>[0], signal: AbortSignal) => m.loadMap({...input,decode:async () => {
  const image = new Image(); Object.defineProperties(image,{naturalWidth:{value:96},naturalHeight:{value:32}});return {image,dispose(){}}
 }},signal)}
})
const cleanup: (() => void)[] = []
afterEach(() => {cleanup.splice(0).reverse().forEach(stop=>stop());document.body.replaceChildren()})
async function setup() {
 const data = await testData(), auth = createSessionController({eventId,cookiePolicy:isolatedCookiePolicy(),fetch:withAdmission(async()=>new Response(null,{status:503}))})
 auth.session.replace(0,{userId:data.bootstrap.participant.user_id,eventId,accessToken:'test-memory'})
 cleanup.push(()=>auth.dispose())
 const host=document.createElement('div');document.body.append(host)
 const order:string[]=[];let withdrawn=false
 const engine:EngineFactory=element=>{
  const canvas=document.createElement('canvas');element.append(canvas)
  const camera:WorldCamera={surface:()=>canvas,center(){},project:p=>p,unproject:p=>p,read:()=>({zoom:1,scrollX:0,scrollY:0,width:390,height:400})}
  return {ready:Promise.resolve(),closed:Promise.resolve(),camera,resize(){},setVisible(){},replaceSkin(){},destroy(){order.push('destroy');withdrawn=composition.presentation.read()===null;canvas.remove()}}
 }
 const writes:string[]=[]
 const composition=createWorldComposition({auth,resolveRelease:resolverFor(auth,data),importEngine:async()=>engine,themePreferences:{read:()=>null,write:(_scope,theme)=>{writes.push(theme)}}})
 cleanup.push(()=>composition.dispose())
 const lifetime=new AbortController(),context={scope:currentScope(auth),privacyScope:auth.privacy.capture(),bootstrap:await project(auth,data),signal:lifetime.signal}
 const lease=composition.mount(host,context)
 await waitFor(()=>expect(composition.presentation.read()).not.toBeNull())
 const presentation=composition.presentation.read()!
 presentation.request.signal.addEventListener('abort',()=>{order.push('abort');expect(composition.presentation.read()).toBeNull()})
 return {auth,data,composition,presentation,lifetime,context,lease,order,writes,withdrawn:()=>withdrawn}
}
it('publishes exact original scope/bootstrap/map pins and the incumbent camera attachment',async()=>{
 const h=await setup(),p=h.presentation
 expect(p.request.privacyScope).toEqual(h.context.privacyScope);expect(p.request.bootstrap).toEqual(h.context.bootstrap)
 expect(p.map.manifest).toEqual(h.data.map);expect(p.map.expected).toMatchObject({contentHash:h.data.map.content_hash,geometryHash:h.data.map.geometry_hash})
 expect(h.composition.presentation.read()).toBe(p);expect(p.camera.read().camera).toBeTruthy()
 await p.theme.select('REVENGE');expect(h.writes).toEqual(['REVENGE'])
 await p.theme.select('PARADE');expect(p.theme.read()).toMatchObject({selected:'REVENGE',phase:'error'})
 expect(h.writes).toEqual(['REVENGE']);expect(p.map.manifest.geometry).toEqual(h.data.map.geometry)
})
it.each(['abort','policy','dispose'] as const)('withdraws synchronously before engine cleanup on %s and never restamps a release',async kind=>{
 const h=await setup(),old=h.presentation
 if(kind==='abort')h.lifetime.abort()
 if(kind==='policy')h.auth.privacy.changePolicy(h.context.privacyScope,'block')
 if(kind==='dispose')h.lease.dispose()
 expect(h.composition.presentation.read()).toBeNull();expect(old.request.signal.aborted).toBe(true)
 expect(h.order).toEqual(['abort','destroy']);expect(h.withdrawn()).toBe(true);expect(old.theme.read().phase).toBe('disposed')
 const writes=h.writes.length;await old.theme.select('REVENGE');expect(h.writes).toHaveLength(writes)
 expect(old.request.privacyScope).toEqual(h.context.privacyScope);await h.composition.settled()
})
