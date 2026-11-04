import {expect,it} from 'vitest'
import {responses} from './data'
import {actorId,eventId,giveaway} from '../../../src/features/gift-discovery/__tests__/fixtures/data'
import {giftActivityDecoder,giveawayListDecoder} from '../../../src/features/gift-discovery/model/contract'
const scope={generation:1,userId:actorId,eventId}
const origin=`https://fixture.test/api/concert/events/${eventId}`
it.each(['normal','ended','empty'])('validates native %s giveaway data against the actual frozen public decoder',async mode=>{
  const fixture=responses(eventId,actorId,giveaway().public_place_id,mode)
  const response=await fixture.respond(new Request(origin+'/giveaways'))
  const data=giveawayListDecoder(scope,{limit:20})(await response!.json(),{scope,path:`/api/concert/events/${eventId}/giveaways`,status:200})
  expect(data.items).toHaveLength(mode==='empty'?0:3)
})
it('validates independently continued own activity with the exact actor and cursor pair',async()=>{
  const fixture=responses(eventId,actorId,giveaway().public_place_id,'normal')
  const queries: Record<string,string>[] = [{},{giving_cursor:'giving_next'},{giving_cursor:'giving_next',receiving_cursor:'receiving_next'}]
  for(const query of queries){
    const response=await fixture.respond(new Request(origin+'/me/giveaways?'+new URLSearchParams(query as Record<string,string>)))
    const data=giftActivityDecoder(scope,query)(await response!.json(),{scope,path:`/api/concert/events/${eventId}/me/giveaways`,status:200})
    expect(data.giving.items).toHaveLength(1);expect(data.receiving.items).toHaveLength(1)
  }
})
