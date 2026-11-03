import { test } from 'node:test'
import assert from 'node:assert/strict'
import { CASES, WIDTHS, viewportFor, summarizeNetwork, counterDelta } from './cases.mjs'
test('all ten distinct routes stay within the original source image and preserve unequal phone aspects',()=>{
 assert.equal(new Set(CASES.map(c=>c.route)).size,10)
 for(const c of CASES){ assert(c.rect.x>=0&&c.rect.y>=0&&c.rect.x+c.rect.width<=1536&&c.rect.y+c.rect.height<=1024)
  for(const w of WIDTHS){ const v=viewportFor(c,w);assert.equal(v.width,w);assert(Math.abs(v.height-w*c.rect.height/c.rect.width)<=.5) } }
 assert.notEqual(viewportFor(CASES[1],390).height,viewportFor(CASES[2],390).height)
})
test('memory cache, disk cache and transferred bytes are not conflated with loader calls',()=>{
 const s=summarizeNetwork([{path:'/atlas.png',type:'Image',fromMemoryCache:true,encodedBytes:0},{path:'/atlas.png',type:'Image',fromDiskCache:true,encodedBytes:0},{path:'/scene.png',type:'Image',encodedBytes:1400},{path:'/app.js',type:'Script',encodedBytes:900}])
 assert.deepEqual(s,{requests:4,transferredBytes:2300,cacheHits:2,imageRequests:3,byPath:{'/app.js':1,'/atlas.png':2,'/scene.png':1}})
 assert.deepEqual(summarizeNetwork([]),{requests:0,transferredBytes:0,cacheHits:0,imageRequests:0,byPath:{}})
})

test('warm request avoidance is distinct from an HTTP cache hit or a fresh decode',()=>{
 const cold={loaderCalls:5,fixtureHttpCalls:6,avatarPuts:0,decode:{calls:4},network:summarizeNetwork([{path:'/atlas.png',encodedBytes:500,type:'Fetch'}])}
 const warm={...cold,loaderCalls:6,fixtureHttpCalls:8,decode:{calls:5}}
 assert.deepEqual(counterDelta(cold,warm),{loaderCalls:1,fixtureHttpCalls:2,avatarPuts:0,explicitDecodeCalls:1,browserRequests:0,transferredBytes:0,cacheHits:0,byPath:{}})
 const saved={...warm,avatarPuts:1,network:summarizeNetwork([{path:'/atlas.png',encodedBytes:500,type:'Fetch'},{path:'/atlas.png',encodedBytes:0,type:'Fetch',fromMemoryCache:true}])}
 assert.deepEqual(counterDelta(warm,saved),{loaderCalls:0,fixtureHttpCalls:0,avatarPuts:1,explicitDecodeCalls:0,browserRequests:1,transferredBytes:0,cacheHits:1,byPath:{'/atlas.png':1}})
})
