// Original deterministic print surface source. No image/reference sampling or remote dependency.
import {writeFile} from 'node:fs/promises'
let seed=460711
const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296}
const svg=(w,h,body)=>`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none">${body}</svg>\n`
let dust=''
for(let i=0;i<2900;i++){
 const x=random()*600,y=random()*960,r=.18+random()**4*1.55,alpha=.04+random()**3*.23
 dust+=`<ellipse cx="${x.toFixed(2)}" cy="${y.toFixed(2)}" rx="${r.toFixed(2)}" ry="${(r*(.5+random())).toFixed(2)}" fill="#4c3b30" opacity="${alpha.toFixed(2)}"/>`
}
const grain='<defs><filter id="paper"><feTurbulence type="fractalNoise" baseFrequency=".62" numOctaves="3" seed="31" stitchTiles="stitch"/><feColorMatrix type="saturate" values="0"/></filter><filter id="wash"><feTurbulence type="fractalNoise" baseFrequency=".018" numOctaves="4" seed="11" stitchTiles="stitch"/><feColorMatrix type="saturate" values="0"/></filter></defs><path fill="#fbf2e8" d="M0 0h600v960H0z"/><path d="M0 0h600v960H0z" filter="url(#wash)" opacity=".14"/><path d="M0 0h600v960H0z" filter="url(#paper)" opacity=".29"/>'
await writeFile(new URL('./paper-print.svg',import.meta.url),svg(600,960,grain+dust))
// A symmetric distressed perimeter is the ordinary window frame, not an accent rail.
let frame='<path fill="none" stroke="#24201c" stroke-width="2" d="M3 3H597V957H3Z"/>'
for(let i=0;i<420;i++){
 const side=i%4,t=random(),x=side===0?4:side===1?596:4+t*592,y=side===2?4:side===3?956:4+t*952
 frame+=`<path d="M${x.toFixed(1)} ${y.toFixed(1)}l${(random()*5-2.5).toFixed(1)} ${(random()*5-2.5).toFixed(1)}" fill="none" stroke="${i%3?'#eee5d6':'#26211c'}" stroke-width="${(.4+random()*1.5).toFixed(1)}"/>`
}
await writeFile(new URL('./frame-print.svg',import.meta.url),svg(600,960,frame))
let wear=''
for(let i=0;i<95;i++){
 const x=random()*600,y=random()*110
 wear+=`<ellipse cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" rx="${(.4+random()*1.8).toFixed(1)}" ry="${(.2+random()*.9).toFixed(1)}" fill="${i%4?'#3f1417':'#f5e5d2'}" opacity="${(.08+random()*.25).toFixed(2)}"/>`
}
await writeFile(new URL('./red-ink.svg',import.meta.url),svg(600,110,wear))
console.log('Auth print SVG surfaces written')
