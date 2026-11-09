/**
 * WP-07 T-606: the poster pixel conversion.
 *
 * Contract (domain-contract.md §6): input is a user image cropped to the poster
 * frame; output is a PNG at the poster's native pixel size.
 *
 * | Preset | Target pixel size | Character |
 * | --- | --- | --- |
 * | Fine pixels | 64 × 64 | More detail, still visibly pixel art |
 * | Chunky pixels | 32 × 32 | Bold, poster-like |
 *
 * The conversion is **deterministic**: the same input bytes, crop and preset
 * produce byte-identical PNGs. That is what makes the preview the placed result:
 * the user sees exactly the pixels the server will store, and re-running the same
 * conversion cannot produce a different poster.
 *
 * Two rules make determinism real rather than hoped for:
 *
 * * the sampling is nearest-neighbour with an integer cell map computed from the
 *   crop rectangle, so no floating-point resampling filter can vary between
 *   engines or runs;
 * * the encoder is the browser's own PNG encoder (`toBlob('image/png')`), and the
 *   *pixels* are the deterministic part. The bytes are asserted byte-identical in
 *   the test suite against the same canvas implementation, and the conversion is
 *   pure in its inputs: no clock, no randomness, no device-dependent state.
 *
 * There is deliberately no colour-count, dithering or layer control. The default
 * preset is immediately usable; the user is never asked about colour.
 */

/** The two presets. There are exactly two, and there is no third. */
export const POSTER_PRESETS = Object.freeze(['FINE', 'CHUNKY'] as const)
export type PosterPreset = typeof POSTER_PRESETS[number]

/** The native pixel size of each preset. */
export const PRESET_SIDE: Readonly<Record<PosterPreset, number>> = Object.freeze({ FINE: 64, CHUNKY: 32 })

/** The default preset: immediately usable, no question asked. */
export const DEFAULT_PRESET: PosterPreset = 'FINE'

/** The largest accepted poster byte length, matching the server's bound. */
export const MAX_POSTER_BYTES = 1024 * 1024

/** A crop rectangle in source-image pixels. Integers, inside the source. */
export interface Crop {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** The pixels of one converted poster: the exact RGBA grid that becomes the PNG. */
export interface PosterPixels {
  readonly preset: PosterPreset
  readonly side: number
  readonly rgba: Uint8ClampedArray<ArrayBuffer>
}

export class PosterConversionError extends Error {
  constructor(readonly reason: 'invalid-crop' | 'invalid-preset' | 'too-large' | 'decode-failed' | 'encode-failed') {
    super(`Poster conversion failed: ${reason}.`)
    this.name = 'PosterConversionError'
  }
}

/** The smallest crop that is still a poster. */
const MIN_CROP = 1

/**
 * The source rectangle a crop addresses, validated against the source size.
 *
 * A crop is refused rather than clamped: silently moving the user's frame is how
 * a preview stops matching what the user chose.
 */
export function validateCrop(crop: Crop, sourceWidth: number, sourceHeight: number): Crop {
  const values = [crop.x, crop.y, crop.width, crop.height]
  if (!values.every(value => Number.isSafeInteger(value))
    || !Number.isSafeInteger(sourceWidth) || !Number.isSafeInteger(sourceHeight)
    || sourceWidth < 1 || sourceHeight < 1
    || crop.width < MIN_CROP || crop.height < MIN_CROP
    || crop.x < 0 || crop.y < 0
    || crop.x + crop.width > sourceWidth || crop.y + crop.height > sourceHeight) {
    throw new PosterConversionError('invalid-crop')
  }
  return Object.freeze({ x: crop.x, y: crop.y, width: crop.width, height: crop.height })
}

/** A square crop centred on the source, which is the default frame. */
export function centredSquare(sourceWidth: number, sourceHeight: number): Crop {
  const side = Math.min(sourceWidth, sourceHeight)
  return Object.freeze({
    x: Math.floor((sourceWidth - side) / 2),
    y: Math.floor((sourceHeight - side) / 2),
    width: side,
    height: side,
  })
}

/**
 * Nearest-neighbour downsample of one crop to `side × side` RGBA pixels.
 *
 * The sampling is pure integer arithmetic: source pixel `i` of the target maps to
 * `crop.x + floor(i * crop.width / side)`. Two runs on the same input therefore
 * produce the same array, element for element, on every engine.
 *
 * `source` is the full source image in RGBA row-major order with `sourceWidth`
 * pixels per row.
 */
export function downsample(
  source: Uint8ClampedArray<ArrayBufferLike> | Uint8Array<ArrayBufferLike>,
  sourceWidth: number,
  sourceHeight: number,
  crop: Crop,
  preset: PosterPreset,
): PosterPixels {
  const side = PRESET_SIDE[preset]
  if (side === undefined) throw new PosterConversionError('invalid-preset')
  const frame = validateCrop(crop, sourceWidth, sourceHeight)
  if (source.length !== sourceWidth * sourceHeight * 4) throw new PosterConversionError('decode-failed')
  const rgba = new Uint8ClampedArray(side * side * 4)
  for (let y = 0; y < side; y++) {
    const sourceY = frame.y + Math.floor((y * frame.height) / side)
    for (let x = 0; x < side; x++) {
      const sourceX = frame.x + Math.floor((x * frame.width) / side)
      const from = (sourceY * sourceWidth + sourceX) * 4
      const to = (y * side + x) * 4
      rgba[to] = source[from]!
      rgba[to + 1] = source[from + 1]!
      rgba[to + 2] = source[from + 2]!
      // Every poster is opaque: a transparent source pixel would otherwise show
      // the wall through the poster, which is not a poster.
      rgba[to + 3] = 255
    }
  }
  return Object.freeze({ preset, side, rgba })
}

/** Encode RGBA pixels as a PNG Blob through the platform's canvas encoder. */
export async function encodePng(pixels: PosterPixels): Promise<Blob> {
  const canvas = document.createElement('canvas')
  canvas.width = pixels.side
  canvas.height = pixels.side
  const context = canvas.getContext('2d')
  if (!context) throw new PosterConversionError('encode-failed')
  // `putImageData` writes the exact pixels; no scaling, smoothing or colour
  // management is applied by this step.
  context.imageSmoothingEnabled = false
  const image = new ImageData(new Uint8ClampedArray(pixels.rgba), pixels.side, pixels.side)
  context.putImageData(image, 0, 0)
  const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, 'image/png'))
  if (!blob) throw new PosterConversionError('encode-failed')
  if (blob.size > MAX_POSTER_BYTES) throw new PosterConversionError('too-large')
  return blob
}

/** Decode an image Blob to its full RGBA pixel grid. */
export async function decodeSource(blob: Blob): Promise<{ readonly width: number; readonly height: number; readonly rgba: Uint8ClampedArray<ArrayBuffer> }> {
  const bitmap = await createImageBitmap(blob)
  try {
    if (bitmap.width < 1 || bitmap.height < 1) throw new PosterConversionError('decode-failed')
    const canvas = document.createElement('canvas')
    canvas.width = bitmap.width
    canvas.height = bitmap.height
    const context = canvas.getContext('2d')
    if (!context) throw new PosterConversionError('decode-failed')
    context.drawImage(bitmap, 0, 0)
    const data = context.getImageData(0, 0, bitmap.width, bitmap.height)
    return Object.freeze({ width: bitmap.width, height: bitmap.height, rgba: new Uint8ClampedArray(data.data) })
  } finally {
    bitmap.close()
  }
}

/** The complete conversion: decode, crop, downsample, encode. */
export async function convertToPoster(blob: Blob, crop: Crop, preset: PosterPreset): Promise<{ readonly blob: Blob; readonly pixels: PosterPixels }> {
  const source = await decodeSource(blob)
  const pixels = downsample(source.rgba, source.width, source.height, crop, preset)
  return Object.freeze({ blob: await encodePng(pixels), pixels })
}

/** Read a Blob as base64 without the data-URL prefix, for the upload body. */
export async function blobToBase64(blob: Blob): Promise<string> {
  const bytes = new Uint8Array(await blob.arrayBuffer())
  let binary = ''
  // Chunked so a large poster cannot blow the argument limit of `fromCharCode`.
  const chunk = 0x8000
  for (let offset = 0; offset < bytes.length; offset += chunk) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunk))
  }
  return btoa(binary)
}

/** A stable digest of the converted pixels, used to prove determinism in tests. */
export function pixelDigest(pixels: PosterPixels): string {
  // FNV-1a over the RGBA bytes: cheap, dependency-free and stable.
  let hash = 0x811c9dc5
  for (const byte of pixels.rgba) {
    hash ^= byte
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return `${pixels.preset}:${pixels.side}:${pixels.rgba.length}:${hash.toString(16).padStart(8, '0')}`
}
