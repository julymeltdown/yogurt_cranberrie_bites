import assert from 'node:assert/strict'
import {resolve} from 'node:path'
const bindingKeys=['runtime_owner_pid','expires_at','gateway_url','world_ws_url','browser_origin','event_id','rooms','release_digest','source_revision','gateway_binary_sha256','concert_binary_sha256','principals']
/** Validate the live window, then freeze the exact cohort used by this browser attempt. */
export function liveBinding(value,now=Date.now()){
 assert(value&&typeof value==='object'&&!Array.isArray(value),'LIVE_DESCRIPTOR_INVALID')
 assert(Number.isSafeInteger(value.runtime_owner_pid)&&value.runtime_owner_pid>1,'LIVE_PARENT_INVALID')
 const expires=Date.parse(value.expires_at)
 assert(Number.isFinite(expires)&&Number.isFinite(now)&&expires-now>=15000,'LIVE_LEASE_EXPIRED_OR_INVALID')
 assert(['http://127.0.0.1:5173','http://localhost:5173'].includes(value.browser_origin),'APP_ORIGIN_NOT_ADMITTED')
 const gateway=new URL(value.gateway_url),world=new URL(value.world_ws_url)
 assert(gateway.protocol==='http:'&&gateway.hostname==='127.0.0.1'&&!gateway.username&&!gateway.password,'GATEWAY_NOT_LOCAL')
 assert(world.protocol==='ws:'&&world.hostname==='127.0.0.1'&&world.pathname==='/world-ws'&&!world.username&&!world.password,'WORLD_ENDPOINT_NOT_LOCAL')
 assert(Array.isArray(value.principals)&&value.principals.length===2&&value.principals.every(p=>typeof p==='string'&&p.startsWith('/'))&&value.principals[0]!==value.principals[1],'TWO_PRIVATE_PRINCIPAL_REFS_REQUIRED')
 return JSON.stringify(bindingKeys.map(k=>value[k]))
}
export function assertSameLiveBinding(value,expected,now=Date.now()){
 assert.equal(liveBinding(value,now),expected,'LIVE_COHORT_CHANGED');return value
}
/** Evidence is append-only by attempt; a run identifier cannot escape its delegated tree. */
export function runDirectory(base,id){
 assert(typeof id==='string'&&/^[A-Za-z0-9][A-Za-z0-9_-]{0,100}$/.test(id),'RUN_ID_INVALID')
 return resolve(base,id)
}
