/**
 * WP-07 T-606: the poster conversion worker.
 *
 * The conversion runs off the UI thread so a large source image cannot freeze
 * the room screen while the user is cropping. The worker owns the whole
 * conversion: decode, crop, nearest-neighbour downsample, PNG encode. It returns
 * the encoded bytes and the pixel grid, never the source photo.
 *
 * The worker is a module worker (`?worker` is not used: the file is loaded with
 * `new Worker(new URL(...), { type: 'module' })`), so the same code path runs in
 * the browser and under the test runner.
 *
 * Messages:
 *
 * * in: `{ id, type: 'convert', png: ArrayBuffer, crop, preset }`
 * * out: `{ id, type: 'converted', png: ArrayBuffer, width, height, preset }`
 * * out: `{ id, type: 'failed', reason }`
 */
import { convertToPoster, type Crop, type PosterPreset } from '../pixelize'

export interface ConvertRequest {
  readonly id: number
  readonly type: 'convert'
  readonly png: ArrayBuffer
  readonly crop: Crop
  readonly preset: PosterPreset
}

export type ConvertReply =
  | { readonly id: number; readonly type: 'converted'; readonly png: ArrayBuffer; readonly width: number; readonly height: number; readonly preset: PosterPreset }
  | { readonly id: number; readonly type: 'failed'; readonly reason: string }

/** The message handler, exported so the worker entry and the test harness share it. */
export async function handleConvert(request: ConvertRequest): Promise<ConvertReply> {
  try {
    const source = new Blob([request.png], { type: 'image/png' })
    const { blob, pixels } = await convertToPoster(source, request.crop, request.preset)
    return {
      id: request.id,
      type: 'converted',
      png: await blob.arrayBuffer(),
      width: pixels.side,
      height: pixels.side,
      preset: pixels.preset,
    }
  } catch (error) {
    return {
      id: request.id,
      type: 'failed',
      reason: error instanceof Error ? error.name : 'PosterConversionError',
    }
  }
}
