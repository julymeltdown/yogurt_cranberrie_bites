import { describe, expect, it, vi } from 'vitest'
import { inspectImage } from '../../../../shared/media/image-policy'
import { uploadPresignedImage } from '../../../../shared/media/presigned-upload'
import { bytes, file, image, setup } from './fixtures'
const signal = () => new AbortController().signal
function png(w: number, h: number) { const b = bytes(); const v = new DataView(b.buffer); v.setUint32(16, w); v.setUint32(20, h); return new Blob([b], { type: 'image/png' }) }
describe('bounded static image policy', () => {
  it.each(['video/mp4', 'image/svg+xml', 'image/gif', 'text/html', ''])('rejects unapproved MIME %s', async type => {
    await expect(inspectImage(new Blob([bytes()], { type }), signal())).rejects.toMatchObject({ code: 'invalid-type' })
  })
  it.each([new Blob([], { type: 'image/png' }), new Blob([new Uint8Array(10485761)], { type: 'image/png' })])('rejects invalid byte count', async b => {
    await expect(inspectImage(b, signal())).rejects.toMatchObject({ code: 'invalid-bytes' })
  })
  it.each([[4097, 1], [0, 10], [4001, 4000], [4096, 4096]])('rejects invalid dimensions %i x %i before decode', async (w, h) => {
    await expect(inspectImage(png(w, h), signal())).rejects.toMatchObject({ code: 'invalid-dimensions' })
  })
  it('accepts exact contract pixel maximum and detects MIME/signature mismatch', async () => {
    expect(await inspectImage(png(4000, 4000), signal())).toMatchObject({ width: 4000, height: 4000 })
    await expect(inspectImage(new Blob([bytes()], { type: 'image/jpeg' }), signal())).rejects.toMatchObject({ code: 'decode-failed' })
  })
  it('rejects PNG animation control chunks', async () => {
    const original = bytes(), animated = new Uint8Array(original.length + 20)
    animated.set(original.subarray(0, 33)); animated.set([0, 0, 0, 8, 97, 99, 84, 76], 33); animated.set(original.subarray(33), 53)
    await expect(inspectImage(new Blob([animated], { type: 'image/png' }), signal())).rejects.toMatchObject({ code: 'animated-image' })
  })
  it('parses JPEG dimensions and rejects truncated JPEG headers', async () => {
    const jpeg = new Blob([new Uint8Array([255,216,255,192,0,11,8,0,20,0,30,1,1,17,0,255,217])], { type: 'image/jpeg' })
    expect(await inspectImage(jpeg, signal())).toMatchObject({ width: 30, height: 20 })
    await expect(inspectImage(new Blob(['invalid'], { type: 'image/jpeg' }), signal())).rejects.toMatchObject({ code: 'decode-failed' })
  })
  it('parses bounded WebP VP8X and rejects animation flags', async () => {
    const b = new Uint8Array(30), v = new DataView(b.buffer)
    b.set(new TextEncoder().encode('RIFF')); v.setUint32(4,22,true); b.set(new TextEncoder().encode('WEBPVP8X'),8); v.setUint32(16,10,true); b[24]=29; b[27]=19
    expect(await inspectImage(new Blob([b], { type: 'image/webp' }), signal())).toMatchObject({ width:30,height:20 })
    b[20]=2; await expect(inspectImage(new Blob([b], { type:'image/webp' }),signal())).rejects.toMatchObject({ code:'animated-image' })
  })
  it('early abort and invalid output prevent preview/network side effects', async () => {
    const c = new AbortController(); c.abort(); await expect(inspectImage(file(),c.signal)).rejects.toMatchObject({ code:'aborted' })
    const h = setup({}, async () => ({ ...image(), blob: new Blob(['video'],{type:'video/mp4'}) }))
    await h.controller.select(file()); expect(h.previews.create).not.toHaveBeenCalled(); expect(h.transport.initialize).not.toHaveBeenCalled()
  })
})
describe('independent credential-free presigned transport (unused by C04)', () => {
  it('omits cookies and all session/authorization/user headers even on an allowlisted same origin', async () => {
    const fetch = vi.fn<typeof globalThis.fetch>(async (_url, options) => {
      expect(options?.credentials).toBe('omit'); expect(options?.redirect).toBe('error'); expect(options?.referrerPolicy).toBe('no-referrer')
      expect([...new Headers(options?.headers)]).toEqual([['content-type', 'image/png']]); expect(options?.body).toBeInstanceOf(Blob)
      return new Response(null,{status:200})
    })
    await uploadPresignedImage({ url:'https://mcr.example/storage?signed=opaque',allowedOrigins:['https://mcr.example'],blob:image().blob,signal:signal(),fetch })
    expect(fetch).toHaveBeenCalledTimes(1)
  })
  it.each(['http://storage.example/u','https://user:secret@storage.example/u','https://storage.example/u#fragment','https://evil.example/u','/relative'])('rejects unqualified target %s',async url=>{
    const fetch=vi.fn<typeof globalThis.fetch>()
    await expect(uploadPresignedImage({url,allowedOrigins:['https://storage.example'],blob:image().blob,signal:signal(),fetch})).rejects.toThrow()
    expect(fetch).not.toHaveBeenCalled()
  })
  it('fails closed on redirect and never turns PUT completion into READY media',async()=>{
    const response=new Response(null,{status:200});Object.defineProperty(response,'redirected',{value:true})
    await expect(uploadPresignedImage({url:'https://storage.example/u',allowedOrigins:['https://storage.example'],blob:image().blob,signal:signal(),fetch:vi.fn(async()=>response)})).rejects.toThrow()
  })
})
