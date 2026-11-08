import { describe, expect, it } from 'vitest'
import {
  ACCEPTED_TYPES,
  MAX_ATTACHMENTS,
  MediaPicker,
  prepare,
  precheck,
  REJECTION_MESSAGE,
} from '../preparation'
import { MAX_IMAGE_BYTES } from '../../../shared/media/image-policy'

/**
 * A minimal PNG: the signature, an IHDR chunk, and a complete IEND chunk.
 *
 * The inspection walks chunks by `length + 12`, so the IEND chunk needs its full
 * twelve bytes (length, type, CRC) present or the walk never sees it.
 */
function pngFile(width = 2, height = 2, name = 'photo.png'): File {
  const header = new Uint8Array([
    // signature
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    // IHDR: length 13, type, width, height, bit depth, colour type, ...
    0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    (width >>> 24) & 0xff, (width >>> 16) & 0xff, (width >>> 8) & 0xff, width & 0xff,
    (height >>> 24) & 0xff, (height >>> 16) & 0xff, (height >>> 8) & 0xff, height & 0xff,
    0x08, 0x06, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    // IEND: length 0, type, CRC
    0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
  ])
  return new File([header], name, { type: 'image/png' })
}

function fileOf(type: string, bytes = 64, name = 'file'): File {
  return new File([new Uint8Array(bytes)], name, { type })
}

describe('media preparation', () => {
  it('accepts exactly the server types', () => {
    expect([...ACCEPTED_TYPES]).toEqual(['image/jpeg', 'image/png', 'image/webp'])
  })

  // The acceptance case: a GIF is rejected client-side with the same message the
  // server would produce.
  it('rejects a GIF client-side with the server message', async () => {
    const gif = fileOf('image/gif', 64, 'anim.gif')
    expect(precheck(gif)).toBe(REJECTION_MESSAGE['invalid-type'])
    expect(REJECTION_MESSAGE['invalid-type']).toContain('GIF')
    const result = await prepare(gif, new AbortController().signal)
    expect(result.prepared).toBeNull()
    expect(result.message).toBe(REJECTION_MESSAGE['invalid-type'])
    // The rejection costs no decoding work: the type alone was enough.
    expect(result.cancelled).toBe(false)
  })

  it('rejects an oversized file', async () => {
    const big = new File([new Uint8Array(MAX_IMAGE_BYTES + 1)], 'big.png', { type: 'image/png' })
    expect(precheck(big)).toBe(REJECTION_MESSAGE['invalid-bytes'])
    expect(REJECTION_MESSAGE['invalid-bytes']).toContain('10 MiB')
  })

  it('rejects an empty file', () => {
    expect(precheck(fileOf('image/png', 0))).toBe(REJECTION_MESSAGE['invalid-bytes'])
  })

  it('accepts a valid PNG and reports its dimensions', async () => {
    const result = await prepare(pngFile(640, 480), new AbortController().signal)
    expect(result.message).toBeNull()
    expect(result.prepared?.width).toBe(640)
    expect(result.prepared?.height).toBe(480)
    expect(result.prepared?.type).toBe('image/png')
  })

  it('rejects an out-of-range dimension with the server message', async () => {
    const result = await prepare(pngFile(5000, 2), new AbortController().signal)
    expect(result.prepared).toBeNull()
    expect(result.message).toBe(REJECTION_MESSAGE['invalid-dimensions'])
  })

  // The acceptance case: cancelling mid-preparation leaves no partial upload.
  it('leaves nothing behind when cancelled', async () => {
    const controller = new AbortController()
    controller.abort()
    const result = await prepare(pngFile(), controller.signal)
    expect(result.prepared).toBeNull()
    expect(result.cancelled).toBe(true)
    expect(result.message).toBeNull()
  })

  it('cancels an in-flight preparation through the picker', async () => {
    const picker = new MediaPicker()
    const pending = picker.add(pngFile(2, 2, 'a.png'))
    // Remove it before the preparation settles.
    picker.remove('file-1')
    await pending
    expect(picker.list()).toHaveLength(0)
    expect(picker.prepared()).toHaveLength(0)
  })

  it('selects, orders and removes files', async () => {
    const picker = new MediaPicker()
    await picker.add(pngFile(2, 2, 'a.png'))
    await picker.add(pngFile(2, 2, 'b.png'))
    await picker.add(pngFile(2, 2, 'c.png'))
    expect(picker.list().map((item) => item.file.name)).toEqual(['a.png', 'b.png', 'c.png'])
    // Reorder without drag-and-drop.
    expect(picker.moveLater('file-1')).toBe(true)
    expect(picker.list().map((item) => item.file.name)).toEqual(['b.png', 'a.png', 'c.png'])
    expect(picker.moveEarlier('file-3')).toBe(true)
    expect(picker.list().map((item) => item.file.name)).toEqual(['b.png', 'c.png', 'a.png'])
    // The ends are no-ops: `file-1` (a.png) is now last, and `file-2` (b.png)
    // is first.
    expect(picker.moveLater('file-1')).toBe(false)
    expect(picker.moveEarlier('file-2')).toBe(false)
    // Removal: `file-2` is b.png, the first item.
    picker.remove('file-2')
    expect(picker.list().map((item) => item.file.name)).toEqual(['c.png', 'a.png'])
    expect(picker.prepared()).toHaveLength(2)
  })

  it('refuses a fifth file', async () => {
    const picker = new MediaPicker()
    for (let index = 0; index < MAX_ATTACHMENTS; index += 1) {
      await picker.add(pngFile(2, 2, `${index}.png`))
    }
    const fifth = await picker.add(pngFile(2, 2, 'fifth.png'))
    expect(fifth.prepared).toBeNull()
    expect(fifth.message).toContain('up to 4')
    expect(picker.list()).toHaveLength(MAX_ATTACHMENTS)
  })

  it('cancelAll drops everything', async () => {
    const picker = new MediaPicker()
    await picker.add(pngFile())
    picker.cancelAll()
    expect(picker.list()).toHaveLength(0)
  })
})
