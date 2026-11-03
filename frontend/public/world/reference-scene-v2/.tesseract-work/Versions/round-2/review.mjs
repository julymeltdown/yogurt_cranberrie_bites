const read = name => fetch('./'+name).then(r=>{if(!r.ok)throw Error(name);return r.json()})
const proposal=await read('geometry-proposal.json'),painted=await read('painted/layout.json'),canvas=document.querySelector('#scene'),ctx=canvas.getContext('2d'),reference=document.querySelector('#reference'),target=document.querySelector('#target')
let view='world',theme='parade',finish='painted',debug=false,imageSet
const imageCache=new Map()
const decode=async path=>{const image=new Image();image.src=path;await image.decode();return image}
async function images(){const key=finish+theme;if(!imageCache.has(key))imageCache.set(key,finish==='painted'?Promise.all([decode('./painted/parade-ground.png'),decode('./painted/parade-source-atlas.png')]).then(([ground,atlas])=>({ground,atlas,frames:painted.frames,placements:painted.placements})):Promise.all([decode(`./${theme}-ground.png`),decode(`./${theme}-scenery.png`),read(theme+'-frames.json')]).then(([ground,atlas,frames])=>({ground,atlas,frames,placements:proposal.placements})));return imageCache.get(key)}
function paint(targetCanvas,camera,labels=false){
 const c=targetCanvas.getContext('2d');c.imageSmoothingEnabled=false;c.clearRect(0,0,targetCanvas.width,targetCanvas.height)
 const sx=targetCanvas.width/camera.width,sy=targetCanvas.height/camera.height
 c.save();c.scale(sx,sy);c.translate(-camera.x,-camera.y);c.drawImage(imageSet.ground,0,0)
 for(const object of [...imageSet.placements].sort((a,b)=>a.depth_y-b.depth_y)){
  const frame=imageSet.frames[object.frame];c.drawImage(imageSet.atlas,frame.x,frame.y,frame.width,frame.height,object.x,object.y,object.width??frame.width,object.height??frame.height)
 }
 if(debug){
  c.globalAlpha=.4;c.fillStyle='#ec3939';for(let y=0;y<proposal.geometry.height_cells;y++)for(let x=0;x<proposal.geometry.width_cells;x++)if(proposal.geometry.collision_grid[y][x])c.fillRect(x*16,y*16,16,16)
  c.globalAlpha=1;c.strokeStyle='#1dd9df';c.lineWidth=2
  for(const o of proposal.placements){c.beginPath();o.footprint.forEach(([x,y],i)=>i?c.lineTo(x,y):c.moveTo(x,y));c.closePath();c.stroke()}
  for(const p of proposal.geometry.spawns){c.fillStyle='#67e2ff';c.fillRect(p.cell_x*16+4,p.cell_y*16+4,8,8)}
 }
 c.restore()
 if(labels){
  c.font='bold 14px system-ui';c.textAlign='center';c.textBaseline='middle'
  for(const item of proposal.map_labels){
   const words=item.label.includes('ZONE')?['MEETUP','ZONE']:item.label.includes('BOOTH')?['GIVEAWAY','BOOTH']:[item.label]
   const w=Math.max(...words.map(x=>c.measureText(x).width))+16,h=words.length*17+10
   const x=Math.max(w/2+3,Math.min(targetCanvas.width-w/2-3,(item.x-camera.x)*sx)),y=(item.y-camera.y)*sy
   c.fillStyle='#221f25';c.fillRect(x-w/2-2,y-h/2-2,w+4,h+4);c.fillStyle='#f0e5cf';c.fillRect(x-w/2,y-h/2,w,h);c.fillStyle='#18171b'
   words.forEach((word,i)=>c.fillText(word,x,y+(i-(words.length-1)/2)*17))
  }
 }
}
function cropReference(){
 const box=view==='world'?{x:597,y:94,w:354,h:360}:{x:975,y:95,w:248,h:420}
 reference.className='reference'+(view==='map'?' map-reference':'');reference.style.aspectRatio=`${box.w}/${box.h}`
 const scale=reference.getBoundingClientRect().width/box.w;target.style.width=`${1536*scale}px`;target.style.height=`${1024*scale}px`;target.style.left=`${-box.x*scale}px`;target.style.top=`${-box.y*scale}px`
}
async function draw(){
 window.referenceSceneReview={ready:false};imageSet=await images()
 const camera=view==='world'?proposal.world_camera:proposal.overview_camera
 canvas.width=Math.round(canvas.getBoundingClientRect().width);canvas.height=Math.round(canvas.width*camera.height/camera.width)
 paint(canvas,camera,view==='map');cropReference()
 const text=view==='world'?'Stage-led courtyard close view':'Portrait diagonal overview; labels are virtual'
 document.querySelector('#state').textContent=text
 document.querySelector('#summary').textContent=JSON.stringify({geometry_hash:proposal.geometry_hash,resources:2,objects:proposal.placements.length,spawns:proposal.geometry.spawns.length,interactions:proposal.geometry.interactions.length,releaseBinding:'NONE'},null,2)
 window.referenceSceneReview={ready:true,view,theme,finish,geometryHash:proposal.geometry_hash,width:canvas.width,height:canvas.height,ground:[imageSet.ground.naturalWidth,imageSet.ground.naturalHeight],atlas:[imageSet.atlas.naturalWidth,imageSet.atlas.naturalHeight]}
}
for(const button of document.querySelectorAll('[data-view]'))button.onclick=async()=>{view=button.dataset.view;for(const b of document.querySelectorAll('[data-view]'))b.setAttribute('aria-pressed',String(b===button));await draw()}
document.querySelector('#theme').onchange=async event=>{theme=event.target.value;await draw()}
document.querySelector('#finish').onchange=async event=>{finish=event.target.value;theme='parade';document.querySelector('#theme').value='parade';for(const option of document.querySelector('#theme').options)option.disabled=finish==='painted'&&option.value!=='parade';await draw()}
document.querySelector('#geometry').onclick=async event=>{debug=!debug;event.target.setAttribute('aria-pressed',String(debug));await draw()}
window.addEventListener('resize',draw)
window.referenceSceneExport=()=>{const output=document.createElement('canvas');output.width=proposal.coordinates.width_px;output.height=proposal.coordinates.height_px;paint(output,proposal.overview_camera,false);return output.toDataURL('image/png')}
window.referenceSceneOcclusion=()=>{
 const out=[];const atlas=document.createElement('canvas');atlas.width=imageSet.atlas.naturalWidth;atlas.height=imageSet.atlas.naturalHeight;const a=atlas.getContext('2d');a.drawImage(imageSet.atlas,0,0)
 for(const p of [...proposal.geometry.spawns,...proposal.geometry.interactions]){
  const foot={x:(p.cell_x+.5)*16,y:(p.cell_y+.5)*16};const blockers=[]
  for(const o of imageSet.placements){if(foot.y>=o.depth_y)continue;const f=imageSet.frames[o.frame],w=o.width??f.width,h=o.height??f.height
   if(foot.x<o.x||foot.x>=o.x+w||foot.y<o.y||foot.y>=o.y+h)continue
   const x=Math.floor(f.x+(foot.x-o.x)*f.width/w),y=Math.floor(f.y+(foot.y-o.y)*f.height/h)
   if(a.getImageData(x,y,1,1).data[3]>128)blockers.push(o.id)
  }
  out.push({id:p.spawn_id??p.interaction_id,foot,blockers})
 }
 return out
}
await draw()
