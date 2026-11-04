// Test oracle: exact artist review.mjs polygon/drawLabels functions at f27cc114.
import presentation from '../../../../../../public/world/reference-scene-v2/map-service-labels-v1/label-presentation.json'
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
export { drawLabels }
