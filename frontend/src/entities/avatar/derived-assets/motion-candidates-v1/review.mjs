const base = '/world/avatars/derived-v1/motion-candidates-v1/'
const manifest = await (await fetch(base + 'candidate-index.json')).json()
if (manifest.productionEligible !== false || manifest.walkFrames.length || manifest.fullAvatarDirections.length) throw Error('Unexpected candidate authority')
const atlas = new Image(); atlas.src = base + manifest.atlas.file; await atlas.decode()
const attachments=await(await fetch(base+'front-attachments.json')).json(),attachmentAtlas=new Image();attachmentAtlas.src=base+attachments.atlas.file;await attachmentAtlas.decode()
const accessory=document.querySelector('#accessory')
attachments.items.forEach((item,index)=>{const option=document.createElement('option');option.value=String(index);option.textContent=item.label;accessory.append(option)})
const eyeLayers=await(await fetch(base+'front-eyes.json')).json(),eyeAtlas=new Image();eyeAtlas.src=base+eyeLayers.atlas.file;await eyeAtlas.decode()
const eyes=document.querySelector('#eyes');eyeLayers.items.forEach((item,index)=>{const option=document.createElement('option');option.value=String(index);option.textContent=item.label;eyes.append(option)})
const garments=await(await fetch(base+'front-garments.json')).json(),garmentAtlas=new Image();garmentAtlas.src=base+garments.atlas.file;await garmentAtlas.decode()
const body = document.querySelector('#body'), hair = document.querySelector('#hair'), theme = document.querySelector('#theme'), large = document.querySelector('#large')
const bodies = manifest.items.map((item, index) => ({ item, index })).filter(p => p.item.sourceId === 'photo_02')
const hairs = manifest.items.map((item, index) => ({ item, index })).filter(p => p.item.sourceId === 'photo_04')
for (const [select, entries] of [[body, bodies], [hair, hairs]]) for (const { item, index } of entries) { const option = document.createElement('option'); option.value = String(index); option.textContent = item.sourceId + ' ' + item.location; select.append(option) }
hair.value = String(hairs[0].index)
const surfaces = [['World',64],['Profile',72],['Feed',56],['Gift',64],['Party',64]]
for (const [name, size] of surfaces) {
  const figure = document.createElement('figure'), canvas = document.createElement('canvas'), caption = document.createElement('figcaption')
  figure.className = 'sample';canvas.width = 64;canvas.height = 64;canvas.style.width = size + 'px';canvas.style.height = size + 'px';canvas.setAttribute('aria-label',name + ' candidate preview');caption.textContent = name + ' · ' + size + 'px';figure.append(canvas,caption);document.querySelector('.samples').append(figure)
}
const allCanvases = [large, ...document.querySelectorAll('.sample canvas')]
function drawFrame(ctx, index) { ctx.drawImage(atlas,index % 8 * 64,Math.floor(index / 8) * 64,64,64,0,0,64,64) }
function drawAttachment(ctx,index){ctx.drawImage(attachmentAtlas,index*64,0,64,64,0,0,64,64)}
function drawEyes(ctx,index){ctx.drawImage(eyeAtlas,index*64,0,64,64,0,0,64,64)}
function render() {
  for (const canvas of allCanvases) { const ctx = canvas.getContext('2d',{willReadFrequently:true});ctx.clearRect(0,0,64,64);ctx.imageSmoothingEnabled = false;drawFrame(ctx,Number(body.value));if(eyes.value!=='none')drawEyes(ctx,Number(eyes.value));if(hair.value !== 'none')drawFrame(ctx,Number(hair.value));if(accessory.value!=='none')drawAttachment(ctx,Number(accessory.value)) }
  document.documentElement.style.setProperty('--terrain',`url('/world/themes/${theme.value.toLowerCase()}.png')`)
  document.querySelector('#state').textContent = 'Front only · foot anchor (32,60) · source pixels unchanged · ' + theme.value
}
body.onchange = hair.onchange = eyes.onchange = accessory.onchange = theme.onchange = render
let referencesRendered = false
document.querySelector('details').addEventListener('toggle',()=>{
  if (!document.querySelector('details').open || referencesRendered) return
  referencesRendered = true
  for(const [index,item] of manifest.items.entries()) {const figure=document.createElement('figure'),canvas=document.createElement('canvas'),caption=document.createElement('figcaption');figure.className='part';canvas.width=64;canvas.height=64;drawFrame(canvas.getContext('2d'),index);caption.textContent=item.sourceId+' '+item.location+' · '+item.registered.kind;figure.append(canvas,caption);document.querySelector('.parts').append(figure)}
  for(const[index,item]of garments.items.entries()){const f=document.createElement('figure'),c=document.createElement('canvas'),l=document.createElement('figcaption');f.className='garment';f.style.margin='0';c.width=c.height=64;c.getContext('2d').drawImage(garmentAtlas,index*64,0,64,64,0,0,64,64);l.textContent='Hands cut · '+item.sourceLocation+' · UNFITTED';f.append(c,l);document.querySelector('.parts').append(f)}
})
render()
Object.assign(window,{__MOTION__:{manifest,render,canvases:allCanvases,drawFrame,atlas,attachments,drawAttachment,eyeLayers,drawEyes,eyeAtlas}})
