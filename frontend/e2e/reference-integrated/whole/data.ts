// Deterministic HTTP/C05 data only. Selected C06 manifest/config/art pins stay unchanged.
import original from '../../../../contracts/fixtures/core/bootstrap-open.json'
import selected from '../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import movement from '../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { illustratedReleases } from '../../../src/world/__tests__/illustrated/fixtures'
import { contentHash } from '../../../src/world/map/json'
import { validate } from '../../../src/world/network/transport/contract'
import { validateWholeCharacterConfig } from '../../../src/entities/avatar/whole-character'
import type { Snapshot, Delta } from '../../../src/world/network/WorldTransport'
import type { AvatarState } from '../../../src/entities/avatar/whole-admission'
export const CLOCK='2026-11-07T07:00:00.000Z'
export const eventId=original.event.event_id,userId=original.participant.user_id
export const presets=selected.presets.map(p=>validateWholeCharacterConfig(p.config))
export const pin=Object.freeze({manifest_id:selected.manifest_id,manifest_version:selected.manifest_version,content_hash:selected.content_hash})
export async function wholeData(read?: (path:string)=>Promise<unknown>){
 const art=await illustratedReleases(read)
 // Only map/theme/place admission is simulated. This never mutates selected avatar authority.
 const testReview={source_kind:'ORIGINAL_ART',review_state:'REVIEWED',reviewed_by:'TEST ONLY map admission; not product approval',reviewed_at:'2026-11-03T00:00:00Z'}
 Object.assign(art.map.provenance,testReview);for(const place of art.map.places)Object.assign(place.provenance,testReview)
 art.map.content_hash=await contentHash(art.map)
 for(const theme of art.themes){Object.assign(theme.provenance,testReview);Object.assign(theme,{map_content_hash:art.map.content_hash,avatar_manifest_id:pin.manifest_id,avatar_manifest_version:pin.manifest_version,avatar_content_hash:pin.content_hash});theme.content_hash=await contentHash(theme)}
 if(await contentHash(selected)!==selected.content_hash)throw Error('Immutable selected manifest pin changed')
 const avatar:AvatarState={revision:7,config:presets[0]!}
 const bootstrap={...structuredClone(original),server_time:CLOCK,avatar,avatar_manifest:structuredClone(selected),map_manifest:art.map,theme_manifests:art.themes,
  participant:{...structuredClone(original.participant),profile:{...structuredClone(original.participant.profile),display_name:'Selected fixture fan',avatar},preferences:{...structuredClone(original.participant.preferences),theme:'REVENGE'}}}
 for(const capability of Object.values(bootstrap.capabilities))Object.assign(capability,{available:true,reason:null})
 return {bootstrap,avatar,pin,presets}
}
export type WholeData=Awaited<ReturnType<typeof wholeData>>
export function participant(data:WholeData,avatar:AvatarState){return {...structuredClone(data.bootstrap.participant),profile:{...structuredClone(data.bootstrap.participant.profile),avatar:structuredClone(avatar)}}}
export function frame(data:WholeData,roomId:string,avatar:AvatarState,cause:Snapshot['cause']='INITIAL',requestId:string|null=null):Snapshot{
 const template=movement.messages[1]!.message
 return validate<Snapshot>('Snapshot',{...template,eventId,roomId,mapId:data.bootstrap.map_manifest.map_id,mapRevision:data.bootstrap.map_manifest.map_revision,
  avatarManifestId:pin.manifest_id,avatarManifestVersion:pin.manifest_version,cause,requestId,selfUserId:userId,ackInputSeq:0,accessExpiresAt:new Date(Date.now()+60000).toISOString(),
  players:presets.map((config,index)=>({userId:index===0?userId:`00000000-0000-4000-8000-00000000090${index}`,displayName:index===0?'Selected fixture fan':`Selected visitor ${index+1}`,position:{x:140+index%3*72,y:520+Math.floor(index/3)*88},facing:'down',moving:false,avatar:index===0?avatar:{revision:1,config}}))})
}
export function changedSelf(snapshot:Snapshot,avatar:AvatarState):Delta{
 return validate<Delta>('Delta',{protocolVersion:'1.0.0',type:'delta',streamId:snapshot.streamId,streamSeq:1,roomEpoch:snapshot.roomEpoch,serverTick:snapshot.serverTick+1,ackInputSeq:0,
  upserts:[{...snapshot.players.find(p=>p.userId===userId)!,avatar}],removes:[]})
}
