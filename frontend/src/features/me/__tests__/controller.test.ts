import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest'
import { decodeParticipant } from '../model/contract'
import { eventId, participant, setup, userId } from './fixtures'
const response = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
const deferred = <T,>() => { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => vi.unstubAllGlobals())
describe('FE34 canonical own profile', () => {
  it('decodes frozen shape and rejects leaked/private or cross-identity fields', () => {
    expect(decodeParticipant(participant(), { userId, eventId, generation: 1 }).profile.bio).toBeNull()
    for (const p of [{ ...participant(), instagram: 'private' }, { ...participant(), user_id: eventId }, { ...participant(), profile: { ...participant().profile, user_id: eventId } }]) expect(() => decodeParticipant(p, { userId, eventId, generation: 1 })).toThrow()
  })
  it('saves only the selected section with exact revision and no token persistence or replay', async () => {
    const p = participant(), calls: {method:string; body:unknown; cache:RequestCache}[] = []
    const { controller, dispose } = setup(vi.fn(async input => {
      const r = input as Request, body = r.method === 'PATCH' ? await r.json() : null; calls.push({ method: r.method, body, cache: r.cache })
      if (r.method === 'PATCH') { p.revision++; p.profile.display_name = body.display_name }
      return response(p)
    }))
    await controller.load(); controller.editPreferences({ low_power: true }); controller.editProfile({ display_name: 'New name' }); await controller.save('profile')
    expect(calls).toEqual([{ method: 'GET', body: null, cache: 'no-store' }, { method: 'PATCH', body: { expected_revision: 1, display_name: 'New name', bio: null }, cache: 'no-store' }])
    expect(controller.read().participant?.revision).toBe(2); expect(controller.read().draft.preferences.low_power).toBe(true); expect(controller.dirty('preferences')).toBe(true)
    controller.cancel('preferences'); expect(controller.read().draft.preferences.low_power).toBe(false); dispose()
  })
  it('keeps conflict draft, never replays, and requires explicit reload', async () => {
    let writes = 0
    const { controller, dispose } = setup(vi.fn(async input => (input as Request).method === 'PATCH' ? (writes++, response({ code:'CONFLICT',message:'changed',details:{reason:'VERSION_CONFLICT',current_revision:2} },409)) : response(participant())))
    await controller.load(); controller.editProfile({ bio: 'My draft' }); await controller.save('profile'); await controller.save('profile')
    expect(controller.read().phase).toBe('conflict'); expect(controller.read().draft.bio).toBe('My draft'); expect(writes).toBe(1)
    await controller.load(); expect(controller.read().draft.bio).toBeNull(); expect(controller.read().blocked).toBe(false); dispose()
  })
  it('blocks replay after an uncertain network save, retaining the draft', async () => {
    const { controller, dispose } = setup(vi.fn(async input => { if ((input as Request).method === 'PATCH') throw new TypeError('network'); return response(participant()) }))
    await controller.load(); controller.editPreferences({ sound_enabled: true }); await controller.save('preferences')
    expect(controller.read().blocked).toBe(true); expect(controller.read().draft.preferences.sound_enabled).toBe(true); expect(controller.read().participant?.preferences.sound_enabled).toBe(false); dispose()
  })
  it('clears drafts immediately on policy change and fences an ignored-abort late read', async () => {
    const late = deferred<Response>(), fetcher = vi.fn(() => late.promise), { controller, auth, dispose } = setup(fetcher)
    const load = controller.load(); await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1))
    auth.privacy.changePolicy(auth.privacy.capture(), 'block'); expect(controller.read().participant).toBeNull(); expect(controller.read().draft.display_name).toBe('')
    late.resolve(response(participant())); await load; expect(controller.read().phase).toBe('expired'); expect(controller.read().participant).toBeNull(); dispose()
  })
  it('rejects oversized unicode names but accepts 40 astral characters and optional null bio', async () => {
    let writes = 0
    const { controller, dispose } = setup(vi.fn(async input => { const r=input as Request; if(r.method==='PATCH'){writes++; const body=await r.json(); return response({...participant(),revision:2,profile:{...participant().profile,display_name:body.display_name,bio:body.bio}})} return response(participant()) }))
    await controller.load(); controller.editProfile({ display_name: 'a'.repeat(41) }); await controller.save('profile'); expect(writes).toBe(0)
    controller.editProfile({ display_name: '🎵'.repeat(40) }); await controller.save('profile'); expect(writes).toBe(1); dispose()
  })
})

it('serializes saves and sends the exact preference-only BE07 subset', async () => {
  const pending=deferred<Response>(), calls:unknown[]=[], fetcher=vi.fn(async input=>{const r=input as Request;if(r.method==='GET')return response(participant());calls.push(await r.json());return pending.promise})
  const {controller,dispose}=setup(fetcher);await controller.load();controller.editPreferences({low_power:true,sound_enabled:true})
  const save=controller.save('preferences');await vi.waitFor(()=>expect(calls).toHaveLength(1));await controller.save('preferences');controller.cancel('preferences');controller.editPreferences({low_power:false})
  expect(calls).toEqual([{expected_revision:1,preferences:{theme:'REVENGE',low_power:true,reduced_motion:false,sound_enabled:true}}])
  pending.resolve(response({...participant(),revision:2,preferences:{...participant().preferences,low_power:true,sound_enabled:true}}));await save
  expect(controller.read().participant?.profile.avatar).toEqual(participant().profile.avatar);expect(controller.dirty('preferences')).toBe(false);dispose()
})
it.each(['read','write'] as const)('fences a late %s after canonical event/identity change',async kind=>{
  const pending=deferred<Response>(),fetcher=vi.fn(async input=>{const r=input as Request;return kind==='read'||r.method==='PATCH'?pending.promise:response(participant())})
  const {controller,auth,dispose}=setup(fetcher)
  if(kind==='write')await controller.load()
  controller.editProfile({bio:'private draft'});const flight=kind==='read'?controller.load():controller.save('profile')
  await vi.waitFor(()=>expect(fetcher).toHaveBeenCalledTimes(kind==='read'?1:2))
  auth.session.replace(auth.session.read().generation,{userId:eventId,eventId,accessToken:'DIFFERENT_SYNTHETIC'})
  expect(controller.read().draft.display_name).toBe('');expect(controller.read().participant).toBeNull()
  pending.resolve(response({...participant(),revision:2}));await flight;expect(controller.read().phase).toBe('expired');dispose()
})
it('rejects stale read and mutation revisions without presenting false save success',async()=>{
  let p=participant(),method='GET';const {controller,dispose}=setup(vi.fn(async input=>{method=(input as Request).method;return response(p)}))
  await controller.load();controller.editProfile({bio:'Draft'});await controller.save('profile');expect(method).toBe('PATCH');expect(controller.read().blocked).toBe(true);expect(controller.read().message).toContain('unknown')
  p={...participant(),revision:3};await controller.load();p=participant();await controller.load();expect(controller.read().phase).toBe('error');expect(controller.read().participant?.revision).toBe(3);dispose()
})
it('does not silently rebase drafts when FE13 notifies of a verified avatar commit',async()=>{
  const {controller,dispose}=setup(vi.fn(async()=>response(participant())));await controller.load();controller.editProfile({bio:'Keep this'})
  await controller.refreshAfterAvatar(controller.scope, 2);expect(controller.read().blocked).toBe(true);expect(controller.read().draft.bio).toBe('Keep this');dispose()
})
it('clears displayed own data on forbidden responses, without rendering server error contents',async()=>{
  let forbidden=false;const {controller,dispose}=setup(vi.fn(async()=>forbidden?response({code:'DENIED',message:'private detail',details:{reason:'PERMISSION_DENIED'}},403):response(participant())))
  await controller.load();forbidden=true;await controller.load();expect(controller.read().participant).toBeNull();expect(controller.read().phase).toBe('expired');expect(controller.read().message).not.toContain('private detail');dispose()
})
it('does not lose a verified avatar notification while a profile save is pending',async()=>{
  const pending=deferred<Response>(),fetcher=vi.fn(async input=>(input as Request).method==='GET'?response(participant()):pending.promise)
  const {controller,dispose}=setup(fetcher);await controller.load();controller.editProfile({bio:'Saving draft'});const save=controller.save('profile')
  await vi.waitFor(()=>expect(fetcher).toHaveBeenCalledTimes(2));await controller.refreshAfterAvatar(controller.scope, 2)
  pending.resolve(response({...participant(),revision:2}));await save;expect(controller.read().blocked).toBe(true);expect(controller.read().message).toContain('avatar changed');dispose()
})
it('ignores stale-scope and duplicate-avatar notifications',async()=>{
  const fetcher=vi.fn(async()=>response(participant())),{controller,dispose}=setup(fetcher);await controller.load();controller.editProfile({bio:'Keep this'})
  await controller.refreshAfterAvatar({...controller.scope,policyGeneration:controller.scope.policyGeneration+1},2)
  await controller.refreshAfterAvatar(controller.scope,1)
  expect(controller.read().blocked).toBe(false);expect(controller.read().draft.bio).toBe('Keep this');expect(fetcher).toHaveBeenCalledTimes(1);dispose()
})
it('rejects a stale avatar GET after a newer verified FE13 commit',async()=>{
  const {controller,dispose}=setup(vi.fn(async()=>response(participant())));await controller.load()
  await controller.refreshAfterAvatar(controller.scope,2)
  expect(controller.read().phase).toBe('error');expect(controller.read().blocked).toBe(true);expect(controller.read().message).toContain('Could not load');dispose()
})
