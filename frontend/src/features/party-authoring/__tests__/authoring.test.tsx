// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { PartyAuthoring } from '../ui/PartyAuthoring'
import { partyDraftSchema, contractValue } from '../model/contract'
import { draft, harness, ids, party, profile, errorResponse, pendingReceipt } from './fixtures/harness'
afterEach(cleanup)
describe('FE22 contract-backed authoring',()=>{
  it('validates capacity, Unicode limits, real UTC date and window',()=>{
    for(const patch of [{capacity:1},{capacity:51},{capacity:2.5},{title:' '},{title:'🙂'.repeat(121)},{meeting_starts_at:'2026-02-30T08:00:00Z'},{meeting_ends_at:draft().meeting_starts_at}]) expect(partyDraftSchema.safeParse({...draft(),...patch}).success).toBe(false)
    expect(partyDraftSchema.safeParse({...draft(),title:'🙂'.repeat(120)}).success).toBe(true)
    const {state:_,...fields}=draft();expect(()=>contractValue('Domain_party_CreateParty',{...fields,ticket_section:null})).not.toThrow()
  })
  it('creates once, uses exact registry payload, blocks duplicate clicks',async()=>{
    const h=harness();await Promise.all([h.controller.submit(draft()),h.controller.submit(draft())]);expect(h.requests).toHaveLength(1)
    expect(h.controller.read().phase).toBe('succeeded');expect(h.requests[0]!.headers.get('Idempotency-Key')).toBeTruthy()
    expect(await h.requests[0]!.json()).toEqual({...draft(),state:undefined});h.dispose()
  })
  it('keeps original key after unknown write, 404 status and repeat submit',async()=>{
    const h=harness(r=>r.method==='POST'?Promise.reject(new Error('lost')):errorResponse(404,'RESOURCE_UNAVAILABLE'))
    await h.controller.submit(draft());expect(h.controller.read().phase).toBe('uncertain');const key=h.requests[0]!.headers.get('Idempotency-Key')
    await h.controller.checkStatus();await h.controller.submit({...draft(),title:'Different intent'});expect(h.requests).toHaveLength(2)
    expect(new URL(h.requests[1]!.url).searchParams.get('idempotency_key')).toBe(key);expect(h.controller.read().phase).toBe('uncertain');h.dispose()
  })
  it('accepts only contextual pending receipt and rereads successful result',async()=>{
    let key=''; const h=harness(r=>{const u=new URL(r.url); if(r.method==='POST'){key=r.headers.get('Idempotency-Key')!;return Response.json(pendingReceipt(key,{operation_id:'createConcertParty'}),{status:202})}
      if(u.pathname.endsWith('/commands'))return Response.json(pendingReceipt(key,{operation_id:'createConcertParty',state:'SUCCEEDED',resource_id:ids.party,resource_revision:3,result_status:200}))
      return Response.json(party({owner:profile(ids.actor)}))})
    await h.controller.submit(draft());expect(h.controller.read().phase).toBe('pending');await h.controller.checkStatus();expect(h.controller.read().phase).toBe('succeeded');expect(h.requests.map(r=>r.method)).toEqual(['POST','GET','GET']);h.dispose()
  })
  it('preserves conflict draft, explicitly rebases, then sends current revision',async()=>{
    let revision=3, writes=0
    const h=harness(r=>{if(r.method==='PATCH'){writes++;if(writes===1){revision=4;return errorResponse(409,'VERSION_CONFLICT')}return Response.json(party({owner:profile(ids.actor),revision:5}))}return Response.json(party({owner:profile(ids.actor),revision}))},true)
    await h.controller.loadLatest();await h.controller.submit(draft());expect(h.controller.read().phase).toBe('conflict')
    await h.controller.loadLatest();expect(h.controller.read().draft.title).toBe('Encore friends');expect(h.controller.read().latest?.revision).toBe(4)
    await h.controller.submit(draft());expect(writes).toBe(1);h.controller.keepDraftWithLatest();await h.controller.submit(draft());expect((await h.requests.at(-1)!.json()).expected_revision).toBe(4);h.dispose()
  })
  it('rejects non-owner edit, private projection and wrong actor receipt',async()=>{
    const denied=harness(()=>Response.json(party()),true);await denied.controller.loadLatest();expect(denied.controller.read().phase).toBe('denied');await denied.controller.submit(draft());expect(denied.requests).toHaveLength(1);denied.dispose()
    for(const value of [{...party({owner:profile(ids.actor)}),private_note:'secret'},pendingReceipt(ids.key,{operation_id:'createConcertParty',actor_id:ids.other})]){
      const h=harness(()=>Response.json(value,{status:'receipt_id' in value?202:200}));await h.controller.submit(draft());expect(h.controller.read().phase).toBe('uncertain');h.dispose()
    }
  })
  it('evicts drafts and ignores late response on policy change and identity replacement',async()=>{
    for(const identity of [false,true]){let resolve!:(r:Response)=>void;const h=harness(()=>new Promise(r=>{resolve=r}));h.controller.edit(draft());h.controller.saveDraft();const pending=h.controller.submit(draft());await waitFor(()=>expect(h.requests).toHaveLength(1))
      if(identity)h.auth.session.replace(h.auth.session.read().generation,{userId:ids.other,eventId:ids.event,accessToken:'next-token'});else h.auth.privacy.changePolicy(h.auth.privacy.capture(),'consent-revoked')
      expect(h.controller.read().saved).toBeNull();expect(h.controller.read().draft.title).toBe('');resolve(Response.json(party({owner:profile(ids.actor)})));await pending;expect(h.controller.read().party).toBeNull();h.dispose()}
  })
  it('shows RHF validation, explicit draft restore and clears rendered inputs on logout',async()=>{
    const h=harness();render(<PartyAuthoring controller={h.controller}/>);fireEvent.click(screen.getByRole('button',{name:'Create party'}));await screen.findAllByRole('alert');expect(h.requests).toHaveLength(0)
    fireEvent.change(screen.getByRole('textbox',{name:'Party title'}),{target:{value:'My private draft'}});fireEvent.click(screen.getByRole('button',{name:'Save session draft'}));fireEvent.change(screen.getByRole('textbox',{name:'Party title'}),{target:{value:'Replacement'}});fireEvent.click(screen.getByRole('button',{name:'Restore draft'}));expect((screen.getByRole('textbox',{name:'Party title'}) as HTMLInputElement).value).toBe('My private draft')
    act(()=>{h.auth.session.replace(h.auth.session.read().generation,null)});expect(screen.queryByRole('textbox',{name:'Party title'})).toBeNull();expect(screen.queryByDisplayValue('My private draft')).toBeNull();h.dispose()
  })
})
