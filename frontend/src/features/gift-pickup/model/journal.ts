import {z} from 'zod'
const schema=z.strictObject({version:z.literal(1),actor:z.uuid(),event:z.uuid(),operation:z.enum(['reserveConcertGiveaway','cancelConcertReservation','issueConcertPickupCode','claimConcertReservation']),key:z.uuid(),giveawayId:z.uuid(),reservationId:z.uuid().nullable(),expectedRevision:z.number().int().min(1)})
export type PickupIntent=z.infer<typeof schema>
/** Only command identity metadata. Never store code, token, note, meeting, draft or an HTTP body. */
export interface IntentJournal {read(actor:string,event:string):PickupIntent|null;write(intent:PickupIntent):void;clear(actor:string,event:string):void}
export function createSessionIntentJournal(storage:Pick<Storage,'getItem'|'setItem'|'removeItem'>=sessionStorage):IntentJournal{
 const key=(actor:string,event:string)=>`mcr:pickup-intent:v1:${actor}:${event}`
 return{read(actor,event){const raw=storage.getItem(key(actor,event));if(raw===null)return null;if(raw.length>2048)throw new Error('Invalid intent journal');const value=schema.parse(JSON.parse(raw));if(value.actor!==actor||value.event!==event)throw new Error('Mismatched journal');return value},write(intent){const value=schema.parse(intent);storage.setItem(key(value.actor,value.event),JSON.stringify(value))},clear(actor,event){storage.removeItem(key(actor,event))}}
}
export function validateIntent(value:unknown):PickupIntent{return schema.parse(value)}
