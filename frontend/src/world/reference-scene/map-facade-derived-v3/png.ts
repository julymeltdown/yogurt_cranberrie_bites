/** PNG decoding for independent source-pixel checks; builtin zlib only. */
import {inflateSync} from 'node:zlib'
export function decodePng(input: Uint8Array) {
  const raw=Buffer.from(input)
  if (!raw.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))) throw Error('PNG signature')
  let width=0,height=0,channels=0,offset=8;const chunks:Buffer[]=[]
  while(offset+12<=raw.length) {
    const size=raw.readUInt32BE(offset),kind=raw.toString('ascii',offset+4,offset+8)
    if(offset+12+size>raw.length)throw Error('PNG chunk')
    const bytes=raw.subarray(offset+8,offset+8+size)
    if(kind==='IHDR') {
      width=bytes.readUInt32BE(0);height=bytes.readUInt32BE(4)
      channels=({0:1,2:3,6:4} as Record<number,number>)[bytes[9]!]??0
      if(!width||!height||width>2048||height>2048||bytes[8]!==8||!channels||bytes[12]!==0)throw Error('PNG format')
    }
    if(kind==='IDAT')chunks.push(bytes)
    offset+=size+12;if(kind==='IEND')break
  }
  const stride=width*channels,unpacked=inflateSync(Buffer.concat(chunks),{maxOutputLength:(stride+1)*height})
  if(unpacked.length!==(stride+1)*height)throw Error('PNG scanline length')
  const pixels=new Uint8Array(stride*height)
  function paeth(a:number,b:number,c:number) {
    const p=a+b-c,pa=Math.abs(p-a),pb=Math.abs(p-b),pc=Math.abs(p-c)
    return pa<=pb&&pa<=pc?a:pb<=pc?b:c
  }
  for(let y=0;y<height;y++) {
    const filter=unpacked[y*(stride+1)]!
    if(filter>4)throw Error('PNG filter')
    for(let x=0;x<stride;x++) {
      const a=x>=channels?pixels[y*stride+x-channels]!:0,b=y?pixels[(y-1)*stride+x]!:0,c=y&&x>=channels?pixels[(y-1)*stride+x-channels]!:0
      const prediction=[0,a,b,Math.floor((a+b)/2),paeth(a,b,c)][filter]!
      pixels[y*stride+x]=(unpacked[y*(stride+1)+1+x]!+prediction)&255
    }
  }
  const rgba=new Uint8Array(width*height*4)
  for(let i=0;i<width*height;i++) {
    rgba[i*4]=pixels[i*channels]!
    rgba[i*4+1]=pixels[i*channels+(channels===1?0:1)]!
    rgba[i*4+2]=pixels[i*channels+(channels===1?0:2)]!
    rgba[i*4+3]=channels===4?pixels[i*channels+3]!:255
  }
  return {width,height,rgba}
}

