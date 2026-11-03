// Pixel bounds read from the preserved 1536x1024 target. Each phone keeps its own aspect.
export const REFERENCE_SHA256 = 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617'
export const CLOCK = '2026-11-07T07:00:00.000Z'
export const WIDTHS = [360, 390, 430]
export const CASES = [
  { id:'landing', route:'/', rect:{x:4,y:3,width:300,height:526}, public:true, content:'.public-entry' },
  { id:'login', route:'/login', rect:{x:309,y:3,width:275,height:526}, public:true, content:'.auth-view' },
  { id:'world', route:'/world', rect:{x:589,y:3,width:370,height:526}, content:'.world-shell' },
  { id:'map', route:'/world/map', rect:{x:963,y:3,width:272,height:526}, content:'.shell-panel' },
  { id:'customize', route:'/me/avatar', rect:{x:1240,y:3,width:294,height:526}, content:'.shell-panel' },
  { id:'feed', route:'/feed', rect:{x:3,y:539,width:298,height:482}, content:'.shell-panel' },
  { id:'meet', route:'/meet', rect:{x:307,y:539,width:295,height:482}, content:'.shell-panel' },
  { id:'gifts', route:'/gifts', rect:{x:607,y:539,width:309,height:482}, content:'.shell-panel' },
  { id:'wall', route:'/wall', rect:{x:920,y:539,width:294,height:482}, content:'.shell-panel' },
  { id:'me', route:'/me', rect:{x:1218,y:539,width:316,height:482}, content:'.shell-panel' },
]
export function viewportFor(item,width){return {width,height:Math.round(width*item.rect.height/item.rect.width)}}
export function summarizeNetwork(entries){
  // Actual browser requests are distinct from fixture HTTP and resolver-loader invocations.
  return { requests:entries.length, transferredBytes:entries.reduce((n,e)=>n+(e.encodedBytes??0),0),
    cacheHits:entries.filter(e=>e.fromDiskCache||e.fromMemoryCache||e.fromServiceWorker).length,
    imageRequests:entries.filter(e=>e.type==='Image').length,
    byPath:Object.fromEntries([...new Set(entries.map(e=>e.path))].sort().map(path=>[path,entries.filter(e=>e.path===path).length])) }
}
