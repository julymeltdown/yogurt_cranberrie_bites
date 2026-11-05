import { describe, expect, it, vi } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../http'
import { createQueryPrivacy } from '../../../query/privacy'
import { buildNotesRequest, createNotesClient, NotesApiError, VenueContractError, type CreatePlaceNote,
  type NoteCommand, type NotePage, type NoteThread, type NotesTransport, type PlaceNote } from '../notes'

const eventId = '00000000-0000-4000-8000-000000000201'
const userId = '00000000-0000-4000-8000-000000000101'
const noteId = '00000000-0000-4000-8000-000000000301'
const replyId = '00000000-0000-4000-8000-000000000302'
const key = '00000000-0000-4000-8000-000000000401'
const landmarkId = 'reviewed.main-gate~v1'
const time = '2026-11-05T01:00:00Z'
const expires = '2026-11-05T02:00:00Z'
const base = {eventId, landmarkId}
const body: CreatePlaceNote = {text:'Meet beside the main gate', audience:'FRIENDS', expiry:'ONE_HOUR',
  landmark_selection:{landmark_id:landmarkId, catalog_revision:2, landmark_revision:7}}
function note(patch: Partial<PlaceNote> = {}): PlaceNote {
  return {id:noteId, landmark_id:landmarkId, landmark_revision:7,
    author:{user_id:userId,display_name:'Fixture friend',bio:null,avatar:{revision:1,config:{contract_version:'1.1.0',kind:'avatar_config',
      manifest_id:'mcr_selected_static_20261103',manifest_version:1,representation:'whole_character',preset_id:'photo01_front_01'}}},
    text:'Meet beside the main gate',created_at:time,expires_at:expires,revision:1,audience:'FRIENDS',expiry:'ONE_HOUR',
    parent_id:null,reply_count:0,can_reply:true,can_withdraw:true,meet_here:null,...patch}
}
function page(patch: Partial<NotePage> = {}): NotePage {
  return {items:[note()],visible_count:1,parent:null,next_before:null,server_now:time,access_until:'2026-11-05T01:00:05Z',...patch}
}
function receipt(patch: Partial<NoteCommand> = {}): NoteCommand {
  return {receipt_id:key,operation_id:'createVenuePlaceNote',state:'PENDING',resource:null,failure_reason:null,created_at:time,expires_at:expires,...patch}
}
function privacyFixture() {
  const session = createMemorySession()
  session.replace(0,{userId,eventId,accessToken:'fixture.token'})
  const query = new QueryClient()
  const privacy = createQueryPrivacy({client:query,session})
  return {session,query,privacy,scope:privacy.capture()}
}

describe('qualified N01 public request projection', () => {
  it('builds a read without acquiring, sending, sharing, inviting, or generating a key', () => {
    const request = buildNotesRequest('listVenuePlaceNotes',{...base,limit:20,before:'1.'+noteId})
    expect(request.method).toBe('GET')
    expect(request.path).toBe(`/api/concert/events/${eventId}/venue/landmarks/${landmarkId}/notes`)
    expect(request.query).toEqual({limit:20,before:'1.'+noteId})
    expect(request.body).toBeUndefined(); expect(request.idempotencyKey).toBeUndefined()
    expect(request.cache).toBe('no-store')
  })
  it('preserves an explicit command key and detaches the original body', () => {
    const original = {...body, landmark_selection:{...body.landmark_selection}}
    const request = buildNotesRequest('createVenuePlaceNote',{...base,idempotencyKey:key,body:original})
    original.text='changed'; original.landmark_selection.landmark_id='different'
    expect(request.method).toBe('POST'); expect(request.idempotencyKey).toBe(key)
    expect(request.body).toEqual(body); expect(Object.isFrozen(request.body)).toBe(true)
    expect(Object.isFrozen((request.body as CreatePlaceNote).landmark_selection)).toBe(true)
  })
  it('keeps withdrawal CAS in the exact query and sends no body', () => {
    const request = buildNotesRequest('withdrawVenuePlaceNote',{...base,noteId,expected_revision:9,idempotencyKey:key})
    expect(request.method).toBe('DELETE'); expect(request.query).toEqual({expected_revision:9})
    expect(request.body).toBeUndefined(); expect(request.idempotencyKey).toBe(key)
  })
  it('replies accept no audience/expiry override and preserve expected parent revision', () => {
    const request = buildNotesRequest('createVenuePlaceReply',{...base,noteId,idempotencyKey:key,body:{text:'On my way',expected_parent_revision:4}})
    expect(request.body).toEqual({text:'On my way',expected_parent_revision:4})
    expect(() => buildNotesRequest('createVenuePlaceReply',{...base,noteId,idempotencyKey:key,
      body:{text:'x',expected_parent_revision:4,audience:'EVERYONE'} as never})).toThrow(VenueContractError)
  })
  it.each(['', '.', '..', '../gate', 'gate/a', 'gate%2fa', 'gate?x', 'gate#x', 'gate x'])('rejects unsafe provider landmark %j', invalid => {
    expect(() => buildNotesRequest('listVenuePlaceNotes',{...base,landmarkId:invalid})).toThrow(VenueContractError)
  })
  it.each(['00000000-0000-0000-0000-000000000000','not-a-uuid','AAAAAAAA-0000-4000-8000-000000000201'])('rejects invalid event %j', invalid => {
    expect(() => buildNotesRequest('listVenuePlaceNotes',{...base,eventId:invalid})).toThrow(VenueContractError)
  })
  it.each([0,51,1.5,NaN])('rejects invalid page bound %j', limit => {
    expect(() => buildNotesRequest('listVenuePlaceNotes',{...base,limit})).toThrow(VenueContractError)
  })
  it('rejects a create whose reviewed selection differs from its actual URL provider ID', () => {
    expect(() => buildNotesRequest('createVenuePlaceNote',{...base,idempotencyKey:key,
      body:{...body,landmark_selection:{...body.landmark_selection,landmark_id:'spatial-art-label'}}})).toThrow(VenueContractError)
  })
  it.each(['', ' \n ', '\ud800', 'line\twith-tab', 'x'.repeat(1001)])('rejects malformed or oversized note text', text => {
    expect(() => buildNotesRequest('createVenuePlaceNote',{...base,idempotencyKey:key,body:{...body,text}})).toThrow(VenueContractError)
  })
  it('counts Unicode scalars and accepts exactly 1000 four-byte characters', () => {
    expect(buildNotesRequest('createVenuePlaceNote',{...base,idempotencyKey:key,body:{...body,text:'🎸'.repeat(1000)}}).method).toBe('POST')
  })
  it('uses VENUE_END_DAY from the wire, never the consumer label or client midnight', () => {
    expect(buildNotesRequest('createVenuePlaceNote',{...base,idempotencyKey:key,body:{...body,expiry:'VENUE_END_DAY'}}).body).toEqual({...body,expiry:'VENUE_END_DAY'})
    expect(() => buildNotesRequest('createVenuePlaceNote',{...base,idempotencyKey:key,body:{...body,expiry:'END_OF_DAY'} as never})).toThrow(VenueContractError)
  })
  it('rejects unknown fields, accessors and a missing caller-owned idempotency key', () => {
    expect(() => buildNotesRequest('listVenuePlaceNotes',{...base,viewer:userId} as never)).toThrow(VenueContractError)
    expect(() => buildNotesRequest('listVenuePlaceNotes',{get eventId(): string {throw new Error('secret')},landmarkId})).toThrow(VenueContractError)
    expect(() => buildNotesRequest('createVenuePlaceNote',{...base,body} as never)).toThrow(VenueContractError)
  })
})

describe('private N01 response boundaries', () => {
  const roots = () => buildNotesRequest('listVenuePlaceNotes',base)
  it('detaches and freezes a valid currently authorized page', () => {
    const input = page(), decoded = roots().decode(input,200)
    expect(decoded).toEqual(input); expect(decoded).not.toBe(input)
    expect(Object.isFrozen(decoded.items[0]!.author.avatar.config)).toBe(true)
    expect(decoded.items[0]!.meet_here).toBeNull()
  })
  it('rejects wrong place, duplicate identity and a reply in the root list', () => {
    for (const items of [[note({landmark_id:'other'})],[note(),note()],[note({parent_id:replyId})]]) {
      expect(() => roots().decode(page({items}),200)).toThrow(VenueContractError)
    }
  })
  it('rejects expired reads/notes and calendar-normalized malformed timestamps', () => {
    expect(() => roots().decode(page({access_until:time}),200)).toThrow(VenueContractError)
    expect(() => roots().decode(page({items:[note({expires_at:time})]}),200)).toThrow(VenueContractError)
    expect(() => roots().decode(page({server_now:'2026-02-30T01:00:00Z'}),200)).toThrow(VenueContractError)
  })
  it('binds a thread and all replies to the requested root and inherited expiry/audience', () => {
    const request = buildNotesRequest('getVenuePlaceNote',{...base,noteId})
    const thread: NoteThread = {note:note(),replies:[note({id:replyId,parent_id:noteId})],next_before:null,server_now:time,access_until:'2026-11-05T01:00:05Z'}
    expect(request.decode(thread,200).replies).toHaveLength(1)
    for (const reply of [note({id:replyId,parent_id:key}),note({id:replyId,parent_id:noteId,audience:'EVERYONE'}),
      note({id:replyId,parent_id:noteId,expires_at:'2026-11-05T03:00:00Z'})]) {
      expect(() => request.decode({...thread,replies:[reply]},200)).toThrow(VenueContractError)
    }
  })
  it('binds reply pagination to the actual parent, including empty authorized results', () => {
    const request = buildNotesRequest('listVenuePlaceReplies',{...base,noteId,limit:1})
    expect(request.decode(page({parent:note(),items:[]}),200).items).toHaveLength(0)
    expect(() => request.decode(page({parent:null,items:[]}),200)).toThrow(VenueContractError)
  })
  it('keeps PENDING factual and rejects a receipt for another operation or private raw body', () => {
    const request = buildNotesRequest('createVenuePlaceNote',{...base,idempotencyKey:key,body})
    expect(request.decode(receipt(),201).state).toBe('PENDING')
    expect(() => request.decode(receipt({operation_id:'createVenuePlaceReply'}),201)).toThrow(VenueContractError)
    expect(() => request.decode({...receipt(),text:'private note'},201)).toThrow(VenueContractError)
    expect(() => request.decode(receipt(),202)).toThrow(VenueContractError)
  })
  it('exposes only a safe typed rejection, not server message/trace/body', () => {
    try {
      roots().decode({code:'CONCERT_404',message:'secret text',traceId:key,timestamp:time,details:{reason:'RESOURCE_UNAVAILABLE'}},404)
      throw new Error('Expected rejection')
    } catch (error) {
      expect(error).toBeInstanceOf(NotesApiError)
      expect(String(error)).not.toContain('secret')
      expect(error).toMatchObject({status:404,reason:'RESOURCE_UNAVAILABLE'})
      expect(error).not.toHaveProperty('traceId')
    }
    expect(() => roots().decode({secret:'raw'},503)).toThrow(VenueContractError)
    expect(() => roots().decode(page(),201)).toThrow(VenueContractError)
  })
})

describe('borrowed canonical privacy lifetime', () => {
  it('constructs without dispatch and rejects a different event before transport', async () => {
    const f = privacyFixture(), send = vi.fn<NotesTransport['send']>()
    const client = createNotesClient({privacy:f.privacy,transport:{send}})
    expect(send).not.toHaveBeenCalled()
    await expect(client.request('listVenuePlaceNotes',{...base,eventId:noteId},{scope:f.scope,signal:new AbortController().signal})).rejects.toThrow(VenueContractError)
    expect(send).not.toHaveBeenCalled(); f.privacy.dispose(); f.query.clear()
  })
  it('rejects a late response after actual canonical privacy withdrawal', async () => {
    const f = privacyFixture()
    let finish!: (response: {status:number;data:unknown}) => void
    const send = vi.fn<NotesTransport['send']>(() => new Promise(resolve => {finish=resolve}))
    const pending = createNotesClient({privacy:f.privacy,transport:{send}}).request('listVenuePlaceNotes',base,{scope:f.scope,signal:new AbortController().signal})
    const rejected = expect(pending).rejects.toThrow()
    await vi.waitFor(() => expect(send).toHaveBeenCalledTimes(1))
    f.privacy.changePolicy(f.scope,'block'); finish({status:200,data:page()})
    await rejected
    expect(send).toHaveBeenCalledTimes(1); f.privacy.dispose(); f.query.clear()
  })
  it('never retries or resends an uncertain command after transport failure', async () => {
    const f = privacyFixture(), send = vi.fn<NotesTransport['send']>().mockRejectedValue(new Error('uncertain'))
    await expect(createNotesClient({privacy:f.privacy,transport:{send}}).request('createVenuePlaceNote',
      {...base,idempotencyKey:key,body},{scope:f.scope,signal:new AbortController().signal})).rejects.toThrow('uncertain')
    expect(send).toHaveBeenCalledTimes(1)
    expect(send.mock.calls[0]![0].idempotencyKey).toBe(key)
    f.privacy.dispose(); f.query.clear()
  })
})
