const read=async path=>{const response=await fetch(path);if(!response.ok)throw Error(path);return response.json()}
const proposal=await read('../spatial-v5/geometry-proposal.json'),canvas=document.querySelector('#scene'),cache=new Map()
const presentation=await read('./map-label-presentation.json');await document.fonts.load('40px \"MCR Map Labels\"')
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
 if(view==='map')drawLabels(c,canvas.width/camera.width)
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
function polygon(c,points,color){c.beginPath();points.forEach(([x,y],i)=>i?c.lineTo(x,y):c.moveTo(x,y));c.closePath();c.fillStyle=color;c.fill()}
function drawLabels(c,scale){
 const s=presentation.style;c.save();c.scale(scale,scale)
 const rect=(x,y,w,h)=>[[x,y],[x+w,y],[x+w,y+h],[x,y+h]]
 for(const l of presentation.labels){
  const r=l.rect,b=s.border_width
  polygon(c,rect(r.x,r.y,r.width,r.height),s.border_fill)
  polygon(c,rect(r.x+b,r.y+b,r.width-2*b,r.height-2*b),s.paper_fill)
  if(l.icon){
   const ix=r.x+s.padding,iy=r.y+(r.height-s.icon_size)/2,k=s.icon_size/24
   polygon(c,rect(ix-3,iy-3,s.icon_size+6,s.icon_size+6),s.icon_paper)
   for(const p of presentation.icons[l.icon])polygon(c,p.map(([x,y])=>[ix+x*k,iy+y*k]),s.icon_fill)
   for(const p of presentation.icon_cutouts[l.icon]||[])polygon(c,p.map(([x,y])=>[ix+x*k,iy+y*k]),s.icon_paper)
  }
  const x=r.x+s.padding+(l.icon?s.icon_cell:0),y=r.y+(r.height-l.lines.length*s.line_height)/2
  c.font=`${s.font_size}px "MCR Map Labels"`;c.textAlign='left';c.textBaseline='alphabetic';c.fillStyle=s.text_fill
  l.lines.forEach((t,i)=>c.fillText(t,x,y+32+i*s.line_height))
 }
 c.restore()
}
window.labelReview=()=>{
 const s=presentation.style,c=canvas.getContext('2d');c.font=`${s.font_size}px "MCR Map Labels"`
 return {fontLoaded:document.fonts.check('40px "MCR Map Labels"'),fontFamily:s.font_family,labels:presentation.labels.map(l=>({label:l.label,rect:l.rect,icon:l.icon,interaction_id:l.interaction_id,lines:l.lines.map(t=>({text:t,width:c.measureText(t).width})),availableTextWidth:l.rect.width-2*s.padding-(l.icon?s.icon_cell:0)}))}
}
window.addEventListener('resize',()=>{if(current)draw()});await update()
