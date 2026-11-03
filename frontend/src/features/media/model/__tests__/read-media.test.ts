import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemorySession, createHttpClient } from '../../../../shared/api/http'
import { createMediaPreviewController } from '../../../../shared/media/read-media'
import { bytes, deferred, event, mediaId, ready, user } from './fixtures'
beforeEach(()=>vi.stubGlobal('location',{origin:'https://mcr.example'}))
afterEach(()=>vi.unstubAllGlobals())
function setup(handler:(r:Request)=>Response|Promise<Response> = r=>new URL(r.url).pathname.endsWith('/content')?new Response(bytes(),{headers:{'Content-Type':'image/png'}}):Response.json(ready())) {
  const session=createMemorySession();session.replace(0,{userId:user,eventId:event,accessToken:'memory'})
  const fetch=vi.fn<typeof globalThis.fetch>(async r=>handler(r as Request)),http=createHttpClient({session,fetch})
  const previews={create:vi.fn(()=> 'blob:private-fixture'),revoke:vi.fn()}
  const controller=createMediaPreviewController({session,request:http.request,mediaId,purpose:'POST',previews})
  return {session,fetch,previews,controller}
}
it('loads protected metadata and bounded bytes through canonical HTTP, owns one ephemeral preview',async()=>{
  const h=setup();await h.controller.load()
  expect(h.controller.read()).toMatchObject({phase:'ready',scope:{userId:user,eventId:event},previewUrl:'blob:private-fixture',media:ready()})
  expect(h.fetch).toHaveBeenCalledTimes(2)
  for(const [r] of h.fetch.mock.calls){expect((r as Request).headers.get('Authorization')).toBe('Bearer memory');expect((r as Request).cache).toBe('no-store')}
  await h.controller.load();expect(h.fetch).toHaveBeenCalledTimes(2)
  h.controller.dispose();expect(h.previews.revoke).toHaveBeenCalledTimes(1);expect(h.controller.read().previewUrl).toBeNull()
})
it.each(['metadata','bytes'] as const)('identity abort during %s clears state and fences late private bytes',async stage=>{
  const blocked=deferred<Response>()
  const h=setup(r=>stage==='metadata'||new URL(r.url).pathname.endsWith('/content')?blocked.promise:Response.json(ready()))
  const pending=h.controller.load();await vi.waitFor(()=>expect(h.fetch).toHaveBeenCalledTimes(stage==='metadata'?1:2))
  h.session.replace(1,null);await pending;blocked.resolve(Response.json(ready()));await Promise.resolve()
  expect(h.controller.read()).toMatchObject({phase:'cancelled',scope:null,previewUrl:null,media:null});expect(h.previews.create).not.toHaveBeenCalled()
})
it.each(['type','dimension','denied'] as const)('does not create a URL for %s byte response',async mode=>{
  const h=setup(r=>new URL(r.url).pathname.endsWith('/content')?mode==='denied'?Response.json({code:'DENIED',message:'Safe',details:{reason:'PERMISSION_DENIED'}},{status:403}):new Response(bytes(),{headers:{'Content-Type':mode==='type'?'text/html':'image/png'}}):Response.json({...ready(),...(mode==='dimension'?{width:2}:{})}))
  await h.controller.load();expect(h.controller.read().phase).toBe('failed');expect(h.previews.create).not.toHaveBeenCalled()
})
it('policy cancellation revokes an already visible private URL synchronously',async()=>{
  const h=setup();await h.controller.load();h.controller.cancel()
  expect(h.previews.revoke).toHaveBeenCalledTimes(1);expect(h.controller.read().media).toBeNull();expect(h.controller.read().previewUrl).toBeNull()
})
