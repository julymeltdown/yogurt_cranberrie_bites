import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { isolatedCookiePolicy } from '../../../auth/__tests__/fixtures/data'
import { createSessionController } from '../../../auth/model/session'
import { createMediaUploadTransport } from '../../../../shared/media/transport'
import { createImageUploadController } from '../upload-controller'
import { event, file, image, intent, mediaId, ready, receipt, uid, user } from './fixtures'
import type { CommandBinding } from '../../../../shared/media/contract'
beforeEach(() => vi.stubGlobal('location', { origin:'https://mcr.example' }))
afterEach(() => vi.unstubAllGlobals())
function setup(handler: (r:Request) => Response|Promise<Response>) {
  const fetch = vi.fn<typeof globalThis.fetch>(async r => handler(r as Request))
  // Canonical FE09 controller is the only token/session owner.
  const auth = createSessionController({eventId:event,fetch,cookiePolicy:isolatedCookiePolicy()})
  auth.session.replace(0,{userId:user,eventId:event,accessToken:'memory-token'})
  const transport=createMediaUploadTransport(auth.request)
  return {auth,transport,fetch}
}
it('runs exact C04 JSON/binary/finalize through canonical SessionController.request',async()=>{
  const calls: string[]=[]
  const h=setup(async r=>{
    const path=new URL(r.url).pathname;calls.push(path)
    expect(r.headers.get('Authorization')).toBe('Bearer memory-token');expect(r.headers.get('X-MCR-Intent')).toBe('browser')
    expect(r.headers.has('Idempotency-Key')).toBe(true);expect(r.cache).toBe('no-store');expect(r.credentials).toBe('same-origin')
    if(path.endsWith('/uploads')) {expect(await r.json()).toEqual({purpose:'POST',content_type:'image/png',byte_length:image().blob.size,sha256:'a'.repeat(64)});return Response.json(intent())}
    if(path.endsWith('/bytes')) {expect(r.headers.get('Content-Type')).toBe('image/png');expect(new Uint8Array(await r.arrayBuffer())).toEqual(new Uint8Array(await image().blob.arrayBuffer()));return Response.json({media_id:mediaId,state:'UPLOADED',revision:2})}
    expect(await r.json()).toEqual({expected_revision:2});return Response.json(ready())
  })
  let n=10
  const controller=createImageUploadController({session:h.auth.session,transport:h.transport,purpose:'POST',prepare:async()=>image(),previews:{create:()=> 'blob:test',revoke:()=>{}},key:()=>uid(n++)})
  await controller.select(file());await controller.upload();expect(controller.read().phase).toBe('ready');expect(calls).toHaveLength(3)
  expect(new Set(h.fetch.mock.calls.map(([r])=>(r as Request).headers.get('Idempotency-Key'))).size).toBe(3)
  controller.dispose();h.auth.dispose()
})
it('does not treat 202 receipt as intent; scopes receipt status and READY reread',async()=>{
  const b:CommandBinding={scope:{generation:1,userId:user,eventId:event},key:uid(10),operation:'initConcertUpload',mediaId:null}
  const h=setup(r=> new URL(r.url).pathname.endsWith('/uploads')?Response.json(receipt(b),{status:202}):Response.json(receipt(b,{state:'SUCCEEDED',resource_id:mediaId,resource_revision:3,result_status:200})))
  expect(await h.transport.initialize(b,'POST',image(),new AbortController().signal)).toMatchObject({kind:'pending'})
  expect(await h.transport.status(b,new AbortController().signal)).toMatchObject({state:'SUCCEEDED'})
  const request=h.fetch.mock.calls.at(-1)![0] as Request
  expect(new URL(request.url).searchParams.get('operation_id')).toBe('initConcertUpload');expect(request.method).toBe('GET');expect(request.headers.has('X-MCR-Intent')).toBe(false)
  h.auth.dispose()
})
it.each(['foreign-path','external-path','expired','extra-field','wrong-status','wrong-media','wrong-purpose'] as const)('rejects %s responses through canonical decoder',async mode=>{
  const b:CommandBinding={scope:{generation:1,userId:user,eventId:event},key:uid(10),operation:mode==='wrong-media'||mode==='wrong-purpose'?'finalizeConcertUpload':'initConcertUpload',mediaId:mediaId}
  const value= mode==='wrong-media'?{...ready(),media_id:uid(99)}:mode==='wrong-purpose'?{...ready(),purpose:'REPORT'}:{...intent(),...(mode==='foreign-path'?{upload_path:`/api/concert/events/${uid(99)}/media/${mediaId}/bytes`}:mode==='external-path'?{upload_path:'https://evil.invalid/bytes'}:mode==='expired'?{expires_at:'2020-01-01T00:00:00Z'}:mode==='extra-field'?{authorization:'do-not-forward'}:{})}
  const h=setup(()=>Response.json(value,{status:mode==='wrong-status'?201:200}))
  await expect(b.operation==='finalizeConcertUpload'?h.transport.finalize(b,'POST',2,new AbortController().signal):h.transport.initialize(b,'POST',image(),new AbortController().signal)).rejects.toMatchObject({kind:'invalid-response'})
  expect(h.fetch).toHaveBeenCalledTimes(1);h.auth.dispose()
})
