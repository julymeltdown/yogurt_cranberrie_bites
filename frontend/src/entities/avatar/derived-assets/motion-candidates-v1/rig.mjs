/** Source-pixel articulated FRONT gait. Not a side/rear reconstruction.
 * Coordinates below are reviewed landmarks on the original 1280x853 photo_01.
 * No new pixels, mirror, palette changes, hidden-surface fill or whole-figure bob.
 */
export const rigs = [
  {id:'photo01-front-01',rect:{x:40,y:80,width:380,height:390},center:244,armTop:360,armLeft:175,armRight:307,legTop:423},
  {id:'photo01-front-02',rect:{x:440,y:80,width:390,height:390},center:640,armTop:362,armLeft:575,armRight:708,legTop:426},
  {id:'photo01-front-03',rect:{x:850,y:80,width:395,height:390},center:1050,armTop:363,armLeft:980,armRight:1110,legTop:429},
  {id:'photo01-front-04',rect:{x:40,y:475,width:380,height:360},center:244,armTop:750,armLeft:176,armRight:307,legTop:800},
  {id:'photo01-front-05',rect:{x:440,y:475,width:390,height:360},center:642,armTop:752,armLeft:576,armRight:711,legTop:800},
  {id:'photo01-front-06',rect:{x:850,y:475,width:395,height:360},center:1050,armTop:752,armLeft:980,armRight:1112,legTop:800},
]
// Alternating planted/lifted feet and counter-swinging arms. All offsets are
// original SOURCE pixels; each frame is independently resampled once to 64px.
export const poses = [
 {name:'idle',leftArm:[0,0],rightArm:[0,0],leftLeg:[0,0],rightLeg:[0,0]},
 {name:'walk-0-left-contact',leftArm:[0,-8],rightArm:[0,5],leftLeg:[-6,0],rightLeg:[4,-12]},
 {name:'walk-1-left-passing',leftArm:[0,-4],rightArm:[0,2],leftLeg:[0,0],rightLeg:[0,-6]},
 {name:'walk-2-right-contact',leftArm:[0,5],rightArm:[0,-8],leftLeg:[-4,-12],rightLeg:[6,0]},
 {name:'walk-3-right-passing',leftArm:[0,2],rightArm:[0,-4],leftLeg:[0,-6],rightLeg:[0,0]},
]
export function classify(rig,x,y) {
 if(y>=rig.legTop)return x<rig.center?'leftLeg':'rightLeg'
 if(y>=rig.armTop&&x<rig.armLeft)return 'leftArm'
 if(y>=rig.armTop&&x>=rig.armRight)return 'rightArm'
 return 'torso'
}
export function articulate(part,rig,pose) {
 const rgba=new Uint8ClampedArray(64*64*4), provenance=new Int16Array(64*64*2).fill(-1)
 const b=part.bounds,scale=rig.scale??56/b.height,anchor=rig.sourceAnchor??{x:rig.foot?.x??b.x+b.width/2,y:b.y+b.height}
 const layers=['leftLeg','rightLeg','leftArm','rightArm','torso']
 // Nearest original pixels, mobile limbs under stationary joint overlap.
 for(const layer of layers){const [dx,dy]=pose[layer]??[0,0]
  for(let y=0;y<64;y++)for(let x=0;x<64;x++){
   const sx=Math.floor((x+.5-32)/scale+anchor.x-dx),sy=Math.floor((y+.5-60)/scale+anchor.y-dy)
   if(sx<0||sy<0||sx>=part.width||sy>=part.height)continue
   const i=(sy*part.width+sx)*4,gx=sx+rig.rect.x,gy=sy+rig.rect.y,region=classify(rig,gx,gy)
   if(!part.rgba[i+3])continue
   const jointRoot=(region.endsWith('Leg')&&gy<rig.legTop+(rig.jointOverlap??12))||(region.endsWith('Arm')&&gy<rig.armTop+(rig.jointOverlap??12))
   if(layer==='torso' ? region!=='torso'&&!jointRoot : region!==layer)continue
   rgba.set(part.rgba.subarray(i,i+4),(y*64+x)*4);provenance[(y*64+x)*2]=gx;provenance[(y*64+x)*2+1]=gy
  }
 }
 return{rgba,width:64,height:64,provenance,scale,anchor}
}
