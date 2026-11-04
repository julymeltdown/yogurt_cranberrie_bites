const base='/world/avatars/derived-v1/motion-candidates-v1/'
const release=await(await fetch(base+'combined-release-candidate.json')).json(),manifest=release.presets
if(manifest.productionEligible!==false||manifest.frames.length!==120||manifest.directions.join()!=='front,left,right,rear')throw Error('Unexpected gait authority')
const atlas=new Image();atlas.src=base+manifest.atlas.file;await atlas.decode()
const direction=document.querySelector('#direction'),preset=document.querySelector('#preset'),theme=document.querySelector('#theme'),play=document.querySelector('#play'),large=document.querySelector('#large')
for(let i=0;i<6;i++){const o=document.createElement('option');o.value=String(i);o.textContent=manifest.frames[i*20].preset;preset.append(o)}
for(const[name,size]of[['World',64],['Profile',72],['Feed',56],['Gift',64],['Party',64]]){const f=document.createElement('figure'),c=document.createElement('canvas'),l=document.createElement('figcaption');f.className='sample';c.width=c.height=64;c.style.width=c.style.height=size+'px';l.textContent=name+' · '+size+'px';f.append(c,l);document.querySelector('.samples').append(f)}
for(let i=0;i<5;i++){const f=document.createElement('figure'),c=document.createElement('canvas'),l=document.createElement('figcaption');c.width=c.height=64;l.textContent=i===0?'Idle':'Walk '+(i-1);f.append(c,l);document.querySelector('.strip').append(f)}
const canvases=[large,...document.querySelectorAll('.sample canvas')],strips=[...document.querySelectorAll('.strip canvas')]
let phase=0,playing=false,last=0,draws=0
function frame(ctx,p){const index=Number(preset.value)*20+Number(direction.value)*5+p;ctx.clearRect(0,0,64,64);ctx.imageSmoothingEnabled=false;ctx.drawImage(atlas,index%20*64,Math.floor(index/20)*64,64,64,0,0,64,64)}
function render(){for(const c of canvases)frame(c.getContext('2d',{willReadFrequently:true}),phase);strips.forEach((c,p)=>frame(c.getContext('2d'),p));document.documentElement.style.setProperty('--terrain',`url('/world/themes/${theme.value.toLowerCase()}.png')`);document.querySelector('#state').textContent=manifest.frames[Number(preset.value)*20+Number(direction.value)*5+phase].pose+' · '+manifest.directions[Number(direction.value)]+' · '+(Number(direction.value)===0?'supplied pixels':'authored reconstruction');draws++}
function setPhase(p){if(!Number.isInteger(p)||p<0||p>4)throw Error('Invalid phase');phase=p;render()}
function tick(time){if(playing&&time-last>=140){last=time;phase=phase===4?1:phase+1;render()}requestAnimationFrame(tick)}
preset.onchange=direction.onchange=theme.onchange=render;play.onclick=()=>{playing=!playing;play.textContent=playing?'Pause walk':'Play walk';if(!playing)setPhase(0)}
render();requestAnimationFrame(tick)
window.__DIRECTION__={manifest,atlas,canvases,setPhase,get draws(){return draws},get phase(){return phase}}
