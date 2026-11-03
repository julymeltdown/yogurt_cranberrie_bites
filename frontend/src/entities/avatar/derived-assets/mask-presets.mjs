/** Deterministic selected-source extraction. Only alpha changes at native resolution. */
export function extractRgba(source,width,height,rect) {
  const {x:offsetX,y:offsetY,width:w,height:h}=rect, count=w*h
  const rgba=new Uint8ClampedArray(count*4),background=new Uint8Array(count),queue=[]
  const eligible=index=>{
    const i=index*4,r=rgba[i],g=rgba[i+1],b=rgba[i+2]
    return Math.min(r,g,b)>=118 && Math.max(r,g,b)-Math.min(r,g,b)<=18
  }
  for(let y=0;y<h;y++) for(let x=0;x<w;x++) {
    const from=((offsetY+y)*width+offsetX+x)*4,to=(y*w+x)*4
    rgba.set(source.subarray(from,from+4),to)
  }
  const add=index=>{if(!background[index]&&eligible(index)){background[index]=1;queue.push(index)}}
  for(let x=0;x<w;x++){add(x);add((h-1)*w+x)}
  for(let y=0;y<h;y++){add(y*w);add(y*w+w-1)}
  for(let at=0;at<queue.length;at++) {
    const index=queue[at],x=index%w,y=Math.floor(index/w)
    if(x>0)add(index-1);if(x+1<w)add(index+1);if(y>0)add(index-w);if(y+1<h)add(index+w)
  }
  // Discard detached JPEG ringing outside the continuous supplied character.
  // Enclosed light pixels remain connected foreground, including every white highlight.
  const visited=new Uint8Array(count),components=[]
  for(let index=0;index<count;index++) {
    if(background[index]||visited[index])continue
    const part=[index];visited[index]=1
    for(let at=0;at<part.length;at++) {
      const p=part[at],x=p%w,y=Math.floor(p/w)
      for(const next of [x>0?p-1:-1,x+1<w?p+1:-1,y>0?p-w:-1,y+1<h?p+w:-1])if(next>=0&&!background[next]&&!visited[next]){visited[next]=1;part.push(next)}
    }
    components.push(part)
  }
  components.sort((a,b)=>b.length-a.length)
  if(!components[0]||components[0].length<10000)throw Error('Character foreground not resolved')
  const keep=new Uint8Array(count);for(const index of components[0])keep[index]=1
  let minX=w,minY=h,maxX=-1,maxY=-1,retained=0,white=0,dark=0,sourceDark=0
  for(let index=0;index<count;index++) {
    if(Math.max(rgba[index*4],rgba[index*4+1],rgba[index*4+2])<=112)sourceDark++
    rgba[index*4+3]=keep[index]?255:0
    if(!keep[index])continue
    const x=index%w,y=Math.floor(index/w),i=index*4
    minX=Math.min(minX,x);minY=Math.min(minY,y);maxX=Math.max(maxX,x);maxY=Math.max(maxY,y);retained++
    if(Math.min(rgba[i],rgba[i+1],rgba[i+2])>=230)white++
    if(Math.max(rgba[i],rgba[i+1],rgba[i+2])<=112)dark++
  }
  if(minX===0||minY===0||maxX===w-1||maxY===h-1)throw Error('Foreground touches review-window boundary')
  let shoeMinX=w,shoeMaxX=-1
  for(let y=Math.max(minY,maxY-7);y<=maxY;y++)for(let x=minX;x<=maxX;x++)if(keep[y*w+x]){shoeMinX=Math.min(shoeMinX,x);shoeMaxX=Math.max(shoeMaxX,x)}
  return {rgba,width:w,height:h,alphaBounds:{x:minX,y:minY,width:maxX-minX+1,height:maxY-minY+1},
    foot:{x:(shoeMinX+shoeMaxX+1)/2,y:maxY+1},
    evidence:{retainedPixels:retained,retainedWhitePixels:white,retainedDarkPixels:dark,sourceDarkPixels:sourceDark,removedDarkPixels:sourceDark-dark,
      removedBorderConnectedBackground:queue.length,discardedDetachedComponents:components.slice(1).map(c=>c.length)}}
}

/** Sample with an explicit uniform transform; every opaque output RGB comes from one source pixel. */
export function registerRgba(extracted,cellSize=64) {
  const {rgba,width,height,alphaBounds:b,foot}=extracted,ratio=cellSize/64
  const left=foot.x-b.x,right=b.x+b.width-foot.x,top=foot.y-b.y
  const scale=Math.min(56/top,29/left,29/right)*ratio
  const result=new Uint8ClampedArray(cellSize*cellSize*4),sourceSamples=[]
  for(let y=0;y<cellSize;y++)for(let x=0;x<cellSize;x++) {
    const sx=Math.floor((x+0.5-32*ratio)/scale+foot.x),sy=Math.floor((y+0.5-60*ratio)/scale+foot.y)
    if(sx<0||sy<0||sx>=width||sy>=height)continue
    const from=(sy*width+sx)*4,to=(y*cellSize+x)*4
    if(!rgba[from+3])continue
    result.set(rgba.subarray(from,from+4),to);sourceSamples.push({output:x+y*cellSize,source:sx+sy*width})
  }
  return {rgba:result,width:cellSize,height:cellSize,scale,foot:{x:32*ratio,y:60*ratio},sourceFoot:foot,sourceSamples}
}

export async function buildPresetPixels(layout) {
  const image=new Image();image.src=layout.source_path;await image.decode()
  if(image.naturalWidth!==1280||image.naturalHeight!==853)throw Error('Unexpected original dimensions')
  const canvas=document.createElement('canvas');canvas.width=1280;canvas.height=853
  const context=canvas.getContext('2d',{willReadFrequently:true});context.drawImage(image,0,0)
  const source=context.getImageData(0,0,1280,853).data
  const png=(rgba,w,h)=>{
    const c=document.createElement('canvas');c.width=w;c.height=h
    c.getContext('2d').putImageData(new ImageData(rgba,w,h),0,0)
    return c.toDataURL('image/png').split(',')[1]
  }
  const atlas64=new Uint8ClampedArray(384*64*4),atlas256=new Uint8ClampedArray(1536*256*4),frames=[]
  for(let index=0;index<layout.regions.length;index++) {
    const region=layout.regions[index],extracted=extractRgba(source,1280,853,region.rect)
    const nativeW=extracted.alphaBounds.width+4,nativeH=extracted.alphaBounds.height+4
    const native=new Uint8ClampedArray(nativeW*nativeH*4),origin={x:extracted.alphaBounds.x-2,y:extracted.alphaBounds.y-2}
    let retainedRgbMismatches=0
    for(let y=0;y<nativeH;y++)for(let x=0;x<nativeW;x++) {
      const sx=x+origin.x,sy=y+origin.y
      if(sx<0||sy<0||sx>=extracted.width||sy>=extracted.height)continue
      const from=(sy*extracted.width+sx)*4,to=(y*nativeW+x)*4
      native.set(extracted.rgba.subarray(from,from+4),to)
      if(!native[to+3])continue
      const sourceIndex=((region.rect.y+sy)*1280+region.rect.x+sx)*4
      if(native[to]!==source[sourceIndex]||native[to+1]!==source[sourceIndex+1]||native[to+2]!==source[sourceIndex+2])retainedRgbMismatches++
    }
    const low=registerRgba(extracted,64),detail=registerRgba(extracted,256)
    let registeredRgbMismatches=0
    for(const variant of [low,detail])for(const sample of variant.sourceSamples)for(let channel=0;channel<4;channel++)if(variant.rgba[sample.output*4+channel]!==extracted.rgba[sample.source*4+channel])registeredRgbMismatches++
    for(let y=0;y<64;y++)atlas64.set(low.rgba.subarray(y*64*4,(y+1)*64*4),(y*384+index*64)*4)
    for(let y=0;y<256;y++)atlas256.set(detail.rgba.subarray(y*256*4,(y+1)*256*4),(y*1536+index*256)*4)
    frames.push({sourceLocation:region.source_location,label:region.label,sourceRect:region.rect,
      native:{png:png(native,nativeW,nativeH),width:nativeW,height:nativeH,
        sourceOrigin:{x:region.rect.x+origin.x,y:region.rect.y+origin.y},foot:{x:extracted.foot.x-origin.x,y:extracted.foot.y-origin.y}},
      transform:{sourceFoot:{x:region.rect.x+extracted.foot.x,y:region.rect.y+extracted.foot.y},scale64:low.scale,foot64:{x:32,y:60}},
      evidence:{...extracted.evidence,retainedRgbMismatches,registeredRgbMismatches,registered64Pixels:low.sourceSamples.length,registered256Pixels:detail.sourceSamples.length}})
  }
  return {frames,atlas64:png(atlas64,384,64),atlas256:png(atlas256,1536,256)}
}
