import {
  ImageError,
  IMAGE_TYPES,
  MAX_IMAGE_BYTES,
  MAX_IMAGE_DIMENSION,
  MAX_IMAGE_PIXELS,
  inspectImage,
} from '../../shared/media/image-policy'
import type { ImageMime } from '../../shared/media/types'

/**
 * WP-05 T-409: client-side media preparation.
 *
 * A file is checked against the server's own limits *before* anything is
 * uploaded, so a user who picks a GIF learns immediately instead of after a
 * round trip. The limits are imported from the shared media policy, which is the
 * same source the server's sanitizer is configured from, so the two cannot
 * drift.
 *
 * The rejection messages are the messages the server would produce for the same
 * input: a client that says "GIF is not supported" and a server that says
 * "INVALID_MEDIA_TYPE" are describing one rule, and the user should read the
 * same sentence either way.
 *
 * Cancellation is honoured at every await, and a cancelled preparation leaves no
 * preview URL and no partial upload behind.
 */

/** The closed set of accepted types, as the server accepts them. */
export const ACCEPTED_TYPES: readonly string[] = IMAGE_TYPES

/** The `accept` attribute for a file input. */
export const ACCEPT_ATTRIBUTE = ACCEPTED_TYPES.join(',')

/** The user-facing message for each rejection. */
export const REJECTION_MESSAGE: Readonly<Record<string, string>> = Object.freeze({
  'invalid-type': 'Only JPEG, PNG and WebP images can be attached. GIF and video are not supported.',
  'invalid-bytes': `Images must be at most ${Math.round(MAX_IMAGE_BYTES / (1024 * 1024))} MiB.`,
  'invalid-dimensions': `Images must be at most ${MAX_IMAGE_DIMENSION} pixels on a side and ${MAX_IMAGE_PIXELS / 1_000_000} megapixels.`,
  'animated-image': 'Animated images are not supported. Attach a still image.',
  'decode-failed': 'That image could not be read. Try a different file.',
  aborted: 'Preparation was cancelled.',
})

export interface PreparedImage {
  readonly blob: Blob
  readonly width: number
  readonly height: number
  readonly type: ImageMime
}

export interface PreparationResult {
  readonly prepared: PreparedImage | null
  /** The user-facing message, or `null` on success. */
  readonly message: string | null
  readonly cancelled: boolean
}

/**
 * A synchronous pre-check, before any decoding work.
 *
 * This is what makes a GIF rejection instant: the type is refused from the
 * `File.type` alone, so no bytes are read.
 */
export function precheck(file: File): string | null {
  if (!ACCEPTED_TYPES.includes(file.type)) {
    return REJECTION_MESSAGE['invalid-type']!
  }
  if (file.size === 0 || file.size > MAX_IMAGE_BYTES) {
    return REJECTION_MESSAGE['invalid-bytes']!
  }
  return null
}

/**
 * Prepare a file: pre-check, then a header inspection that also refuses an
 * animated image and an out-of-range dimension.
 *
 * The inspection reads only the file header, so a 10 MiB photograph is not
 * decoded just to learn its size.
 */
export async function prepare(file: File, signal: AbortSignal): Promise<PreparationResult> {
  const rejected = precheck(file)
  if (rejected !== null) {
    return { prepared: null, message: rejected, cancelled: false }
  }
  try {
    const inspected = await inspectImage(file, signal)
    if (signal.aborted) {
      return { prepared: null, message: null, cancelled: true }
    }
    return {
      prepared: {
        blob: file,
        width: inspected.width,
        height: inspected.height,
        type: inspected.type,
      },
      message: null,
      cancelled: false,
    }
  } catch (error) {
    if (error instanceof ImageError) {
      if (error.code === 'aborted') {
        return { prepared: null, message: null, cancelled: true }
      }
      return {
        prepared: null,
        message: REJECTION_MESSAGE[error.code] ?? REJECTION_MESSAGE['decode-failed']!,
        cancelled: false,
      }
    }
    return { prepared: null, message: REJECTION_MESSAGE['decode-failed']!, cancelled: false }
  }
}

/** The maximum number of attachments a post may carry. */
export const MAX_ATTACHMENTS = 4

export interface SelectedFile {
  readonly id: string
  readonly file: File
  readonly prepared: PreparedImage | null
  readonly message: string | null
}

/**
 * The picker model: selection, preview, removal and ordering.
 *
 * Ordering is explicit (move earlier / move later) rather than drag-and-drop:
 * no `dnd-kit`, no pointer-only interaction, and every reorder is reachable from
 * the keyboard.
 */
export class MediaPicker {
  private items: SelectedFile[] = []
  private readonly aborters = new Map<string, AbortController>()
  private nextId = 0

  list(): readonly SelectedFile[] {
    return this.items
  }

  /** Add a file, preparing it in the background. Returns the new id. */
  async add(file: File): Promise<SelectedFile> {
    const id = `file-${++this.nextId}`
    if (this.items.length >= MAX_ATTACHMENTS) {
      return { id, file, prepared: null, message: `You can attach up to ${MAX_ATTACHMENTS} photographs.` }
    }
    const controller = new AbortController()
    this.aborters.set(id, controller)
    const entry: SelectedFile = { id, file, prepared: null, message: null }
    this.items = [...this.items, entry]
    const result = await prepare(file, controller.signal)
    this.aborters.delete(id)
    const updated: SelectedFile = {
      id,
      file,
      prepared: result.prepared,
      message: result.message,
    }
    this.items = this.items.map((item) => (item.id === id ? updated : item))
    return updated
  }

  /** Remove a file. A cancelled preparation leaves nothing behind. */
  remove(id: string): void {
    this.aborters.get(id)?.abort()
    this.aborters.delete(id)
    this.items = this.items.filter((item) => item.id !== id)
  }

  /** Move an item one position earlier. */
  moveEarlier(id: string): boolean {
    const index = this.items.findIndex((item) => item.id === id)
    if (index <= 0) return false
    const next = [...this.items]
    const [item] = next.splice(index, 1)
    next.splice(index - 1, 0, item!)
    this.items = next
    return true
  }

  /** Move an item one position later. */
  moveLater(id: string): boolean {
    const index = this.items.findIndex((item) => item.id === id)
    if (index < 0 || index >= this.items.length - 1) return false
    const next = [...this.items]
    const [item] = next.splice(index, 1)
    next.splice(index + 1, 0, item!)
    this.items = next
    return true
  }

  /** Cancel every in-flight preparation and drop everything. */
  cancelAll(): void {
    for (const controller of this.aborters.values()) controller.abort()
    this.aborters.clear()
    this.items = []
  }

  /** The prepared files, in order, ready to upload. */
  prepared(): readonly PreparedImage[] {
    return this.items.flatMap((item) => (item.prepared === null ? [] : [item.prepared]))
  }
}
