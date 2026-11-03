// @vitest-environment jsdom
import {afterEach,expect,it} from 'vitest'
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react'
import {GiveawayAuthoring} from '../ui/GiveawayAuthoring'
import {giftDraftSchema,type QualifiedImagePort} from '../model/contract'
import {harness,draft,management,event,actor,key,place} from './harness'
import {errorResponse,pendingReceipt} from '../../party-discovery/__tests__/fixtures/data'
afterEach(cleanup)
it('validates quantity/window and creates text-only DRAFT with no fake image',async()=>{
 expect(giftDraftSchema.safeParse({...draft(),quantity_total:0}).success).toBe(false);expect(giftDraftSchema.safeParse({...draft(),pickup_ends_at:draft().pickup_starts_at}).success).toBe(false)
 const h=harness();await h.controller.submit(draft());expect(h.controller.read().phase).toBe('succeeded');const body=await h.requests[0]!.json();expect(body.image_media_id).toBeNull();expect(body.state).toBeUndefined();expect(h.requests[0]!.headers.get('Idempotency-Key')).toBe(key);h.dispose()
})
it('unknown create remains locked across altered form and missing status',async()=>{
 const h=harness(r=>r.method==='POST'?Promise.reject(new Error('lost')):errorResponse(404,'RESOURCE_UNAVAILABLE'));await h.controller.submit(draft());await h.controller.submit({...draft(),title:'Duplicate intent'});await h.controller.checkStatus();expect(h.requests.filter(r=>r.method==='POST')).toHaveLength(1);expect(h.controller.read().phase).toBe('uncertain');h.dispose()
})
it('stock conflict preserves form and explicit rebase uses latest revision',async()=>{
 let revision=1,writes=0;const h=harness(r=>{const m=management();m.giveaway={...m.giveaway,revision};if(r.method==='PATCH'){writes++;if(writes===1){revision=2;return errorResponse(409,'VERSION_CONFLICT')}return Response.json({...m.giveaway,revision:3})}return Response.json(m)},true)
 await h.controller.loadLatest();const form={...h.controller.read().draft,title:'Preserve my edit'};await h.controller.submit(form);expect(h.controller.read().phase).toBe('conflict');await h.controller.loadLatest();expect(h.controller.read().draft.title).toBe(form.title);h.controller.keepDraftWithLatest();await h.controller.submit(form);expect((await h.requests.filter(r=>r.method==='PATCH').at(-1)!.json()).expected_revision).toBe(2);h.dispose()
})
it('cannot undercut reservations/claims or move outstanding pickup window',async()=>{
 const h=harness(undefined,true);await h.controller.loadLatest();const form=h.controller.read().draft;await h.controller.submit({...form,quantity_total:1});await h.controller.submit({...form,pickup_ends_at:'2026-11-07T11:00:00Z'});expect(h.requests).toHaveLength(1);h.dispose()
})
it('wrong owner, stock mismatch, foreign reservation or leaked pickup code fails closed',async()=>{
 for(const change of ['owner','stock','foreign','leak']){const m=management();if(change==='owner')m.giveaway={...m.giveaway,owner:{...m.giveaway.owner,user_id:m.reservations[0]!.recipient_id}};if(change==='stock')m.quantity_claimed=2;if(change==='foreign')m.reservations=[{...m.reservations[0]!,event_id:actor}];if(change==='leak')Object.assign(m.reservations[0]!,{confirmation_code:'123456'})
 const h=harness(()=>Response.json(m),true);await h.controller.loadLatest();expect(h.controller.read().management).toBeNull();expect(['denied','failed']).toContain(h.controller.read().phase);h.dispose()}
})
it('only current scoped READY GIVEAWAY media attaches; stale and failed upload preserves form',async()=>{
 let value:ReturnType<QualifiedImagePort['read']>={phase:'failed',scope:null,attachment:null,previewUrl:null,error:'fixture failure'},cancelled=0
 const image:QualifiedImagePort={read:()=>value,subscribe:()=>()=>{},select:async()=>{},upload:async()=>{},recover:async()=>{},cancel(){cancelled++}}
 const h=harness(undefined,false,image);h.controller.edit(draft());await h.controller.selectImage(new File(['x'],'test.png',{type:'image/png'}));await h.controller.submit(draft());expect(h.requests).toHaveLength(0);expect(h.controller.read().draft.title).toBe(draft().title)
 value={phase:'ready',scope:h.auth.session.read(),previewUrl:null,error:null,attachment:{media_id:place,state:'READY',revision:1,width:10,height:10,content_type:'image/png',purpose:'GIVEAWAY',read_path:`/api/concert/events/${event}/media/${place}/content`}}
 await h.controller.submit(draft());expect((await h.requests[0]!.json()).image_media_id).toBe(place);h.auth.privacy.changePolicy(h.auth.privacy.capture(),'consent-revoked');expect(cancelled).toBeGreaterThan(0);expect(h.controller.read().draft.title).toBe('');h.dispose()
})
it('successful pending receipt still needs current authorized stock read',async()=>{
 const h=harness(r=>r.method==='POST'?Response.json(pendingReceipt(key,{actor_id:actor,event_id:event,operation_id:'createConcertGiveaway'}),{status:202}):r.url.includes('/commands')?Response.json(pendingReceipt(key,{actor_id:actor,event_id:event,operation_id:'createConcertGiveaway',state:'SUCCEEDED',resource_id:management().giveaway.giveaway_id,resource_revision:1,result_status:200})):Response.json(management()))
 await h.controller.submit(draft());expect(h.controller.read().phase).toBe('pending');await h.controller.checkStatus();expect(h.controller.read().phase).toBe('succeeded');expect(h.requests.map(r=>r.method)).toEqual(['POST','GET','GET']);h.dispose()
})
it('owner route UI shows unavailable upload and clears form/inventory after logout',async()=>{
 const h=harness(undefined,true);render(<GiveawayAuthoring controller={h.controller} editing/>);await screen.findByRole('table');await waitFor(()=>expect((screen.getByRole('textbox',{name:'Item title'}) as HTMLInputElement).value).toBe('Fan badges'));expect(screen.getByText(/Image upload is unavailable/)).toBeTruthy();await waitFor(()=>expect(h.controller.read().phase).toBe('idle'));fireEvent.change(screen.getByRole('textbox',{name:'Item title'}),{target:{value:'Private gift draft'}});fireEvent.click(screen.getByRole('button',{name:'Save session draft'}))
 act(()=>{h.auth.session.replace(h.auth.session.read().generation,null)});expect(screen.queryByRole('table')).toBeNull();expect(screen.queryByDisplayValue('Private gift draft')).toBeNull();h.dispose()
})
