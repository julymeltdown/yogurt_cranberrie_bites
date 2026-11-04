/** Pure source qualification; no consumer, registry, network or renderer wiring. */
import {decodePng} from './png.ts'
export type Pin={path:string;bytes:number;sha256:string}
type Rect=readonly[number,number,number,number]
type Row={id:string;frame:string;rect:Rect;depth_y:number;crop_role:string}
type Crop={themeId:string;frame:string;sourceRect:{x:number;y:number;width:number;height:number};rgbaSha256:string;atlas:Pin}
type Operation={kind:'rect'|'clear';rect:Rect;rgba?:readonly number[]}
type Component={id:string;role:'negative_gap'|'recessed_bay';rect:Rect;operations:Operation[]}
export interface Layout { [key:string]:unknown;version:string;decorations:Row[];serviceLabelAnchors:{exclusionRect:{x:number;y:number;width:number;height:number}}[];canonicalDisplayAnchors:{x:number;y:number}[];roofStreetDerivation:{roofPlansByTheme:Record<string,{roofDomain:Rect}[]>};facadeDerivation?:{sourceCommit:string;allowedBounds:Record<string,Rect>;componentsByTheme:Record<string,Record<string,Component[]>>;groundBinding:Pin} }
export interface Proof {
 version:string;sourceCommit:string;baselinePins:Pin[];sourceInputs:Pin[];originalCropPins:Crop[];additionCropPins:unknown[];
 helperPin:Pin;groundBinding:Pin;allowedBounds:Record<string,Rect>;componentsByTheme:Record<string,Record<string,Component[]>>;
 facadeMaskPins:Record<string,Record<string,{bytes:number;sha256:string;pixels:number}>>;
 groundMaskPins:Record<string,{sha256:string;pixels:number}>;
 actualByTheme:Record<string,{changedPixels:number;facadeChangedPixels:number;groundChangedPixels:number;trueGaps:{id:string;rect:Rect;underlyingRgbSha256:string}[]}>;
 incumbentDecorationsExact:number;protectedLabelRectanglesExact:number;policy:string;recipe:Pin;outputs:Pin[];
 frozenV2RecompositionExactEras:number;
}
type GroundOperation={id:string;kind:'rect';x:number;y:number;width:number;height:number;fill:string}
export interface Ground {
 format:string;sourceCommit:string;dimensions:readonly number[];baselinePins:Pin[];helperPin:{path:string;sha256:string};provenancePin:Pin;
 qualification:{sourceCommit:string;receipt:Pin;status:string};
 eras:Record<string,{overlay:Pin;svg:Pin;operationsPin:Pin;allowedMaskPin:Pin;operations:GroundOperation[];allowedMaskSha256:string;allowedPixels:number;changedPixels:number}>;
}
const SOURCE='da33467cd084ecba071244a1939b6217f4b0de02',PREFIX='frontend/public/world/map-facade-derived-v3/',BASE='frontend/public/world/map-roof-street-derived-v2/'
const ERAS=['REVENGE','PARADE','DANGER'] as const,W=768,H=1296
export const BOUNDS:Record<string,Rect>={map_only_stage_west_wing:[159,361,164,33],map_only_stage_central_hall:[307,361,199,41],map_only_stage_east_wing:[456,362,161,33]}
const FROZEN:Record<string,string>={
  "README.md": "5994b8074a7c201b001bb0ebe04bec6b39deb4826f5790c41b04fb83c4684311",
  "build.py": "4c192b13a967bb490a1318f5a6cce0944e5d52ee91e33f1bdc233432b0ae1314",
  "danger-scene.png": "b4dbf3f41e392e2f37ce78f907336055938c4b6611925c79ad13f41b15d2e931",
  "danger-scene.svg": "995d576574c0221d61599db395061f26b8a0208db97cc97775e01e764ce1c513",
  "layout.json": "d9e401f6254df05530174b04b16655aa5f89fed68fa02c90f6a653e6a8010e91",
  "parade-scene.png": "d820853d14efe04f84cd8b8f08da5bc8c67850f3b19c43170b237207583a93c2",
  "parade-scene.svg": "956f6a6aab8ceec3756d4ad60a45aeed9b1b67a99ebd31fcf5f08f8b867dbcc3",
  "revenge-scene.png": "5669a7861851a1b95bd1f2ad48e222f2e73dcd471622405f48000a33419f1606",
  "revenge-scene.svg": "6c1d91478f8131e347f96b0a8a5c186618196d282fac366afe861e6080ce8e8b",
  "source-provenance.json": "259a407d711e520e4abc5c0d8c3259cf118d3511d9effd9198521a5b7d996990"
}
const same=(a:unknown,b:unknown)=>JSON.stringify(a)===JSON.stringify(b)
const hexsha=(v:string)=>v.replace(/^sha256:/,'')
const pointIn=(x:number,y:number,r:Rect)=>x>=r[0]&&y>=r[1]&&x<r[0]+r[2]&&y<r[1]+r[3]
const subset=(a:Rect,b:Rect)=>a[2]>0&&a[3]>0&&pointIn(a[0],a[1],b)&&pointIn(a[0]+a[2]-1,a[1]+a[3]-1,b)
function projection(row:Row,crop:Crop) {
 const [x,y,w,h]=row.rect,sw=crop.sourceRect.width,sh=crop.sourceRect.height,scale=Math.min(w/sw,h/sh)
 const rw=Math.max(1,Math.floor(sw*scale+.5)),rh=Math.max(1,Math.floor(sh*scale+.5))
 const venue=['meeting_pavilion','giveaway_pavilion','north_hall','west_hall'].includes(row.frame)
 const trim=venue?Math.floor(sh*.54):0,fh=venue?Math.max(1,Math.floor(rh*.46*.8+.5)):rh
 return {dx:x+Math.floor((w-rw)/2),dy:y+h-fh,rw,fh,trim,sw,sh}
}
async function digest(bytes:Uint8Array) { return 'sha256:'+Buffer.from(await crypto.subtle.digest('SHA-256',Uint8Array.from(bytes))).toString('hex') }
/** Reconstruct original projected alpha independently of the Python compositor. */
export function facadeMasks(base:Layout,original:Proof,era:string,atlas:ReturnType<typeof decodePng>) {
 const crops=new Map(original.originalCropPins.filter(p=>p.themeId===era).map(p=>[p.frame,p]))
 const projected=base.decorations.map(row=>({row,crop:crops.get(row.frame)!,p:projection(row,crops.get(row.frame)!)}))
 function opaque(index:number,x:number,y:number) {
  const {crop,p}=projected[index]!
  if(x<p.dx||y<p.dy||x>=p.dx+p.rw||y>=p.dy+p.fh)return false
  const sx=crop.sourceRect.x+Math.min(p.sw-1,Math.floor((x-p.dx+.5)*p.sw/p.rw))
  const sy=crop.sourceRect.y+p.trim+Math.min(p.sh-p.trim-1,Math.floor((y-p.dy+.5)*(p.sh-p.trim)/p.fh))
  return atlas.rgba[(sy*atlas.width+sx)*4+3]!==0
 }
 const masks:Record<string,Uint8Array>={}
 for(const [id,bound]of Object.entries(BOUNDS)) {
  const index=base.decorations.findIndex(r=>r.id===id),mask=new Uint8Array(W*H)
  if(index<0)throw new Error('Missing named facade: '+id)
  for(let y=bound[1];y<bound[1]+bound[3];y++)for(let x=bound[0];x<bound[0]+bound[2];x++) {
   if(!opaque(index,x,y))continue
   if(base.roofStreetDerivation.roofPlansByTheme[era]!.some(p=>pointIn(x,y,p.roofDomain)))continue
   if(base.serviceLabelAnchors.some(({exclusionRect:r})=>pointIn(x,y,[r.x,r.y,r.width,r.height])))continue
   if(base.canonicalDisplayAnchors.some(a=>Math.hypot(x+.5-a.x,y+.5-a.y)<=45))continue
   let foreground=false
   for(let next=index+1;next<projected.length;next++)if(opaque(next,x,y)){foreground=true;break}
   if(!foreground)mask[y*W+x]=255
  }
  masks[id]=mask
 }
 return masks
}
function pinsMatch(pins:Pin[],expected:Record<string,string>) {
 return pins.length===Object.keys(expected).length&&new Set(pins.map(p=>p.path)).size===pins.length&&Object.entries(expected).every(([n,h])=>pins.some(p=>p.path===BASE+n&&hexsha(p.sha256)===h))
}
export function qualifyFacade(base:Layout,candidate:Layout,proof:Proof,original:Proof,ground:Ground):readonly string[] {
 const reasons=new Set<string>(),fail=(s:string)=>{reasons.add(s)}
 if(proof.sourceCommit!==SOURCE||proof.version!=='map-facade-derived-v3'||candidate.version!==proof.version||candidate.facadeDerivation?.sourceCommit!==SOURCE)fail('SOURCE_IDENTITY')
 for(const key of Object.keys(base))if(!['version','recipeRevision','assets'].includes(key)&&!same(base[key],candidate[key]))fail('IMMUTABLE_LAYOUT:'+key)
 if(base.decorations.length!==285||!same(base.decorations,candidate.decorations)||proof.incumbentDecorationsExact!==285)fail('INCUMBENT_DECORATION')
 if(proof.protectedLabelRectanglesExact!==18)fail('LABEL_RECTANGLES')
 if(proof.frozenV2RecompositionExactEras!==3)fail('FROZEN_LAYER_RECOMPOSITION')
 if(!pinsMatch(proof.baselinePins,FROZEN)||!pinsMatch(ground.baselinePins,FROZEN))fail('FROZEN_V2')
 if(!same(proof.originalCropPins,original.originalCropPins)||!same(proof.additionCropPins,original.additionCropPins)||!same(proof.sourceInputs,original.sourceInputs))fail('ORIGINAL_RGBA_OR_INPUT')
 if(!same(proof.allowedBounds,BOUNDS)||!same(candidate.facadeDerivation?.allowedBounds,BOUNDS))fail('ROOT_PAINT_BOUNDS')
 if(!same(proof.componentsByTheme,candidate.facadeDerivation?.componentsByTheme)||!same(proof.groundBinding,candidate.facadeDerivation?.groundBinding))fail('DERIVATION_PROOF')
 if(ground.format!=='MCR_GROUND_OVERLAY_V1'||ground.sourceCommit!==SOURCE||!same(ground.dimensions,[W,H])||ground.qualification.status!=='PASS'||!ground.qualification.sourceCommit)fail('QUALIFIED_GROUND')
 if(proof.groundBinding.path!=='frontend/public/world/map-lawns-derived-v3/interface.json')fail('GROUND_INTERFACE_PATH')
 if(proof.helperPin.path!=='frontend/public/world/map-reference-composition-v1/build.py'||proof.helperPin.sha256!=='sha256:6bf696b7c3edd5adf35e0b5995ca9cd33f915e87898de0e9266f2923cffc0433')fail('IMMUTABLE_HELPER')
 for(const era of ERAS) {
  const byId=proof.componentsByTheme[era],stats=proof.actualByTheme[era]
  if(!byId||!stats||!same(Object.keys(byId).sort(),Object.keys(BOUNDS).sort())){fail('THREE_NAMED_FACADES');continue}
  if(stats.facadeChangedPixels<1000||stats.groundChangedPixels<=0||stats.changedPixels<=stats.facadeChangedPixels)fail('ACTUAL_COMBINED_DELTA')
  const gaps=new Set<string>()
  for(const [id,components]of Object.entries(byId)) {
   if(!components.some(c=>c.role==='negative_gap')||!components.some(c=>c.role==='recessed_bay'))fail('FACADE_ARTICULATION')
   for(const c of components) {
    if(!c.rect.every(Number.isInteger)||!subset(c.rect,BOUNDS[id]!))fail('ROOT_PAINT_BOUNDS')
    for(const op of c.operations) {
     if(!op.rect.every(Number.isInteger)||!subset(op.rect,c.rect))fail('COMPONENT_OPERATION')
     if(op.kind!=='rect'&&op.kind!=='clear')fail('COMPONENT_OPERATION')
     if(op.kind==='rect'&&(!op.rgba||op.rgba.length!==4||op.rgba[3]!==255||op.rgba.some(v=>!Number.isInteger(v)||v<0||v>255)))fail('COMPONENT_RGBA')
    }
    if(c.role==='negative_gap') {
     gaps.add(c.id)
     if(c.operations.length!==1||c.operations[0]!.kind!=='clear'||!same(c.operations[0]!.rect,c.rect))fail('TRUE_RECOMPOSED_GAP')
     if(!stats.trueGaps.some(g=>g.id===c.id&&same(g.rect,c.rect)&&/^sha256:[a-f0-9]{64}$/.test(g.underlyingRgbSha256)))fail('TRUE_RECOMPOSED_GAP')
    }
   }
  }
  if(stats.trueGaps.length!==gaps.size)fail('TRUE_RECOMPOSED_GAP')
  if(!same(byId,proof.componentsByTheme.PARADE))fail('ERA_GEOMETRY')
 }
 return [...reasons]
}
export async function verifyArtifacts(proof:Proof,ground:Ground,readset:ReadonlyMap<string,Uint8Array>):Promise<readonly string[]> {
 const reasons=new Set<string>(),fail=(s:string)=>{reasons.add(s)}
 const expected=['layout.json',...ERAS.flatMap(e=>['png','svg'].map(x=>e.toLowerCase()+'-scene.'+x))]
 if(proof.outputs.length!==7||new Set(proof.outputs.map(p=>p.path)).size!==7||expected.some(n=>!proof.outputs.some(p=>p.path===PREFIX+n)))fail('OUTPUT_SET')
 if(ground.helperPin.path!==proof.helperPin.path||hexsha(ground.helperPin.sha256)!==hexsha(proof.helperPin.sha256))fail('GROUND_HELPER')
 const pins=[...proof.baselinePins,...proof.sourceInputs,proof.helperPin,proof.recipe,proof.groundBinding,...proof.outputs,...ground.baselinePins,ground.provenancePin,ground.qualification.receipt,...Object.values(ground.eras).flatMap(e=>[e.overlay,e.svg,e.operationsPin,e.allowedMaskPin])]
 function provenancePins(value:unknown):void {
  if(!value||typeof value!=='object')return
  if('path'in value&&'bytes'in value&&'sha256'in value){pins.push(value as Pin);return}
  for(const child of Object.values(value))provenancePins(child)
 }
 const provenance=readset.get(ground.provenancePin.path)
 if(provenance)provenancePins(JSON.parse(Buffer.from(provenance).toString()))
 for(const p of pins) {
  const raw=readset.get(p.path)
  if(!raw||raw.length!==p.bytes){fail('BYTES:'+p.path);continue}
  if(hexsha(await digest(raw))!==hexsha(p.sha256))fail('SHA256:'+p.path)
 }
 if(reasons.size)return [...reasons]
 if(!same(JSON.parse(Buffer.from(readset.get(proof.groundBinding.path)!).toString()),ground))return ['GROUND_INTERFACE_BYTES']
 const receipt=Buffer.from(readset.get(ground.qualification.receipt.path)!).toString().trim().split('\n').map(line=>JSON.parse(line) as {event:string;status?:string;head?:string;tempAbsent?:boolean;allChildrenExited?:boolean})
 const last=receipt.at(-1)!
 if(last.event!=='FULL'||last.status!=='PASS'||!last.tempAbsent||!last.allChildrenExited||!receipt.some(r=>r.event==='ACCEPT'&&r.head===ground.qualification.sourceCommit))fail('GROUND_LITERAL_FULL')
 const base=JSON.parse(Buffer.from(readset.get(BASE+'layout.json')!).toString()) as Layout
 const v2=JSON.parse(Buffer.from(readset.get(BASE+'source-provenance.json')!).toString()) as Proof
 for(const era of ERAS) {
  const atlasPin=v2.originalCropPins.find(p=>p.themeId===era)!.atlas
  const atlas=decodePng(readset.get(atlasPin.path)!)
  const masks=facadeMasks(base,v2,era,atlas)
  const facadeUnion=new Uint8Array(W*H)
  for(const id of Object.keys(BOUNDS)) {
   const mask=masks[id]!;let count=0
   for(let i=0;i<mask.length;i++)if(mask[i]){facadeUnion[i]=255;count++}
   const pin=proof.facadeMaskPins[era]![id]!
   if(pin.bytes!==mask.length||pin.pixels!==count||pin.sha256!==await digest(mask))fail('INDEPENDENT_FACADE_ALPHA_MASK:'+era+':'+id)
   for(const c of proof.componentsByTheme[era]![id]!)for(let y=c.rect[1];y<c.rect[1]+c.rect[3];y++)for(let x=c.rect[0];x<c.rect[0]+c.rect[2];x++)if(!mask[y*W+x])fail('SOURCE_ALPHA_OR_FOREGROUND:'+c.id)
  }
  const before=decodePng(readset.get(BASE+era.toLowerCase()+'-scene.png')!),after=decodePng(readset.get(PREFIX+era.toLowerCase()+'-scene.png')!)
  if(after.width!==W||after.height!==H){fail('DIMENSIONS');continue}
  const overlay=decodePng(readset.get(ground.eras[era]!.overlay.path)!)
  const allowedGround=decodePng(readset.get(ground.eras[era]!.allowedMaskPin.path)!)
  if(overlay.width!==W||overlay.height!==H||allowedGround.width!==W||allowedGround.height!==H){fail('GROUND_DIMENSIONS');continue}
  const groundRaw=new Uint8Array(W*H)
  for(let i=0;i<W*H;i++)groundRaw[i]=allowedGround.rgba[i*4]!
  if(hexsha(await digest(groundRaw))!==hexsha(ground.eras[era]!.allowedMaskSha256)||hexsha(await digest(groundRaw))!==hexsha(proof.groundMaskPins[era]!.sha256))fail('GROUND_MASK_IDENTITY')
  let changed=0,facadeChanged=0,groundChanged=0
  for(let i=0;i<W*H;i++) {
   const at=i*4,alpha=overlay.rgba[at+3]!
   if(facadeUnion[i]&&groundRaw[i])fail('DISJOINT_PAINT')
   if(alpha&&(!groundRaw[i]||alpha!==255))fail('GROUND_ALPHA_OUTSIDE')
   if(before.rgba[at]===after.rgba[at]&&before.rgba[at+1]===after.rgba[at+1]&&before.rgba[at+2]===after.rgba[at+2])continue
   changed++;if(facadeUnion[i])facadeChanged++;else if(groundRaw[i])groundChanged++;else fail('UNOWNED_PIXEL')
  }
  const stats=proof.actualByTheme[era]!
  if(stats.changedPixels!==changed||stats.facadeChangedPixels!==facadeChanged||stats.groundChangedPixels!==groundChanged)fail('ACTUAL_DELTA_COUNTS')
  for(const gap of stats.trueGaps) {
   const [x,y,w,h]=gap.rect,rgb=new Uint8Array(w*h*3);let n=0
   for(let yy=y;yy<y+h;yy++)for(let xx=x;xx<x+w;xx++){const at=(yy*W+xx)*4;for(let c=0;c<3;c++)rgb[n++]=after.rgba[at+c]!}
   if(await digest(rgb)!==gap.underlyingRgbSha256)fail('GAP_UNDERLYING_PIXELS')
  }
 }
 return [...reasons]
}

