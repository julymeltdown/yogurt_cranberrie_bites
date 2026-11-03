// Contract fixture projections only. No selected character, media or physical authority is invented.
import { party, activity, ids } from '../../src/features/party-discovery/__tests__/fixtures/data'
import { giveaway } from '../../src/features/gift-discovery/__tests__/fixtures/data'
import { participant } from '../../src/features/me/__tests__/fixtures'
export const fixtureClock = '2026-11-07T07:00:00.000Z'
export function domainResponse(request: Request, eventId: string, actorId: string, placeId: string): Response | null {
  const path = new URL(request.url).pathname
  const bind = <T extends object>(value:T) => ({...value,event_id:eventId})
  if(path.endsWith('/me') && request.method==='GET'){
    const p=participant();return Response.json({...p,event_id:eventId,user_id:actorId,profile:{...p.profile,user_id:actorId}})
  }
  if(path.endsWith('/me/parties'))return Response.json(activity())
  if(path.endsWith('/parties'))return Response.json({items:[
    party({party_id:ids.party,event_id:eventId,title:'Area A Queue Group',ticket_section:'Area A',capacity:20,member_count:12,public_place_id:placeId}),
    party({party_id:ids.next,event_id:eventId,title:'After Show Hangout',capacity:12,member_count:8,public_place_id:placeId}),
    party({party_id:'00000000-0000-4000-8000-000000000012',event_id:eventId,title:'Photo Walk (Parade ver.)',capacity:8,member_count:5,public_place_id:placeId}),
  ],next_cursor:null})
  if(path.endsWith('/me/giveaways'))return Response.json({giving:{items:[],next_cursor:null},receiving:{items:[],next_cursor:null}})
  if(path.endsWith('/giveaways'))return Response.json({items:[
    bind(giveaway({title:'Sticker pack · fixture',public_place_id:placeId})),
    bind(giveaway({giveaway_id:'00000000-0000-4000-8000-000000000062',title:'Hand banner · fixture',public_place_id:placeId})),
    bind(giveaway({giveaway_id:'00000000-0000-4000-8000-000000000063',title:'Custom button · fixture',public_place_id:placeId})),
  ],next_cursor:null})
  return null
}
