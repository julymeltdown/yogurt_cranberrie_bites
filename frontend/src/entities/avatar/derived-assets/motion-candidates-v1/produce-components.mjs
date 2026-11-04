import assert from'node:assert/strict'
import{readFile,writeFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath,evidencePath}from'./server.mjs'
const hash=b=>createHash('sha256').update(b).digest('hex'),inputs=JSON.parse(await readFile(root+evidencePath+'component-source-inputs.json','utf8')),eyes=JSON.parse(await readFile(root+publicPath+'front-eyes.json','utf8'))
for(const input of Object.values(inputs))assert.equal(hash(await readFile(root+publicPath+input.file)),input.sha256)
const{chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href),server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/tools/pixels.mjs`)
 const result=await page.evaluate(async({inputs,eyes})=>{
  const{decode,pack,register}=await import('/tools/pixels.mjs'),{alphaCut,alphaBounds,gridCuts,normalized,removeCuffFill,canonicalRig,frontPoses,sidePoses}=await import('/tools/component-pixels.mjs'),{articulate}=await import('/tools/rig.mjs'),base='/world/avatars/derived-v1/motion-candidates-v1/',parts=[],items=[],studies=[],palettes=[]
  const skinSource=await decode('/world/avatars/supplied-v1/photo-02.jpg')
  for(let skin=0;skin<8;skin++){const candidates=[],ox=skin%4*301,oy=Math.floor(skin/4)*395;for(let y=135+oy;y<315+oy;y++)for(let x=90+ox;x<285+ox;x++){const i=(y*skinSource.width+x)*4,[r,g,b]=skinSource.rgba.subarray(i,i+3);if(r>60&&r-g>=8&&g-b>=4)candidates.push([r,g,b])}candidates.sort((a,b)=>(a[0]+a[1]+a[2])-(b[0]+b[1]+b[2]));palettes.push({sourceId:'photo_02',bodySourceIndex:skin,method:'ACTUAL_SELECTED_SKIN_RGB_QUANTILES',colors:[.1,.4,.7,.95].map(q=>candidates[Math.floor((candidates.length-1)*q)])})}
  function push(part){const index=parts.length;parts.push(part);return index}
  function phases(part,direction,animated){if(!animated){const index=push(part);return Array(5).fill(index)}return(direction===1||direction===2?sidePoses:frontPoses).map(pose=>push(articulate(part,canonicalRig(direction),pose)))}
  const sourceParts={}
  for(const[kind,input]of Object.entries(inputs)){const source=await decode(base+input.file),grid=gridCuts(source,...input.grid),rows=[];for(let row=0;row<input.grid[1];row++){const views=[];for(let d=0;d<4;d++){const rect={x:grid.x[d],y:grid.y[row],width:grid.x[d+1]-grid.x[d],height:grid.y[row+1]-grid.y[row]},part=alphaCut(source,rect);views.push(part);studies.push({kind,row,direction:d,source:input.file,rect,bounds:part.bounds,attribution:'AUTHORED_RECONSTRUCTION_ALPHA128',sourceView:'NOT_SUPPLIED_ORIGINAL'})}rows.push(views)}sourceParts[kind]=rows}
  // Body skin recoloring is restricted to the authored pale-skin semantic mask.
  // Black outlines/base garments remain bit-identical. Every palette RGB is an
  // actual selected photo_02 flesh sample; this is not a whole-image tint.
  for(let skin=0;skin<8;skin++){const item={key:'body-'+skin,kind:'body',source:'body',skinPalette:palettes[skin],phaseMotion:'ARTICULATED_LIMBS',directions:[]}
   for(let d=0;d<4;d++){const original=sourceParts.body[0][d],copy={...original,rgba:new Uint8ClampedArray(original.rgba)};let semanticSkinPixels=0
    for(let i=0;i<copy.rgba.length;i+=4){const[r,g,b,a]=copy.rgba.subarray(i,i+4);if(a&&r>=150&&g>=90&&b>=65&&r-g>5&&g-b>3){const l=(r+g+b)/3,shade=l<190?0:l<210?1:l<225?2:3;copy.rgba.set(palettes[skin].colors[shade],i);semanticSkinPixels++}}
    const part=normalized(copy,'body',d);item.directions.push({direction:d,frames:phases(part,d,true),semanticSkinPixels,transform:part.transform})
   }items.push(item)
  }
  for(const kind of['hair','outfit','accessory'])for(let row=0;row<sourceParts[kind].length;row++){const item={key:kind+'-'+row,kind,source:kind,sourceRow:row,phaseMotion:kind==='outfit'?'ARTICULATED_LIMBS':'STATIONARY_HEAD_ATTACHMENT',directions:[]}
   for(let d=0;d<4;d++){const raw=sourceParts[kind][row][d];let hand
    if(kind==='outfit'){const result=removeCuffFill(raw,d);hand=normalized(result.hand,kind,d,row);item.cuffPolicy='SEMANTIC_CUFF_FILL_REMOVED_AND_BODY_SKIN_RENDERED_BEHIND';item.directions.push({direction:d,frames:[],handFrames:phases(hand,d,true),removedCuffFill:result.regions})}
    const part=normalized(raw,kind,d,row),entry=kind==='outfit'?item.directions.at(-1):{direction:d};entry.frames=phases(part,d,kind==='outfit');entry.transform=part.transform;if(kind!=='outfit')item.directions.push(entry)
   }items.push(item)
  }
  for(let eye=0;eye<4;eye++){const input=eyes.items[eye],native=await decode(base+input.native.file),b=input.native.bounds,item={key:'eyes-'+eye,kind:'eyes',sourceId:input.sourceId,sourceLocation:input.sourceLocation,phaseMotion:'STATIONARY_HEAD_ATTACHMENT',directions:[]}
   for(let d=0;d<4;d++){if(d===3){const blank={rgba:new Uint8ClampedArray(4096*4),width:64,height:64};item.directions.push({direction:d,frames:phases(blank,d,false),visibility:'NON_VISIBLE_FROM_REAR'});continue}
    let part=native,sourceAnchor={x:b.x+b.width/2,y:b.y+b.height/2},targetAnchor={x:32,y:31}
    if(d!==0){const rgba=new Uint8ClampedArray(native.rgba),mid=b.x+b.width/2;for(let y=0;y<native.height;y++)for(let x=0;x<native.width;x++)if(d===1?x>=mid:x<mid)rgba[(y*native.width+x)*4+3]=0;const bounds=alphaBounds(rgba,native.width,native.height);part={...native,rgba};sourceAnchor={x:bounds.x+bounds.width/2,y:bounds.y+bounds.height/2};targetAnchor={x:d===1?22:42,y:31}}
    const frame=register(part,sourceAnchor,targetAnchor,.11);item.directions.push({direction:d,frames:phases(frame,d,false),transform:frame.transform})
   }items.push(item)
  }
  // Articulated cloth can occlude a cuff edge at the joint. Export the visible
  // cuff mask only; runtime draws it below cloth, so this does not repaint or
  // enlarge a hand or alter the resulting composition.
  for(const item of items.filter(i=>i.kind==='outfit'))for(const view of item.directions){view.occludedHandPixels=[];for(let phase=0;phase<5;phase++){const hand=parts[view.handFrames[phase]].rgba,cloth=parts[view.frames[phase]].rgba;let removed=0,visible=0;for(let i=0;i<hand.length;i+=4)if(hand[i+3]){if(cloth[i+3]){hand[i+3]=0;removed++}else visible++}if(!visible)throw Error('Fully occluded cuff '+item.key+'/'+view.direction+'/'+phase);view.occludedHandPixels.push(removed)}}
  return{items,palettes,studies,atlas:pack(parts,20),physicalFrames:parts.length}
 },{inputs,eyes})
 const b=Buffer.from(result.atlas.png,'base64'),sha256=hash(b),file=`layered-wardrobe-${sha256.slice(0,16)}.png`;await writeFile(root+publicPath+file,b);delete result.atlas.png
 const manifest={kind:'SELECTED_SOURCE_COMPOSABLE_WARDROBE_CANDIDATE',status:'IN_PROGRESS',productionEligible:false,sourcePack:eyes.sourcePack,frameSize:[64,64],foot:[32,60],directions:['front','left','right','rear'],phases:5,counts:{body:8,hair:6,eyes:4,outfit:6,accessory:4},items:result.items,palettes:result.palettes,sourceViews:result.studies,authoredInputs:inputs,atlas:{...result.atlas,file,sha256,bytes:b.length},physicalFrames:result.physicalFrames,composition:{order:['body-head-or-complete-base','outfit-cuff-skin','outfit','eyes','hair','accessory'],outfitBodyClipY:42,hands:'Color the explicit cuff mask with selected body skin palette before outfit; no source-baked hand colors.',headAttachments:'Static UV shared across gait phases because heads remain stationary; never a repeated full-body walk.'},qualification:'Actual four-direction composable candidate layers with source-palette skin isolation and articulated body/outfit limbs. Authored reconstruction, explicit cuff alpha; not C06 registry, inventory, production renderer or release admission.'}
 await writeFile(root+publicPath+'wardrobe-release-candidate.json',JSON.stringify(manifest,null,2)+'\n');console.log(JSON.stringify({result:'GENERATED_NOT_ADMITTED',counts:manifest.counts,physicalFrames:manifest.physicalFrames,file,bytes:b.length}))
}finally{await browser?.close();await new Promise(r=>server.close(r))}
