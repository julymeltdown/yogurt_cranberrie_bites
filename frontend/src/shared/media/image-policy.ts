import type { ImageMime } from './types'
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024
export const MAX_IMAGE_DIMENSION = 4096
export const MAX_IMAGE_PIXELS = 16_000_000
export const IMAGE_TYPES: readonly ImageMime[] = ['image/jpeg', 'image/png', 'image/webp']
export class ImageError extends Error {
  constructor(readonly code: 'invalid-type' | 'invalid-bytes' | 'invalid-dimensions' | 'animated-image' | 'decode-failed' | 'aborted') {
    super(code); this.name = 'ImageError'
  }
}
export function checkAbort(signal: AbortSignal) { if (signal.aborted) throw new ImageError('aborted') }
export function dimensions(width: number, height: number) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1
    || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION || width * height > MAX_IMAGE_PIXELS) throw new ImageError('invalid-dimensions')
  return { width, height }
}
/** Header gate before browser decode. This is not a substitute for authoritative server decoding. */
export async function inspectImage(blob: Blob, signal: AbortSignal): Promise<{ width: number; height: number; type: ImageMime }> {
  checkAbort(signal)
  if (!IMAGE_TYPES.includes(blob.type as ImageMime)) throw new ImageError('invalid-type')
  if (!blob.size || blob.size > MAX_IMAGE_BYTES) throw new ImageError('invalid-bytes')
  const bytes = new Uint8Array(await blob.arrayBuffer()); checkAbort(signal)
  const v = new DataView(bytes.buffer), text = (at: number, n: number) => String.fromCharCode(...bytes.subarray(at, at + n))
  const fail = (): never => { throw new ImageError('decode-failed') }
  let size: { width: number; height: number } | undefined
  try {
    if (blob.type === 'image/png') {
      if (text(0, 8) !== '\x89PNG\r\n\x1a\n' || text(12, 4) !== 'IHDR' || v.getUint32(8) !== 13) fail()
      size = dimensions(v.getUint32(16), v.getUint32(20))
      let ended = false
      for (let p = 8; p + 12 <= bytes.length;) {
        const length = v.getUint32(p), kind = text(p + 4, 4)
        if (length > bytes.length - p - 12) fail()
        if (kind === 'acTL' || kind === 'fcTL' || kind === 'fdAT') throw new ImageError('animated-image')
        p += length + 12
        if (kind === 'IEND') { ended = true; break }
      }
      if (!ended) fail()
    } else if (blob.type === 'image/jpeg') {
      if (v.getUint16(0) !== 0xffd8) fail()
      for (let p = 2; p + 4 <= bytes.length;) {
        if (bytes[p++] !== 0xff) fail()
        while (bytes[p] === 0xff) p++
        const marker = bytes[p++]!
        if (marker === 0xda || marker === 0xd9) break
        const length = v.getUint16(p)
        if (length < 2 || p + length > bytes.length) fail()
        if ([0xc0, 0xc1, 0xc2].includes(marker)) size = dimensions(v.getUint16(p + 5), v.getUint16(p + 3))
        p += length
      }
    } else {
      if (text(0, 4) !== 'RIFF' || text(8, 4) !== 'WEBP' || v.getUint32(4, true) + 8 !== bytes.length) fail()
      for (let p = 12; p + 8 <= bytes.length;) {
        const kind = text(p, 4), length = v.getUint32(p + 4, true), d = p + 8
        if (length > bytes.length - d) fail()
        if (kind === 'ANIM' || kind === 'ANMF' || (kind === 'VP8X' && (bytes[d]! & 2))) throw new ImageError('animated-image')
        if (kind === 'VP8X' && length >= 10) {
          const u24 = (i: number) => bytes[i]! + (bytes[i + 1]! << 8) + (bytes[i + 2]! << 16)
          size = dimensions(u24(d + 4) + 1, u24(d + 7) + 1)
        } else if (kind === 'VP8 ' && length >= 10) {
          if (text(d + 3, 3) !== '\x9d\x01\x2a') fail()
          size ??= dimensions(v.getUint16(d + 6, true) & 0x3fff, v.getUint16(d + 8, true) & 0x3fff)
        } else if (kind === 'VP8L' && length >= 5) {
          if (bytes[d] !== 0x2f) fail()
          const n = v.getUint32(d + 1, true)
          size ??= dimensions((n & 0x3fff) + 1, ((n >>> 14) & 0x3fff) + 1)
        }
        p = d + length + (length % 2)
      }
    }
    if (!size) fail()
    return { ...size!, type: blob.type as ImageMime }
  } catch (error) { if (error instanceof ImageError) throw error; return fail() }
}
