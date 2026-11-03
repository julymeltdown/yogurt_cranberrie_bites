// @vitest-environment jsdom
import {afterEach,expect,it,vi} from 'vitest'
import {cleanup,render,screen,fireEvent,waitFor,act} from '@testing-library/react'
import {PartyManagement,PartyRally} from '../ui/PartyManagement'
import {harness,response,ids,party,profile,application,member,errorResponse,pendingReceipt} from './harness'
afterEach(()=>{cleanup();vi.useRealTimers()})
it('last-slot conflict refreshes count and rejects repeated approval without another write',async()=>{
 let full=false;const h=harness(r=>{if(r.method==='POST'){full=true;return errorResponse(409,'VERSION_CONFLICT')}if(new URL(r.url).pathname.endsWith(ids.party))return Response.json(party({owner:profile(ids.actor),capacity:3,member_count:full?3:2,revision:full?4:3}));return response(r)})
 await h.controller.load();await h.controller.decide(ids.app,'APPROVED');expect(h.controller.read().party?.member_count).toBe(3);await h.controller.decide(ids.app,'APPROVED');expect(h.requests.filter(r=>r.method==='POST')).toHaveLength(1);h.dispose()
})
it('uses exact captured applicant and revisions for decision, rejects foreign response',async()=>{
 const h=harness(r=>r.method==='POST'?Response.json(application({applicant:profile(ids.actor),state:'APPROVED',revision:2})):response(r));await h.controller.load();await h.controller.decide(ids.app,'APPROVED');expect(h.controller.read().phase).toBe('uncertain');expect(await h.requests.find(r=>r.method==='POST')!.json()).toEqual({decision:'APPROVED',expected_revision:1,party_revision:3});h.dispose()
})
it('requires explicit removal/cancel confirmations and preserves original key after lost response',async()=>{
 const h=harness(r=>r.method==='DELETE'?Promise.reject(new Error('lost')):r.url.includes('/commands')?errorResponse(404,'RESOURCE_UNAVAILABLE'):response(r));await h.controller.load();await h.controller.remove(ids.other);await h.controller.cancel();expect(h.requests.every(r=>r.method==='GET')).toBe(true)
 h.controller.confirm('remove',ids.other);await h.controller.remove(ids.other);expect(h.controller.read().phase).toBe('uncertain');await h.controller.checkStatus();await h.controller.remove(ids.other);const write=h.requests.find(r=>r.method==='DELETE')!;expect(new URL(write.url).searchParams.get('party_revision')).toBe('3');expect(h.requests.filter(r=>r.method==='DELETE')).toHaveLength(1);expect(new URL(h.requests.at(-1)!.url).searchParams.get('idempotency_key')).toBe(write.headers.get('Idempotency-Key'));h.dispose()
})
it('cancellation receipt rereads actual terminal party before success',async()=>{
 let key='',cancelled=false;const h=harness(r=>{if(r.method==='POST'){key=r.headers.get('Idempotency-Key')!;return Response.json(pendingReceipt(key,{operation_id:'cancelConcertParty'}),{status:202})}
 if(r.url.includes('/commands')){cancelled=true;return Response.json(pendingReceipt(key,{operation_id:'cancelConcertParty',state:'SUCCEEDED',resource_id:ids.party,resource_revision:4,result_status:200}))}
 if(cancelled)return Response.json(party({owner:profile(ids.actor),state:'CANCELLED',revision:4}));return response(r)})
 await h.controller.load();h.controller.confirm('cancel');await h.controller.cancel();expect(h.controller.read().phase).toBe('pending');await h.controller.checkStatus();expect(h.controller.read().phase).toBe('succeeded');expect(h.controller.read().party?.state).toBe('CANCELLED');expect(h.controller.read().members).toBeNull();h.dispose()
})
it('non-owner has no owner commands and removed access scrubs every private row',async()=>{
 const h=harness(r=>new URL(r.url).pathname.endsWith(ids.party)?Response.json(party()):response(r));await h.controller.load();expect(h.controller.isOwner()).toBe(false);await h.controller.decide(ids.app,'APPROVED');h.controller.confirm('cancel');await h.controller.cancel();expect(h.requests.every(r=>r.method==='GET')).toBe(true);expect(h.controller.read().applications).toBeNull();h.updateAccess(null);expect(h.controller.read().members).toBeNull();expect(h.controller.read().party).toBeNull();h.dispose()
})
it('403 on a member read removes meeting/member/application views',async()=>{
 const h=harness(r=>r.url.endsWith('/members?limit=20')?errorResponse(403,'PERMISSION_DENIED'):response(r));await h.controller.load();expect(h.controller.read().phase).toBe('denied');expect(h.controller.read().party).toBeNull();h.dispose()
})
it('rally requires qualified current authority, active membership, explicit acknowledgement; meeting changes clear old rally',async()=>{
 const h=harness(r=>r.method==='PUT'?Response.json({user_id:ids.actor,state:'HERE',meeting_revision:1,revision:1,self_reported_at:'2026-11-04T00:00:00Z'}):response(r));await h.controller.load();await h.controller.loadRally();expect(h.controller.read().rally).not.toBeNull();await h.controller.report('HERE',false);expect(h.requests.every(r=>r.method==='GET')).toBe(true)
 await h.controller.report('HERE',true);expect(h.requests.filter(r=>r.method==='PUT')).toHaveLength(1);expect(await h.requests.find(r=>r.method==='PUT')!.json()).toEqual({state:'HERE',expected_revision:0,meeting_revision:1,physical_presence_acknowledged:true})
 h.updateAuthority(null);expect(h.controller.read().rally).toBeNull();await h.controller.report('HERE',true);expect(h.requests.filter(r=>r.method==='PUT')).toHaveLength(1);h.dispose()
})
it('rejects private extra fields, wrong event members and duplicate IDs',async()=>{
 for(const items of [[{...member(),event_id:ids.other}],[member(),member()],[{...member(),secret:'private'}]]){const h=harness(r=>new URL(r.url).pathname.endsWith('/members')?Response.json({items,next_cursor:null}):response(r));await h.controller.load();expect(h.controller.read().phase).toBe('failed');expect(h.controller.read().members).toBeNull();h.dispose()}
})
it('late requests after logout cannot repopulate management state',async()=>{
 let resolve!:(r:Response)=>void;const h=harness(()=>new Promise(r=>{resolve=r}));const load=h.controller.load();await waitFor(()=>expect(h.requests).toHaveLength(1));h.auth.session.replace(h.auth.session.read().generation,null);resolve(Response.json(party({owner:profile(ids.actor)})));await load;expect(h.controller.read().party).toBeNull();h.dispose()
})
it('renders owner confirmations and clears DOM synchronously on policy revocation',async()=>{
 const h=harness();render(<PartyManagement controller={h.controller}/>);await screen.findByText('Applications');await waitFor(()=>expect(h.controller.read().phase).toBe('ready'));fireEvent.click(screen.getByRole('button',{name:'Cancel party'}));expect(screen.getByRole('dialog')).toBeTruthy();fireEvent.click(screen.getByRole('button',{name:'Keep unchanged'}));expect(screen.queryByRole('dialog')).toBeNull()
 act(()=>{h.auth.privacy.changePolicy(h.auth.privacy.capture(),'consent-revoked')});expect(screen.queryByText('Jules · fixture')).toBeNull();expect(screen.queryByText('Before the encore')).toBeNull();h.dispose()
})
it('expired authority cannot retain visible physical description or report controls',async()=>{
 const h=harness();await h.controller.load();h.updateAuthority({scope:h.auth.privacy.capture(),partyId:ids.party,meetingRevision:1,expiresAt:Date.now()+30,description:'Temporary meeting description'});render(<PartyRally controller={h.controller}/>);await waitFor(()=>expect(screen.queryByText('Temporary meeting description')).toBeNull());expect((screen.getByRole('button',{name:'HERE'}) as HTMLButtonElement).disabled).toBe(true);h.dispose()
})
it('member receipt binds exact user and party and compares only that membership revision',async()=>{
 let key='',removed=false;const h=harness(r=>{if(r.method==='DELETE'){key=r.headers.get('Idempotency-Key')!;return Response.json(pendingReceipt(key,{operation_id:'removeConcertMember'}),{status:202})}
 if(r.url.includes('/commands')){removed=true;return Response.json(pendingReceipt(key,{operation_id:'removeConcertMember',state:'SUCCEEDED',resource_id:ids.other,resource_revision:2,result_status:200}))}
 if(removed&&new URL(r.url).pathname.endsWith('/members'))return Response.json({items:[member(),{...member(ids.other),state:'REMOVED',revision:2}],next_cursor:null});return response(r)})
 await h.controller.load();h.controller.confirm('remove',ids.other);await h.controller.remove(ids.other);await h.controller.checkStatus();expect(h.controller.read().phase).toBe('succeeded');expect(h.controller.read().message).toContain('for this party');h.dispose()
})
it('rally receipt uses actor UUID and compares only exact party/meeting self revision',async()=>{
 let key='',reported=false;const entry={user_id:ids.actor,state:'HERE',meeting_revision:1,revision:1,self_reported_at:'2026-11-04T00:00:00Z'}
 const h=harness(r=>{if(r.method==='PUT'){key=r.headers.get('Idempotency-Key')!;return Response.json(pendingReceipt(key,{operation_id:'setConcertRally'}),{status:202})}
 if(r.url.includes('/commands')){reported=true;return Response.json(pendingReceipt(key,{operation_id:'setConcertRally',state:'SUCCEEDED',resource_id:ids.actor,resource_revision:1,result_status:200}))}
 if(new URL(r.url).pathname.endsWith('/rally'))return Response.json({party_id:ids.party,event_id:ids.event,meeting_revision:1,beacon_id:null,entries:reported?[entry]:[]});return response(r)})
 await h.controller.load();await h.controller.loadRally();await h.controller.report('HERE',true);await h.controller.checkStatus();expect(h.controller.read().phase).toBe('succeeded');expect(h.controller.read().message).toContain('for this meeting');h.dispose()
})
it('superseded meeting reports render STALE and retain monotonic self revision for fresh explicit intent',async()=>{
 const old={user_id:ids.actor,state:'HERE',meeting_revision:1,revision:7,self_reported_at:'2026-11-04T00:00:00Z'}
 const h=harness(r=>{const p=new URL(r.url).pathname;if(r.method==='PUT')return Response.json({...old,meeting_revision:2,revision:8})
 if(p.endsWith('/rally'))return Response.json({party_id:ids.party,event_id:ids.event,meeting_revision:2,beacon_id:null,entries:[old]})
 if(p.endsWith(ids.party))return Response.json(party({owner:profile(ids.actor),meeting_revision:2}));return response(r)})
 h.updateAuthority({scope:h.auth.privacy.capture(),partyId:ids.party,meetingRevision:2,expiresAt:Date.now()+60000,description:'New qualified meeting fixture'})
 await h.controller.load();await h.controller.loadRally();render(<PartyRally controller={h.controller}/>);expect(screen.getByText(/STALE · previous meeting report/)).toBeTruthy()
 await act(()=>h.controller.reportIntent({state:'HERE',meeting_revision:2,physical_presence_acknowledged:true}));expect(await h.requests.find(r=>r.method==='PUT')!.json()).toEqual({state:'HERE',expected_revision:7,meeting_revision:2,physical_presence_acknowledged:true});h.dispose()
})
