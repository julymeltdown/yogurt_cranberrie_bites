const read=async path=>{const response=await fetch(path);if(!response.ok)throw Error(path);return response.json()}
const proposal=await read('../spatial-v3/geometry-proposal.json'),canvas=document.querySelector('#scene'),cache=new Map()
let era='parade',view='world',current
const decode=async path=>{const i=new Image();i.src=path;await i.decode();return i}
async function load(era){
 if(!cache.has(era))cache.set(era,(async()=>{
  const folder=`./${era}/`,layout=await read(folder+'layout.json')
  const [ground,atlas,meta]=await Promise.all([decode(folder+layout.ground),decode(folder+layout.atlas),read(folder+'map-metadata.json')])
  return {layout,ground,atlas,meta}
 })());return cache.get(era)
}
function draw(){
 const {layout,ground,atlas}=current,camera=view==='world'?layout.world_camera:layout.overview_camera
 canvas.width=Math.round(canvas.getBoundingClientRect().width);canvas.height=Math.round(canvas.width*camera.height/camera.width)
 const c=canvas.getContext('2d');c.imageSmoothingEnabled=false;c.scale(canvas.width/camera.width,canvas.height/camera.height);c.translate(-camera.x,-camera.y);c.drawImage(ground,0,0)
 if(view==='map'){
  c.beginPath();proposal.plaza_parcel.forEach(([x,y],i)=>i?c.lineTo(x,y):c.moveTo(x,y));c.closePath();c.fillStyle='#7a8742';c.fill();c.strokeStyle='#b4a27d';c.lineWidth=6;c.stroke()
 }
 const reps=view==='map'?layout.overview_representations:[],replaced=new Set(reps.map(o=>o.replaces_prop_id).filter(Boolean))
 for(const o of layout.placements.filter(o=>!replaced.has(o.id)).concat(reps).sort((a,b)=>a.depth_y-b.depth_y)){
  const f=layout.frames[o.frame];c.drawImage(atlas,f.x,f.y,f.width,f.height,o.x,o.y,o.width,o.height)
 }
 c.setTransform(1,0,0,1,0,0)
 if(view==='map')for(const label of current.meta.map_labels){
  const words=label.label.includes('ZONE')?['MEETUP','ZONE']:label.label.includes('BOOTH')?['GIVEAWAY','BOOTH']:[label.label]
  c.font='bold 12px system-ui';c.textAlign='center';c.textBaseline='middle'
  const w=Math.max(...words.map(w=>c.measureText(w).width))+12,h=words.length*15+8
  const x=Math.max(w/2+3,Math.min(canvas.width-w/2-3,label.x/camera.width*canvas.width)),y=label.y/camera.height*canvas.height
  c.fillStyle='#332c31';c.fillRect(x-w/2-1,y-h/2-1,w+2,h+2);c.fillStyle='#f3ead8';c.fillRect(x-w/2,y-h/2,w,h);c.fillStyle='#252028'
  words.forEach((word,i)=>c.fillText(word,x,y+(i-(words.length-1)/2)*15))
 }
 const box=view==='world'?{x:597,y:94,w:354,h:360}:{x:975,y:95,w:248,h:420},reference=document.querySelector('#reference'),target=document.querySelector('#target'),scale=canvas.width/box.w
 reference.style.height=(box.h*scale)+'px';target.style.width=(1536*scale)+'px';target.style.height=(1024*scale)+'px';target.style.left=(-box.x*scale)+'px';target.style.top=(-box.y*scale)+'px'
 window.eraSceneReview={ready:true,era,view,geometryHash:layout.geometry_hash,ground:[ground.width,ground.height],atlas:[atlas.width,atlas.height],physicalObjects:layout.placements.length,mapRepresentations:layout.overview_representations.length,resources:2,cacheEntries:cache.size}
}
window.eraSceneOcclusion=()=>{
 const a=document.createElement('canvas');a.width=current.atlas.width;a.height=current.atlas.height;const c=a.getContext('2d',{willReadFrequently:true});c.drawImage(current.atlas,0,0)
 return [...proposal.geometry.spawns,...proposal.geometry.interactions].map(p=>{
  const foot={x:(p.cell_x+.5)*16,y:(p.cell_y+.5)*16},blockers=[]
  for(const o of current.layout.placements){if(foot.y>=o.depth_y||foot.x<o.x||foot.x>=o.x+o.width||foot.y<o.y||foot.y>=o.y+o.height)continue
   const f=current.layout.frames[o.frame],x=Math.floor(f.x+(foot.x-o.x)*f.width/o.width),y=Math.floor(f.y+(foot.y-o.y)*f.height/o.height)
   if(c.getImageData(x,y,1,1).data[3]>128)blockers.push(o.id)
  }return {id:p.spawn_id??p.interaction_id,blockers}
 })
}
async function update(){window.eraSceneReview={ready:false};current=await load(era);draw()}
document.querySelector('#era').onchange=async e=>{era=e.target.value;await update()}
for(const b of document.querySelectorAll('[data-view]'))b.onclick=()=>{view=b.dataset.view;for(const x of document.querySelectorAll('[data-view]'))x.setAttribute('aria-pressed',String(x===b));draw()}
window.addEventListener('resize',()=>{if(current)draw()});await update()
