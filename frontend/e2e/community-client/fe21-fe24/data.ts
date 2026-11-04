// Synthetic C04 responses only; no backend, physical authority, or product art.
import {party, application, activity, pendingReceipt, ids, errorResponse} from '../../../src/features/party-discovery/__tests__/fixtures/data'
import {giveaway, reservation, uid} from '../../../src/features/gift-discovery/__tests__/fixtures/data'
import type {Application, CommandOperation} from '../../../src/features/party-discovery/model/contract'

export function responses(eventId: string, actorId: string, placeId: string, initialMode: string) {
  let mode = initialMode, status = 'PENDING', current: Application | null = null, sequence = 800
  const commands = new Map<string, {operation: CommandOperation; result: Application}>()
  const calls: {path: string; query: string; method: string; key: string | null; body: string}[] = []
  const parties = () => [party({event_id: eventId, public_place_id: placeId, member_count: mode === 'full' ? 5 : 2,
    ...(mode === 'cancelled' ? {state: 'CANCELLED'} : {})}), party({event_id: eventId, public_place_id: placeId, party_id: ids.next, title: 'After the encore'})]
  const gifts = () => ['Fan badges', 'Paused badges', 'Last badges'].map((title, index) => giveaway({event_id: eventId,
    giveaway_id: uid(100-index), title, owner: {...giveaway().owner, user_id: uid(900)}, public_place_id: placeId,
    image_media_id: null, created_at: new Date(Date.parse('2026-11-01T00:00Z')-index*60000).toISOString(),
    pickup_starts_at: '2026-11-07T06:00:00Z', pickup_ends_at: mode === 'ended' ? '2026-11-07T06:30:00Z' : '2026-11-07T08:00:00Z',
    state: index === 1 ? 'PAUSED' : 'OPEN', ...(index === 2 ? {available: 0} : {}),
  }))
  return {calls, setMode(value: string) {mode=value}, setStatus(value: string) {status=value},
    async respond(request: Request): Promise<Response | null> {
      const url = new URL(request.url), path=url.pathname, query=url.searchParams, key=request.headers.get('Idempotency-Key')
      if(!/\/(parties|party-applications|commands|giveaways)(\/|$)/.test(path)) return null
      calls.push({path,query:url.search,method:request.method,key,body:await request.clone().text()})
      if(mode==='private')return errorResponse(403,'PERMISSION_DENIED')
      if(path.endsWith('/commands')) {
        if(status==='404')return errorResponse(404,'RESOURCE_UNAVAILABLE')
        const stored=commands.get(query.get('idempotency_key')!)!
        if(status==='SUCCEEDED')current=stored.result
        return Response.json(pendingReceipt(query.get('idempotency_key')!, {event_id:eventId,actor_id:actorId,operation_id:stored.operation,
          ...(status==='SUCCEEDED'?{state:'SUCCEEDED',resource_id:stored.result.application_id,resource_revision:stored.result.revision,result_status:200}: {})}))
      }
      if(request.method==='POST'||request.method==='DELETE') {
        const operation=request.method==='POST'?'applyConcertParty':'cancelConcertApplication', result=application({event_id:eventId,
          applicant:{...application().applicant,user_id:actorId},application_id:request.method==='POST'?uid(++sequence):path.split('/').at(-1)!,
          party_id:request.method==='POST'?path.split('/').at(-2)!:current!.party_id,state:request.method==='POST'?'PENDING':'CANCELLED',revision:(current?.revision??0)+1})
        commands.set(key!,{operation,result})
        if(mode==='lost')throw new TypeError('TEST ONLY lost command response')
        if(mode==='pending')return Response.json(pendingReceipt(key!,{event_id:eventId,actor_id:actorId,operation_id:operation}),{status:202})
        current=result;return Response.json(result)
      }
      if(path.endsWith('/me/parties'))return Response.json({...activity(),applications:{items:current?[current]:[],next_cursor:null}})
      if(path.endsWith('/parties'))return Response.json({items:mode==='empty'?[]:parties().filter(p=>
        (!query.has('ticket_section')||p.ticket_section===query.get('ticket_section'))&&(!query.has('state')||p.state===query.get('state'))
        &&(!query.has('has_space')||p.member_count<p.capacity)&&(!query.has('starts_after')||p.meeting_starts_at>=query.get('starts_after')!)
        &&(!query.has('starts_before')||p.meeting_starts_at<query.get('starts_before')!)),next_cursor:null})
      if(path.includes('/parties/'))return Response.json(parties().find(p=>path.endsWith(p.party_id)))
      if(path.endsWith('/me/giveaways')) {
        const givingNext=query.has('giving_cursor'), receivingNext=query.has('receiving_cursor')
        return Response.json({giving:{items:[{...gifts()[0]!,giveaway_id:uid(givingNext?200:201),title:givingNext?'Your published badges':'Your draft badges',
          created_at:givingNext?'2026-10-31T00:00:00Z':'2026-11-01T00:00:00Z',state:givingNext?'OPEN':'DRAFT',owner:{...gifts()[0]!.owner,user_id:actorId}}],next_cursor:givingNext?null:'giving_next'},
          receiving:{items:[reservation({event_id:eventId,recipient_id:actorId,giveaway_id:uid(100),reservation_id:uid(receivingNext?500:501),
            created_at:receivingNext?'2026-10-31T00:00:00Z':'2026-11-01T00:00:00Z',expires_at:'2026-11-07T08:00:00Z'})],next_cursor:receivingNext?null:'receiving_next'}})
      }
      if(path.endsWith('/giveaways'))return Response.json({items:mode==='empty'?[]:gifts().filter(g=>
        (!query.has('state')||g.state===query.get('state'))&&(!query.has('in_stock')||(g.available>0)===(query.get('in_stock')==='true'))
        &&(!query.has('pickup_after')||g.pickup_starts_at>=query.get('pickup_after')!)&&(!query.has('pickup_before')||g.pickup_starts_at<query.get('pickup_before')!)),next_cursor:null})
      if(path.includes('/giveaways/')) {
        const gift=gifts().find(g=>path.endsWith(g.giveaway_id))
        return Response.json(mode==='private-projection'?{...gift,recipient_id:'MUST_NOT_RENDER_PRIVATE_RECIPIENT'}:gift)
      }
      return null
    },
  }
}
