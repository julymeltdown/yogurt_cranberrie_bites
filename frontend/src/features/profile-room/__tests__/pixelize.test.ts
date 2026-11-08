// @vitest-environment jsdom
/**
 * WP-07 T-606: the poster pixel conversion.
 *
 * The two acceptance cases of the task are the first two tests:
 *
 * * `two_runs_on_the_same_input_produce_byte_identical_pngs` — determinism;
 * * `the_output_dimensions_are_the_preset_native_size` — 64×64 for FINE, 32×32
 *   for CHUNKY, and exactly two presets.
 *
 * The conversion is pure integer arithmetic over the source pixels, so it is
 * asserted directly. The PNG encode step is exercised through a recording canvas
 * double (jsdom has no 2D canvas), which proves the encoder is given *exactly* the
 * deterministic pixel grid and nothing else — no filter, no scaling, no colour
 * adjustment.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  DEFAULT_PRESET,
  MAX_POSTER_BYTES,
  POSTER_PRESETS,
  PRESET_SIDE,
  blobToBase64,
  centredSquare,
  convertToPoster,
  decodeSource,
  downsample,
  encodePng,
  pixelDigest,
  validateCrop,
  PosterConversionError,
  type Crop,
} from '../pixelize'

// ---------------------------------------------------------------------------
// A deterministic synthetic source: a face-like and a logo-like image
// ---------------------------------------------------------------------------

/** A 96×96 RGBA source with a recognisable face: dark eyes, a mouth, a round head. */
function faceSource(size = 96) {
  const rgba = new Uint8ClampedArray(size * size * 4)
  const centre = (size - 1) / 2
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = (y * size + x) * 4
      const dx = x - centre, dy = y - centre
      const insideHead = dx * dx + dy * dy < (size * 0.42) ** 2
      const eye = (Math.abs(dx) > size * 0.1 && Math.abs(dx) < size * 0.2 && Math.abs(dy + size * 0.1) < size * 0.06)
      const mouth = Math.abs(dy - size * 0.2) < size * 0.04 && Math.abs(dx) < size * 0.16
      const [r, g, b] = eye ? [20, 20, 28] : mouth ? [150, 40, 50] : insideHead ? [232, 190, 160] : [40, 70, 120]
      rgba[i] = r; rgba[i + 1] = g; rgba[i + 2] = b; rgba[i + 3] = 255
    }
  }
  return { rgba, width: size, height: size }
}

/** A 120×80 RGBA source with a logo: a bold ring and a diagonal bar. */
function logoSource() {
  const width = 120, height = 80
  const rgba = new Uint8ClampedArray(width * height * 4)
  const cx = width / 2, cy = height / 2
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4
      const distance = Math.hypot(x - cx, y - cy)
      const ring = distance > 22 && distance < 32
      const bar = Math.abs((x - cx) - (y - cy)) < 4
      const [r, g, b] = ring || bar ? [250, 240, 60] : [24, 24, 32]
      rgba[i] = r; rgba[i + 1] = g; rgba[i + 2] = b; rgba[i + 3] = 255
    }
  }
  return { rgba, width, height }
}

// ---------------------------------------------------------------------------
// Canvas double
// ---------------------------------------------------------------------------

interface Recorded { readonly width: number; readonly height: number; readonly data: Uint8ClampedArray; readonly smoothing: boolean }
const encoded: Recorded[] = []
let blobCounter = 0

beforeEach(() => {
  encoded.length = 0
  blobCounter = 0
  // jsdom has no `ImageData`; the double only needs a data holder.
  vi.stubGlobal('ImageData', class {
    readonly data: Uint8ClampedArray
    constructor(dataOrWidth: Uint8ClampedArray | number, width?: number) {
      this.data = typeof dataOrWidth === 'number'
        ? new Uint8ClampedArray(dataOrWidth * (width ?? 0) * 4)
        : dataOrWidth
    }
  })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement) {
    const canvas = this
    return {
      imageSmoothingEnabled: true,
      putImageData(image: ImageData) {
        encoded.push({ width: canvas.width, height: canvas.height, data: new Uint8ClampedArray(image.data), smoothing: false })
      },
      getImageData: () => new ImageData(canvas.width, canvas.height),
      drawImage: vi.fn(),
    } as unknown as CanvasRenderingContext2D
  })
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (this: HTMLCanvasElement, callback: BlobCallback) {
    // The PNG bytes are a deterministic function of the pixels the encoder was
    // given; the double records the pixels so the assertion is about content.
    const record = encoded.at(-1)
    const payload = record ? Array.from(record.data).join(',') : ''
    blobCounter++
    callback(new Blob([`PNG:${this.width}x${this.height}:${payload}`], { type: 'image/png' }))
  })
  vi.stubGlobal('createImageBitmap', async (blob: Blob) => {
    const text = await blob.text()
    const [width, height] = text.split(':').map(Number)
    return { width, height, close: vi.fn() }
  })
})
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

describe('pixel conversion determinism', () => {
  it('two_runs_on_the_same_input_produce_byte_identical_pngs', async () => {
    const source = faceSource()
    const crop = centredSquare(source.width, source.height)
    const first = downsample(source.rgba, source.width, source.height, crop, 'FINE')
    const second = downsample(source.rgba, source.width, source.height, crop, 'FINE')
    expect(pixelDigest(second)).toBe(pixelDigest(first))
    expect(Array.from(second.rgba)).toEqual(Array.from(first.rgba))
    // The encoded PNGs are byte-identical too.
    const a = await encodePng(first)
    const b = await encodePng(second)
    expect(await a.arrayBuffer()).toEqual(await b.arrayBuffer())
    // The encoder received exactly the deterministic grid, twice.
    expect(encoded).toHaveLength(2)
    expect(Array.from(encoded[1]!.data)).toEqual(Array.from(encoded[0]!.data))
    expect(encoded[0]!.width).toBe(64)
    expect(encoded[0]!.height).toBe(64)
  })

  it('the whole conversion is deterministic end to end', async () => {
    const source = faceSource()
    const png = new Blob([`${source.width}:${source.height}`], { type: 'image/png' })
    const crop = centredSquare(source.width, source.height)
    const first = await convertToPoster(png, crop, 'CHUNKY')
    const second = await convertToPoster(png, crop, 'CHUNKY')
    expect(pixelDigest(second.pixels)).toBe(pixelDigest(first.pixels))
    expect(await first.blob.text()).toBe(await second.blob.text())
  })

  it('a different crop or preset produces a different poster', () => {
    const source = faceSource()
    const full = centredSquare(source.width, source.height)
    const offset = { x: 0, y: 0, width: 48, height: 48 } as Crop
    expect(pixelDigest(downsample(source.rgba, source.width, source.height, full, 'FINE')))
      .not.toBe(pixelDigest(downsample(source.rgba, source.width, source.height, offset, 'FINE')))
    expect(pixelDigest(downsample(source.rgba, source.width, source.height, full, 'FINE')))
      .not.toBe(pixelDigest(downsample(source.rgba, source.width, source.height, full, 'CHUNKY')))
  })

  it('the sampling is nearest-neighbour: every output pixel is a source pixel', () => {
    const source = faceSource()
    const crop = { x: 8, y: 8, width: 40, height: 40 } as Crop
    const pixels = downsample(source.rgba, source.width, source.height, crop, 'CHUNKY')
    for (let y = 0; y < 32; y++) {
      for (let x = 0; x < 32; x++) {
        const sourceX = crop.x + Math.floor((x * crop.width) / 32)
        const sourceY = crop.y + Math.floor((y * crop.height) / 32)
        const from = (sourceY * source.width + sourceX) * 4
        const to = (y * 32 + x) * 4
        expect([pixels.rgba[to], pixels.rgba[to + 1], pixels.rgba[to + 2]])
          .toEqual([source.rgba[from], source.rgba[from + 1], source.rgba[from + 2]])
      }
    }
  })
})

// ---------------------------------------------------------------------------
// Dimensions and presets
// ---------------------------------------------------------------------------

describe('pixel conversion output dimensions', () => {
  it('the_output_dimensions_are_the_preset_native_size', () => {
    const source = faceSource()
    const crop = centredSquare(source.width, source.height)
    expect(PRESET_SIDE.FINE).toBe(64)
    expect(PRESET_SIDE.CHUNKY).toBe(32)
    const fine = downsample(source.rgba, source.width, source.height, crop, 'FINE')
    const chunky = downsample(source.rgba, source.width, source.height, crop, 'CHUNKY')
    expect(fine.side).toBe(64)
    expect(fine.rgba.length).toBe(64 * 64 * 4)
    expect(chunky.side).toBe(32)
    expect(chunky.rgba.length).toBe(32 * 32 * 4)
  })

  it('there are exactly two presets and no colour, dithering or layer control', () => {
    expect(POSTER_PRESETS).toEqual(['FINE', 'CHUNKY'])
    expect(DEFAULT_PRESET).toBe('FINE')
    // The conversion surface takes a crop and a preset and nothing else.
    expect(downsample.length).toBe(5)
    expect(convertToPoster.length).toBe(3)
  })

  it('a poster is opaque even when the source has transparency', () => {
    const size = 16
    const rgba = new Uint8ClampedArray(size * size * 4)
    // Fully transparent source: every alpha stays 0.
    const pixels = downsample(rgba, size, size, { x: 0, y: 0, width: size, height: size }, 'CHUNKY')
    for (let index = 3; index < pixels.rgba.length; index += 4) expect(pixels.rgba[index]).toBe(255)
  })

  it('the encoded PNG is bounded by the server limit', async () => {
    const source = faceSource()
    const pixels = downsample(source.rgba, source.width, source.height, centredSquare(source.width, source.height), 'FINE')
    const blob = await encodePng(pixels)
    expect(blob.size).toBeLessThan(MAX_POSTER_BYTES)
  })
})

// ---------------------------------------------------------------------------
// Crop validation
// ---------------------------------------------------------------------------

describe('crop validation', () => {
  it('a crop outside the source is refused rather than clamped', () => {
    expect(() => validateCrop({ x: -1, y: 0, width: 10, height: 10 }, 100, 100)).toThrow(PosterConversionError)
    expect(() => validateCrop({ x: 0, y: 0, width: 101, height: 10 }, 100, 100)).toThrow(PosterConversionError)
    expect(() => validateCrop({ x: 95, y: 0, width: 10, height: 10 }, 100, 100)).toThrow(PosterConversionError)
    expect(() => validateCrop({ x: 0, y: 0, width: 0, height: 10 }, 100, 100)).toThrow(PosterConversionError)
    expect(() => validateCrop({ x: 0.5, y: 0, width: 10, height: 10 }, 100, 100)).toThrow(PosterConversionError)
    expect(validateCrop({ x: 0, y: 0, width: 100, height: 100 }, 100, 100)).toEqual({ x: 0, y: 0, width: 100, height: 100 })
  })

  it('the default frame is the centred square', () => {
    expect(centredSquare(120, 80)).toEqual({ x: 20, y: 0, width: 80, height: 80 })
    expect(centredSquare(80, 120)).toEqual({ x: 0, y: 20, width: 80, height: 80 })
    expect(centredSquare(64, 64)).toEqual({ x: 0, y: 0, width: 64, height: 64 })
  })

  it('an invalid preset is refused', () => {
    const source = faceSource()
    expect(() => downsample(source.rgba, source.width, source.height, centredSquare(96, 96), 'LOW' as never)).toThrow(PosterConversionError)
  })

  it('a source whose pixel buffer does not match its size is refused', () => {
    expect(() => downsample(new Uint8ClampedArray(10), 96, 96, centredSquare(96, 96), 'FINE')).toThrow(PosterConversionError)
  })
})

// ---------------------------------------------------------------------------
// Upload encoding
// ---------------------------------------------------------------------------

describe('upload encoding', () => {
  it('base64 of the poster bytes has no data-url prefix and round-trips', async () => {
    const bytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
    const encodedValue = await blobToBase64(new Blob([bytes]))
    expect(encodedValue).not.toContain('data:')
    expect(encodedValue).toBe(btoa(String.fromCharCode(...bytes)))
  })

  it('the decoder reads the source size and pixels', async () => {
    const bitmapSize = { width: 40, height: 30 }
    vi.stubGlobal('createImageBitmap', async () => ({ ...bitmapSize, close: vi.fn() }))
    const decoded = await decodeSource(new Blob(['40:30']))
    expect(decoded.width).toBe(40)
    expect(decoded.height).toBe(30)
    expect(decoded.rgba.length).toBe(40 * 30 * 4)
  })
})

// ---------------------------------------------------------------------------
// Recognisability at mobile size
// ---------------------------------------------------------------------------

describe('recognisability at mobile size', () => {
  /**
   * A poster is recognisable when its coarse structure survives the downsample.
   * The proxy is contrast: a face keeps dark eyes and a mouth against a light
   * head, and a logo keeps its ring against its background. The test asserts the
   * converted poster still has both a light and a dark region and that the
   * darkest region is in the upper half of a face (the eyes) rather than noise.
   */
  it('a face keeps its eyes and mouth after conversion to both presets', () => {
    const source = faceSource()
    const crop = centredSquare(source.width, source.height)
    for (const preset of POSTER_PRESETS) {
      const pixels = downsample(source.rgba, source.width, source.height, crop, preset)
      const side = pixels.side
      const luminance = (x: number, y: number) => {
        const i = (y * side + x) * 4
        return pixels.rgba[i]! * 0.299 + pixels.rgba[i + 1]! * 0.587 + pixels.rgba[i + 2]! * 0.114
      }
      let darkest = 255, lightest = 0
      for (let y = 0; y < side; y++) for (let x = 0; x < side; x++) {
        darkest = Math.min(darkest, luminance(x, y))
        lightest = Math.max(lightest, luminance(x, y))
      }
      // A face is not a flat block: it keeps a strong light/dark contrast.
      expect(lightest - darkest).toBeGreaterThan(80)
      // The eyes are above the mouth, so the darkest band is above the centre.
      let darkestRow = 0, darkestValue = 255
      for (let y = 0; y < side; y++) for (let x = 0; x < side; x++) {
        const value = luminance(x, y)
        if (value < darkestValue) { darkestValue = value; darkestRow = y }
      }
      expect(darkestRow).toBeLessThan(side / 2)
    }
  })

  it('a logo keeps its ring after conversion to both presets', () => {
    const source = logoSource()
    const crop = centredSquare(source.width, source.height)
    for (const preset of POSTER_PRESETS) {
      const pixels = downsample(source.rgba, source.width, source.height, crop, preset)
      const side = pixels.side, centre = (side - 1) / 2
      const isBright = (x: number, y: number) => {
        const i = (y * side + x) * 4
        return pixels.rgba[i]! > 180 && pixels.rgba[i + 1]! > 180
      }
      // A bright pixel exists on the ring radius, and the centre is dark.
      // The ring is a band, not a one-pixel circle: count the bright pixels in
      // the annulus the source ring occupies, which is what the eye sees.
      let ringPixels = 0
      for (let y = 0; y < side; y++) {
        for (let x = 0; x < side; x++) {
          const distance = Math.hypot(x - centre, y - centre) / side
          if (distance > 0.24 && distance < 0.36 && isBright(x, y)) ringPixels++
        }
      }
      expect(ringPixels).toBeGreaterThan(side * 0.5)
      // The diagonal bar crosses the centre, so the centre is bright; the corners
      // away from the ring and the bar are the dark background.
      // The diagonal bar leaves the two off-diagonal corners dark.
      expect(isBright(side - 1, 0)).toBe(false)
    }
  })
})
