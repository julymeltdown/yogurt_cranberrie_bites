// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createAccountOwner } from '../owner'
import { thumbnailSource, type AccountAssetsPort } from '../assets'
import { createReviewedAvatarPreview } from '../../../features/avatar'
import { eventId, userId, participant, setup } from '../../../features/me/__tests__/fixtures'
import { bootstrapFor } from '../../__tests__/fixtures/ports'
import manifest from '../../../../../contracts/fixtures/assets/avatar-manifest.json'
vi.mock('../../../features/avatar/preview',()=>({createReviewedAvatarPreview:vi.fn()}))
afterEach(()=>vi.resetAllMocks())
const assets={binding:{bakedSkinToneId:'tone_fixture'},resources:{}} as AccountAssetsPort
function fixture() {
  const p=participant(),fetcher=vi.fn(async input=>{const request=input as Request
    if(request.method==='PUT'){const body=await request.json();p.profile.avatar={revision:p.profile.avatar.revision+1,config:body.config};return Response.json(p.profile.avatar)}
    return Response.json(p)
  })
  const f=setup(fetcher),scope=f.auth.privacy.capture(),bootstrap={...bootstrapFor({userId,eventId,generation:scope.generation}),document:{avatar_manifest:manifest}}
  const disposed:ReturnType<typeof vi.fn>[]=[],rendered:HTMLCanvasElement[]=[]
  vi.mocked(createReviewedAvatarPreview).mockImplementation(async()=>{const dispose=vi.fn();disposed.push(dispose);return{
    catalog:{presets:[{id:'fixture',config:p.profile.avatar.config}],items:[],tones:['tone_fixture']},supports:()=>true,
    async render(){const canvas=document.createElement('canvas');rendered.push(canvas);return{canvas,dispose(){canvas.width=canvas.height=0}}},dispose,
  }})
  return{...f,p,fetcher,scope,bootstrap,disposed,rendered}
}
it('retains the exact authorized manifest, captured lease, binding and resources with no default inputs',()=>{
  const f=fixture(),source=thumbnailSource(f.bootstrap,f.scope,assets)
  expect(source?.scope).toBe(f.scope);expect(source?.options.document).toBe(manifest);expect(source?.options.binding).toBe(assets.binding);expect(source?.options.resources).toBe(assets.resources)
  expect(thumbnailSource(f.bootstrap,f.scope)).toBeUndefined();f.dispose()
})
it('does not request anything until a panel starts and reports missing art without blocking ME',async()=>{
  const f=fixture(),owner=createAccountOwner({auth:f.auth,bootstrap:f.bootstrap,scope:f.scope})
  expect(f.fetcher).not.toHaveBeenCalled();owner.avatar.start();await vi.waitFor(()=>expect(owner.avatar.read().notice).toBe('preview-unavailable'))
  expect(createReviewedAvatarPreview).not.toHaveBeenCalled();await owner.me.load();expect(owner.me.read().participant?.user_id).toBe(userId)
  owner.dispose();f.dispose()
})
it('uses actual FE13 verified PUT callback to refresh ME from the canonical GET, without a second bootstrap',async()=>{
  const f=fixture(),owner=createAccountOwner({auth:f.auth,bootstrap:f.bootstrap,scope:f.scope,assets})
  await owner.me.load();owner.avatar.start();await vi.waitFor(()=>expect(owner.avatar.read().phase).toBe('ready'))
  expect(owner.avatar.selectItem('accessory',null)).toBe(true);expect(await owner.avatar.save()).toBe(true)
  await vi.waitFor(()=>expect(owner.me.read().participant?.profile.avatar.revision).toBe(2))
  const config=owner.me.read().participant?.profile.avatar.config
  expect(config?.contract_version).toBe('1.0.0')
  if(config?.contract_version!=='1.0.0')throw new Error('Expected the retained layered fixture')
  expect(config.item_ids.accessory).toBeNull()
  expect(f.fetcher.mock.calls.every(([input])=>!(input as Request).url.includes('/bootstrap'))).toBe(true)
  expect(createReviewedAvatarPreview).toHaveBeenCalledWith(expect.objectContaining({document:manifest,pin:expect.objectContaining({content_hash:manifest.content_hash}),binding:assets.binding}))
  owner.dispose();f.dispose()
})
it('keeps a dirty ME draft when verified avatar state advances',async()=>{
  const f=fixture(),owner=createAccountOwner({auth:f.auth,bootstrap:f.bootstrap,scope:f.scope,assets})
  await owner.me.load();owner.me.editProfile({bio:'Keep this draft'});owner.avatar.start();await vi.waitFor(()=>expect(owner.avatar.read().phase).toBe('ready'))
  owner.avatar.selectItem('accessory',null);await owner.avatar.save()
  expect(owner.me.read().draft.bio).toBe('Keep this draft');expect(owner.me.read().blocked).toBe(true)
  owner.dispose();f.dispose()
})
it('synchronously disposes both controllers and preview resources on policy invalidation',async()=>{
  const f=fixture(),owner=createAccountOwner({auth:f.auth,bootstrap:f.bootstrap,scope:f.scope,assets})
  await owner.me.load();owner.avatar.start();await vi.waitFor(()=>expect(owner.avatar.read().preview.phase).toBe('ready'))
  f.auth.privacy.changePolicy(f.scope,'block')
  expect(owner.isDisposed()).toBe(true);expect(owner.me.read().participant).toBeNull();expect(owner.avatar.read().persisted).toBeNull()
  expect(f.disposed.every(d=>d.mock.calls.length>0)).toBe(true);expect(f.rendered.every(c=>c.width===0)).toBe(true);f.dispose()
})
it('rejects stale lease or mismatched bootstrap identity before allocating owners',()=>{
  const f=fixture();expect(()=>createAccountOwner({auth:f.auth,bootstrap:{...f.bootstrap,participant:{userId:eventId,eventId}},scope:f.scope})).toThrow()
  f.auth.privacy.changePolicy(f.scope,'consent-revoked');expect(()=>createAccountOwner({auth:f.auth,bootstrap:f.bootstrap,scope:f.scope})).toThrow()
  expect(f.fetcher).not.toHaveBeenCalled();f.dispose()
})
it('disposes an ignored-abort late preview and never reads participant after owner teardown',async()=>{
  const f=fixture(),owner=createAccountOwner({auth:f.auth,bootstrap:f.bootstrap,scope:f.scope,assets})
  let resolve!:(value:Awaited<ReturnType<typeof createReviewedAvatarPreview>>)=>void
  vi.mocked(createReviewedAvatarPreview).mockImplementationOnce(()=>new Promise(r=>{resolve=r}))
  owner.avatar.start();await vi.waitFor(()=>expect(createReviewedAvatarPreview).toHaveBeenCalledTimes(1))
  owner.dispose();const dispose=vi.fn();resolve({catalog:{presets:[],items:[],tones:[]},supports:()=>false,render:vi.fn(),dispose})
  await vi.waitFor(()=>expect(dispose).toHaveBeenCalledTimes(1));expect(f.fetcher).not.toHaveBeenCalled();expect(owner.avatar.read().phase).toBe('disposed');f.dispose()
})
