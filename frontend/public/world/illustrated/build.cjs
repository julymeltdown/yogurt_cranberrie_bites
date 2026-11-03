// Original editable scene authoring. Reads immutable AR02; writes ONLY this folder.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto')
const out=__dirname,root=path.resolve(out,'../../../..')
const read=p=>JSON.parse(fs.readFileSync(path.join(root,p),'utf8'))
const g=read('frontend/public/world/maps/geometry.json')
const landmarks=read('frontend/public/world/maps/landmarks.json')
const originalInputs=read('frontend/public/world/maps/binding-inputs.json')
const hash=bytes=>'sha256:'+crypto.createHash('sha256').update(bytes).digest('hex')
const W=1152,H=896,T=32
const palette={
 REVENGE:{grass:['#7b8854','#87945e','#657849','#a2a970'],stone:['#c7bba4','#d8ccb4','#bdb29c','#a59c89'],accent:'#b33248',paper:'#f2e5cb',ink:'#252329',metal:'#8b8584',flower:'#e3b4aa'},
 PARADE:{grass:['#77816c','#8d9580','#626f5e','#a5aa91'],stone:['#bfc1b5','#d3d4c5','#aeb5ab','#909e95'],accent:'#dfd3b2',paper:'#f2ead6',ink:'#282a30',metal:'#a1a5a3',flower:'#ece9d7'},
 DANGER:{grass:['#8ca451','#a4b45d','#708c46','#bfca7a'],stone:['#cfbda0','#dfcfad','#c4b596','#a99b7b'],accent:'#f2d142',paper:'#fff1cb',ink:'#243842',metal:'#67b1ba',flower:'#efb85d'},
}
function rng(seed){let n=seed>>>0;return()=>{n=(Math.imul(n,1664525)+1013904223)>>>0;return n/4294967296}}
function rect(x,y,w,h,fill,extra=''){return `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${fill}" ${extra}/>`}
function poly(points,fill,extra=''){return `<polygon points="${points.map(p=>p.join(',')).join(' ')}" fill="${fill}" ${extra}/>`}
function line(x1,y1,x2,y2,color,width=1){return `<path d="M${x1} ${y1}L${x2} ${y2}" fill="none" stroke="${color}" stroke-width="${width}"/>`}
function svg(body,title){return `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}"><title>${title}</title>${body}</svg>`}
function ground(theme){
 const p=palette[theme],pieces=[rect(0,0,W,H,p.grass[0])]
 for(let cy=0;cy<28;cy++)for(let cx=0;cx<36;cx++){
  const id=g.terrain_layer[cy][cx],x=cx*T,y=cy*T,r=rng(cx*927+cy*673+19)
  if(id===2||id===3||([6,7].includes(id)&&cx>=22)){
   pieces.push(rect(x,y,32,32,p.stone[3]))
   // Irregular staggered paving stones with material chips; no square tile grid.
   for(let row=0;row<4;row++)for(let col=-1;col<4;col++){
    const sx=x+col*13+(row%2?6:0),sy=y+row*8
    const left=Math.max(x,sx),right=Math.min(x+32,sx+12)
    if(right<=left)continue
    pieces.push(poly([[left,sy+1],[right-1,sy],[right,sy+5],[right-2,sy+7],[left,sy+6]],p.stone[Math.floor(r()*3)]))
    if(right-left>5)pieces.push(line(left+2,sy+1,right-2,sy+1,p.paper))
   }
  }else if(id===4){
   pieces.push(rect(x,y,32,32,p.stone[2]));for(let n=0;n<18;n++){let sx=x+Math.floor(r()*31),sy=y+Math.floor(r()*31);pieces.push(rect(sx,sy,1+(n%2),1,n%3?p.stone[3]:p.stone[0]))}
  }else if(id===5){pieces.push(rect(x,y,32,32,'#473c36'));for(let sy=0;sy<32;sy+=8)pieces.push(line(x,y+sy,x+32,y+sy,'#6c5b4b'))}
  else{
   for(let n=0;n<11;n++){let sx=x+Math.floor(r()*30),sy=y+Math.floor(r()*30);pieces.push(rect(sx,sy,1,2,p.grass[1+n%3]));if(n%3===0)pieces.push(rect(sx+1,sy+1,2,1,p.grass[1]))}
   if(r()>.65)pieces.push(rect(x+5+Math.floor(r()*23),y+5+Math.floor(r()*23),2,1,p.flower))
  }
 }
 // Natural paving/lawn edge treatment follows the EXACT immutable terrain cells.
 for(let y=1;y<27;y++)for(let x=1;x<35;x++)if([2,3].includes(g.terrain_layer[y][x])){
  const px=x*32,py=y*32
  for(const [dx,dy]of[[1,0],[-1,0],[0,1],[0,-1]])if(g.terrain_layer[y+dy][x+dx]===1){
   if(dx){pieces.push(line(px+(dx>0?31:1),py,px+(dx>0?31:1),py+32,p.stone[3],2))}
   else pieces.push(line(px,py+(dy>0?31:1),px+32,py+(dy>0?31:1),p.stone[3],2))
  }
 }
 // A flush, walkable plaza medallion: ground illustration only, no new obstacle.
 const cx=16*32,cy=18.5*32
 pieces.push(`<g id="plaza-inlay"><circle cx="${cx}" cy="${cy}" r="42" fill="${p.stone[1]}" stroke="${p.stone[3]}" stroke-width="3"/><circle cx="${cx}" cy="${cy}" r="34" fill="none" stroke="${p.stone[3]}"/>`)
 for(let i=0;i<16;i++){const a=i*Math.PI/8;pieces.push(line(cx+36*Math.cos(a),cy+36*Math.sin(a),cx+40*Math.cos(a),cy+40*Math.sin(a),p.stone[3],2))}
 pieces.push(poly([[cx,cy-28],[cx+7,cy-7],[cx+28,cy],[cx+7,cy+7],[cx,cy+28],[cx-7,cy+7],[cx-28,cy],[cx-7,cy-7]],p.stone[3]),`</g>`)
 return `<g id="editable-ground" shape-rendering="crispEdges">${pieces.join('')}</g>`
}
const placements=[
 {id:'studio',source:'studio-original.png',x:256,y:69,width:352,height:219,depth_y:288,footprint:[8,3,11,6],interaction:'studio_info'},
 {id:'stage',source:'stage-original.png',x:96,y:329,width:224,height:151,depth_y:480,footprint:[3,11,7,4],interaction:'stage_info'},
 {id:'merch_gifts',source:'booth-original.png',x:864,y:267,width:160,height:117,depth_y:384,footprint:[27,10,5,2],interaction:'gifts_info'},
 {id:'mural',source:'mural-original.png',x:832,y:609,width:192,height:95,depth_y:704,footprint:[26,20,6,2],interaction:'wall_info'},
 {id:'greenery_west',source:'greenery-original.png',x:92,y:107,width:104,height:149,depth_y:256,footprint:[3,4,3,4]},
 {id:'greenery_north',source:'greenery-original.png',x:668,y:57,width:104,height:167,depth_y:224,footprint:[21,3,3,4]},
 {id:'greenery_east',source:'greenery-original.png',x:988,y:57,width:104,height:167,depth_y:224,footprint:[31,3,3,4]},
 {id:'greenery_southwest',source:'greenery-original.png',x:92,y:653,width:168,height:115,depth_y:768,footprint:[3,22,5,2]},
 {id:'greenery_south',source:'greenery-original.png',x:604,y:650,width:104,height:150,depth_y:800,footprint:[19,22,3,3]},
]
// Boundary planting has roots on the existing blocked perimeter only. Canopies
// may overhang walkable ground; they never create or modify a collision cell.
for(const [id,x,y]of[['west_a',-31,42],['west_b',-31,232],['west_c',-31,444],['west_d',-31,658],['east_a',1095,28],['east_b',1095,235],['east_c',1095,466],['east_d',1095,663]]){
 const left=x<0,cy=Math.min(24,Math.floor((y+128)/32))
 placements.push({id:`perimeter_${id}`,source:'greenery-original.png',x,y,width:88,height:150,depth_y:y+150,footprint:[left?0:35,cy,1,1],atlas_frame:[112,224,104,167],role:'canopy-overhang; roots remain on immutable blocked perimeter'})
}
const atlasFrames={studio:[0,0,352,219],stage:[352,0,224,151],merch_gifts:[576,0,160,117],mural:[352,160,192,95],greenery_west:[0,224,104,149],greenery_north:[112,224,104,167],greenery_east:[112,224,104,167],greenery_southwest:[224,272,168,115],greenery_south:[400,272,104,150]}
for(const o of placements)o.atlas_frame??=atlasFrames[o.id]
function eraDefs(theme){return theme==='PARADE'?`<filter id="era"><feColorMatrix type="saturate" values="0.12"/></filter>`:theme==='DANGER'?`<filter id="era"><feColorMatrix type="hueRotate" values="145"/><feColorMatrix type="saturate" values="0.8"/></filter>`:''}
function filterFor(theme,o){return theme==='REVENGE'||o.source==='greenery-original.png'||o.id==='studio'?'':` filter="url(#era)"`}
function props(theme){
 const p=palette[theme]
 const content=[`<defs>${eraDefs(theme)}</defs>`]
 for(const o of placements.slice().sort((a,b)=>a.depth_y-b.depth_y)){
  const artFilter=filterFor(theme,o)
  content.push(`<g id="${o.id}" data-depth-y="${o.depth_y}"><image href="${o.source}" x="${o.x}" y="${o.y}" width="${o.width}" height="${o.height}" preserveAspectRatio="none"${artFilter}/></g>`)
 }
 // Small native editable details tie the large raster illustrations to the era.
 for(const [x,y]of [[324,365],[809,316],[799,645]]){
  content.push(`<g id="banner-${x}-${y}" shape-rendering="crispEdges">`,rect(x,y,2,76,p.ink),rect(x-3,y+73,8,3,p.ink),poly([[x+2,y+2],[x+25,y+2],[x+25,y+48],[x+14,y+43],[x+2,y+48]],p.ink),poly([[x+4,y+4],[x+23,y+4],[x+23,y+43],[x+14,y+39],[x+4,y+43]],p.accent))
  if(theme==='DANGER')content.push(poly([[x+15,y+11],[x+9,y+24],[x+14,y+24],[x+11,y+35],[x+21,y+20],[x+15,y+20]],p.ink))
  else if(theme==='PARADE')for(let n=0;n<3;n++)content.push(rect(x+8+n*4,y+16,2,12,p.ink))
  else content.push(`<path d="M${x+14} ${y+12}l7 9-7 9-7-9Z" fill="${p.paper}"/>`)
  content.push('</g>')
 }
 for(const [x,y]of[[12*32+16,19*32],[18*32+16,18*32],[24*32+16,17*32],[32*32+16,17*32]]){
  content.push(`<g id="lamp-${x}-${y}" shape-rendering="crispEdges">`,rect(x-2,y-35,4,34,p.ink),rect(x-6,y-43,12,10,p.ink),rect(x-4,y-41,8,6,p.paper),poly([[x-8,y-43],[x,y-48],[x+8,y-43]],p.ink),rect(x-6,y,12,3,p.ink),'</g>')
 }
 // Signs stay ABOVE blocked foreground geometry; no text or prop covers a target.
 const signs=[{x:432,y:163,label:'STUDIO',w:106},{x:206,y:371,label:'LIVE STAGE',w:86},{x:944,y:321,label:'MERCH / GIFTS',w:100},{x:925,y:631,label:'PIXEL WALL',w:100},{x:420,y:476,label:'PLAZA',w:75}]
 for(const s of signs)content.push(`<g id="label-${s.label.replaceAll(/[^A-Z]/g,'')}" aria-label="${s.label}">`,rect(s.x-s.w/2-2,s.y-12,s.w+4,23,p.ink),rect(s.x-s.w/2,s.y-10,s.w,19,p.paper),`<text x="${s.x}" y="${s.y+4}" text-anchor="middle" font-family="sans-serif" font-weight="900" font-size="11" fill="${p.ink}">${s.label}</text></g>`)
 return content.join('')
}
for(const theme of Object.keys(palette)){
 const n=theme.toLowerCase(),bg=ground(theme),fg=props(theme)
 fs.writeFileSync(path.join(out,`${n}-ground.svg`),svg(bg,`${theme}: editable illustrated ground; immutable virtual AR02`))
 fs.writeFileSync(path.join(out,`${n}-landmarks.svg`),svg(fg,`${theme}: editable landmark image layers and native labels`))
 fs.writeFileSync(path.join(out,`${n}-scene.svg`),svg(bg+fg,`${theme}: original illustrated virtual courtyard`))
 const sprites=[`<defs>${eraDefs(theme)}</defs>`]
 for(const o of placements.filter(o=>o.id!=='greenery_east'&&!o.id.startsWith('perimeter'))){const [x,y,w,h]=o.atlas_frame;sprites.push(`<image id="${o.id}" href="${o.source}" x="${x}" y="${y}" width="${w}" height="${h}" preserveAspectRatio="none"${filterFor(theme,o)}/>`)}
 fs.writeFileSync(path.join(out,`${n}-landmarks-atlas.svg`),`<svg xmlns="http://www.w3.org/2000/svg" width="768" height="512" viewBox="0 0 768 512"><title>${theme} editable isolated landmark frames</title>${sprites.join('')}</svg>`)
 const p=palette[theme],tileSamples=[[1,1],[13,11],[12,9],[1,0],[3,11]]
 const tiles=[`<defs><g id="source-ground">${bg}</g></defs>`]
 tileSamples.forEach(([x,y],i)=>tiles.push(`<svg x="${i*32}" y="0" width="32" height="32" viewBox="${x*32} ${y*32} 32 32"><use href="#source-ground"/></svg>`))
 tiles.push(rect(160,0,32,32,theme==='DANGER'?'#4f8f99':'#65717a'))
 for(let y=2;y<32;y+=8)tiles.push(line(160,y,192,y,p.metal),line(160,y+1,192,y+1,p.ink))
 tiles.push(rect(192,0,32,32,p.stone[3]))
 for(let y=1;y<32;y+=8)for(let x=193;x<222;x+=15)tiles.push(rect(x+(y%16===1?0:5),y,Math.min(13,224-x-(y%16===1?0:5)),6,p.paper))
 tiles.push(rect(224,0,32,32,'#4c8290'))
 for(const[x,y]of[[228,5],[241,11],[231,20],[246,27]])tiles.push(line(x,y,Math.min(255,x+6),y,'#91b9ba'))
 fs.writeFileSync(path.join(out,`${n}-terrain-tiles.svg`),`<svg xmlns="http://www.w3.org/2000/svg" width="256" height="128" viewBox="0 0 256 128"><title>${theme} editable terrain tiles 1-8 only; not a complete C06 theme texture</title>${tiles.join('')}</svg>`)
}
const sources=['frontend/public/world/maps/geometry.json','frontend/public/world/maps/courtyard-tiled.json','frontend/public/world/maps/landmarks.json','docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg']
const binding={kind:'ILLUSTRATED_WORLD_SOURCE_BINDING_PROPOSAL_NOT_RELEASE_MANIFEST',version:1,width_px:W,height_px:H,tile_width_px:32,tile_height_px:32,
 map_id:originalInputs.map_id,map_revision:originalInputs.map_revision,geometry_hash:originalInputs.geometry_hash,
 geometry_semantics:'AR02 remains byte-for-byte immutable. Illustrations do not grant collision, physical location or arrival authority.',
 sources:sources.map(file=>({path:file,sha256:hash(fs.readFileSync(path.join(root,file)))})),
 depth:'Ground at depth -1. Each landmark image is a separate y-sorted object at its depth_y; player foot depth shares the same y ordering. Composite scene PNG is preview/static overview only; do not place the entire landmarks PNG above all players.',
 compositing:{ground:'{era}-ground.png at(0,0)',landmark_objects:'Use individual source rectangles from {era}-landmarks-atlas.png (768x512) with exact placements below',landmark_composite:'{era}-landmarks.png for static map/preview only; contains native banners, lamps and labels',scene:'{era}-scene.png static comparison/overview only'},
 terrain_tiles:{resource:'{era}-terrain-tiles.png',width_px:256,height_px:128,cell_width_px:32,cell_height_px:32,positive_ids:[1,2,3,4,5,6,7,8],purpose:'Reusable terrain art only; IDs9..32 blank. NOT a complete replacement C06 theme texture; illustrated landmark objects replace decorative scenery through separately owned composition.'},
 native_details:{labels:'Native SVG label-* groups attach to corresponding landmark object, sharing its depth. PLAZA label is a ground/DOM label.',banners:'Native banner-x-y groups at(324,365),(809,316),(799,645), base depth=y+76; instance original groups from SVG.',lamps:'Native lamp-x-y groups at(400,608),(592,576),(784,544),(1040,544), base depth=y+3. Source groups remain editable.'},
 placements,interactions:g.interactions,spawns:g.spawns,palette,landmarks,
 exclusions:['No character art or avatar fallback','No operational venue or walking-route claim','No existing theme/map/contract/manifest/root edits','No live collaborative mural state in scenery illustration'],
 status:'UNBOUND source art; final runtime release registry/manifests owned by integrator'}
fs.writeFileSync(path.join(out,'binding-proposal.json'),JSON.stringify(binding,null,2)+'\n')
console.log('Authored 15 editable SVG scenes/layers/atlases/tiles and exact immutable-geometry binding proposal')
