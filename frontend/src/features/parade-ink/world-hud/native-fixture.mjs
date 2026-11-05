// TEST ONLY: unchanged actual main/Auth Map4 fixture; add only controlled SOCIAL baseline/observations.
import original from '../../../app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json'
await import('../../../app/world-entry/__tests__/map-roof-street-uptake/preview')
const documentId=crypto.randomUUID(),sockets=[]
let hold=false
const RetainedSocket=window.WebSocket
class PolicySocket extends RetainedSocket{
 constructor(...args){super(...args);sockets.push(this)}
 receive(text){
  const frame=JSON.parse(text)
  if(frame.type==='snapshot'&&hold)return
  super.receive(text)
  if(frame.type==='snapshot')super.receive(JSON.stringify({protocolVersion:'1.0.0',type:'policy.baseline',streamId:frame.streamId,eventId:frame.eventId,scope:'SOCIAL',policyRevision:1}))
 }
}
Object.defineProperty(window,'WebSocket',{configurable:true,writable:true,value:PolicySocket})
const fixture={qualification:'CURRENT_MAP4_APPROVED_STATIC_C06_HTTP_C05_CONTROLLED_NOT_LIVE',map:{id:original.map_manifest.map_id,revision:original.map_manifest.map_revision,contentHash:original.map_manifest.content_hash,geometryHash:original.map_manifest.geometry_hash},avatarPin:{manifest_id:original.avatar_manifest.manifest_id,manifest_version:original.avatar_manifest.manifest_version,content_hash:original.avatar_manifest.content_hash}}
const runtime={
 state(){const value=window.__ART06_DEFAULT__.state();return{...value,active:value.activeSockets,peak:value.sockets,documentId,avatar:original.avatar,manifestPin:fixture.avatarPin}},
 async worldProof(){const value=await window.__ART06_DEFAULT__.worldProof();return{...value,actors:value.objects.filter(item=>item.texture?.startsWith('mcr-avatar-'))}},
 offline(value){Object.defineProperty(navigator,'onLine',{configurable:true,get:()=>!value});dispatchEvent(new Event(value?'offline':'online'))},
 hold(value){hold=value},close(){sockets.at(-1)?.serverClose()},
}
Object.assign(window,{__PI08_FIXTURE__:fixture,__PI08_RUNTIME__:runtime})
