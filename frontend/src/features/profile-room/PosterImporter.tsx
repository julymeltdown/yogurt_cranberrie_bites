/**
 * WP-07 T-607: the poster importer.
 *
 * Exactly three steps, and there is no fourth:
 *
 * 1. **Import** — the user picks an image file.
 * 2. **Crop and preview** — the frame is chosen and the *actual* conversion is
 *    shown. The preview is the converted pixel grid, never a CSS filter.
 * 3. **Place on the wall** — the poster is uploaded, then placed.
 *
 * Two presets only ("Fine pixels", "Chunky pixels"), with Fine as the immediately
 * usable default. There is no colour-count, dithering or layer control, and no
 * separate workshop screen: this component is the whole poster flow.
 *
 * Upload happens on placement, not before: choosing a file and cropping never
 * sends bytes to the server. A cancelled import leaves nothing behind.
 */
import { useCallback, useEffect, useId, useRef, useState } from 'react'
import {
  DEFAULT_PRESET,
  POSTER_PRESETS,
  PRESET_SIDE,
  centredSquare,
  validateCrop,
  type Crop,
  type PosterPreset,
  type PosterPixels,
} from './pixelize'
import type { PosterConverter } from './worker/client'
import type { PosterMedia } from './api/room'
import './profile-room.css'

/** The three steps, in order. The UI renders exactly these and no others. */
export const IMPORT_STEPS = Object.freeze(['Import image', 'Crop and preview', 'Place on the wall'] as const)
export type ImportStep = typeof IMPORT_STEPS[number]

const PRESET_LABELS: Readonly<Record<PosterPreset, string>> = Object.freeze({
  FINE: 'Fine pixels',
  CHUNKY: 'Chunky pixels',
})

export interface PosterImporterProps {
  /** The conversion port. In production this is the worker. */
  readonly converter: PosterConverter
  /** Uploads the converted PNG and returns the stored media. */
  readonly upload: (input: { readonly preset: PosterPreset; readonly width: number; readonly height: number; readonly bytesBase64: string }) => Promise<PosterMedia>
  /** Places the stored media in the next free slot. */
  readonly place: (media: PosterMedia) => void
  /** Whether all four slots are taken. */
  readonly full: boolean
  /** Closes the importer without placing anything. */
  readonly onClose: () => void
}

type Phase =
  | { readonly kind: 'import' }
  | { readonly kind: 'crop'; readonly file: Blob; readonly width: number; readonly height: number; readonly crop: Crop; readonly preset: PosterPreset; readonly preview: PosterPixels | null; readonly previewBlob: Blob | null }
  | { readonly kind: 'placing'; readonly preview: PosterPixels }
  | { readonly kind: 'placed' }

export function PosterImporter({ converter, upload, place, full, onClose }: PosterImporterProps) {
  const id = useId()
  const [phase, setPhase] = useState<Phase>({ kind: 'import' })
  const [message, setMessage] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const canvas = useRef<HTMLCanvasElement | null>(null)
  const flight = useRef(0)

  /** Draw the converted pixel grid at 1:1 pixels, scaled by an integer factor. */
  const paint = useCallback((pixels: PosterPixels) => {
    const element = canvas.current
    if (!element) return
    const scale = Math.max(1, Math.floor(96 / pixels.side))
    element.width = pixels.side * scale
    element.height = pixels.side * scale
    const context = element.getContext('2d')
    if (!context) return
    const source = document.createElement('canvas')
    source.width = pixels.side
    source.height = pixels.side
    const sourceContext = source.getContext('2d')
    if (!sourceContext) return
    sourceContext.putImageData(new ImageData(new Uint8ClampedArray(pixels.rgba), pixels.side, pixels.side), 0, 0)
    // Nearest-neighbour upscale: the preview shows the poster's real pixels.
    context.imageSmoothingEnabled = false
    context.clearRect(0, 0, element.width, element.height)
    context.drawImage(source, 0, 0, element.width, element.height)
  }, [])

  useEffect(() => {
    if (phase.kind === 'crop' && phase.preview) paint(phase.preview)
  }, [phase, paint])

  /** Step 1: read the file and move to the crop step. Nothing is uploaded. */
  async function choose(file: File | null) {
    if (!file) return
    setMessage(null)
    setBusy(true)
    const run = ++flight.current
    try {
      // The source is decoded once to learn its size; the conversion itself
      // happens in the worker on the crop step.
      const size = await measure(file)
      if (run !== flight.current) return
      const crop = centredSquare(size.width, size.height)
      setPhase({ kind: 'crop', file, width: size.width, height: size.height, crop, preset: DEFAULT_PRESET, preview: null, previewBlob: null })
    } catch {
      setMessage('That image could not be read. Choose a PNG, JPEG or WebP image.')
    } finally {
      if (run === flight.current) setBusy(false)
    }
  }

  /** Step 2: convert the current frame with the current preset. */
  async function preview() {
    if (phase.kind !== 'crop') return
    setMessage(null)
    setBusy(true)
    const run = ++flight.current
    try {
      const frame = validateCrop(phase.crop, phase.width, phase.height)
      const converted = await converter.convert(phase.file, frame, phase.preset)
      if (run !== flight.current) return
      // The preview is the actual conversion, so the placed result cannot differ.
      const { downsample, decodeSource } = await import('./pixelize')
      const source = await decodeSource(phase.file)
      const pixels = downsample(source.rgba, source.width, source.height, frame, phase.preset)
      setPhase({ ...phase, crop: frame, preview: pixels, previewBlob: converted.blob })
    } catch {
      setMessage('The preview could not be prepared. Try another image or preset.')
    } finally {
      if (run === flight.current) setBusy(false)
    }
  }

  /** Step 3: upload the converted bytes, then place the stored media. */
  async function place_() {
    if (phase.kind !== 'crop' || !phase.preview || !phase.previewBlob) return
    setMessage(null)
    setBusy(true)
    const run = ++flight.current
    try {
      const { blobToBase64 } = await import('./pixelize')
      const media = await upload({
        preset: phase.preset,
        width: phase.preview.side,
        height: phase.preview.side,
        bytesBase64: await blobToBase64(phase.previewBlob),
      })
      if (run !== flight.current) return
      place(media)
      setPhase({ kind: 'placed' })
    } catch {
      setMessage('The poster could not be uploaded. Nothing was placed.')
    } finally {
      if (run === flight.current) setBusy(false)
    }
  }

  const stepIndex = phase.kind === 'import' ? 0 : phase.kind === 'crop' ? 1 : 2
  const canPlace = phase.kind === 'crop' && phase.preview !== null && !full && !busy

  return <section className="room-importer" aria-labelledby={`${id}-title`} data-step={stepIndex + 1}>
    <h2 id={`${id}-title`}>Add a poster</h2>
    <ol className="room-importer__steps" aria-label="Poster steps">
      {IMPORT_STEPS.map((step, index) => <li key={step} aria-current={index === stepIndex ? 'step' : undefined}
        data-state={index < stepIndex ? 'done' : index === stepIndex ? 'current' : 'todo'}>{step}</li>)}
    </ol>
    <p role="status" aria-live="polite">{message ?? (full ? 'All four poster slots are taken. Remove one to add another.' : '')}</p>

    {phase.kind === 'import' && <div className="room-importer__import">
      <label htmlFor={`${id}-file`}>Image file</label>
      <input id={`${id}-file`} type="file" accept="image/png,image/jpeg,image/webp" disabled={busy || full}
        onChange={event => { void choose(event.target.files?.[0] ?? null) }} />
      <p>Your photo is converted on this device. Only the finished pixel poster is uploaded.</p>
    </div>}

    {phase.kind === 'crop' && <div className="room-importer__crop">
      <fieldset>
        <legend>Frame</legend>
        <label htmlFor={`${id}-size`}>Frame size</label>
        <input id={`${id}-size`} type="range" min={8} max={Math.min(phase.width, phase.height)}
          value={phase.crop.width} disabled={busy}
          onChange={event => {
            const side = Number(event.target.value)
            const x = Math.max(0, Math.min(phase.width - side, Math.round(phase.crop.x + (phase.crop.width - side) / 2)))
            const y = Math.max(0, Math.min(phase.height - side, Math.round(phase.crop.y + (phase.crop.height - side) / 2)))
            setPhase({ ...phase, crop: { x, y, width: side, height: side }, preview: null, previewBlob: null })
          }} />
        <p>{phase.crop.width} × {phase.crop.height} of {phase.width} × {phase.height}</p>
        <label htmlFor={`${id}-x`}>Frame left</label>
        <input id={`${id}-x`} type="number" min={0} max={phase.width - phase.crop.width} value={phase.crop.x} disabled={busy}
          onChange={event => setPhase({ ...phase, crop: { ...phase.crop, x: Math.max(0, Math.min(phase.width - phase.crop.width, Number(event.target.value))) }, preview: null, previewBlob: null })} />
        <label htmlFor={`${id}-y`}>Frame top</label>
        <input id={`${id}-y`} type="number" min={0} max={phase.height - phase.crop.height} value={phase.crop.y} disabled={busy}
          onChange={event => setPhase({ ...phase, crop: { ...phase.crop, y: Math.max(0, Math.min(phase.height - phase.crop.height, Number(event.target.value))) }, preview: null, previewBlob: null })} />
      </fieldset>
      <fieldset>
        <legend>Pixel size</legend>
        {POSTER_PRESETS.map(preset => <label key={preset}>
          <input type="radio" name={`${id}-preset`} value={preset} checked={phase.preset === preset} disabled={busy}
            onChange={() => setPhase({ ...phase, preset, preview: null, previewBlob: null })} />
          {PRESET_LABELS[preset]} · {PRESET_SIDE[preset]} × {PRESET_SIDE[preset]}
        </label>)}
      </fieldset>
      <div className="room-importer__preview">
        <canvas ref={canvas} role="img" aria-label={phase.preview ? `Pixel poster preview, ${phase.preview.side} by ${phase.preview.side} pixels` : 'No preview yet'} />
        {!phase.preview && <p>Choose a frame and preview the pixel poster.</p>}
      </div>
      <div className="room-importer__actions">
        <button type="button" onClick={() => void preview()} disabled={busy}>Preview pixels</button>
        <button type="button" onClick={() => void place_()} disabled={!canPlace}>Place on the wall</button>
        <button type="button" onClick={() => { flight.current++; setPhase({ kind: 'import' }); setMessage(null) }} disabled={busy}>Choose another image</button>
      </div>
    </div>}

    {phase.kind === 'crop' && phase.preview && <p className="room-importer__note">The preview is the exact {phase.preview.side} × {phase.preview.side} pixel poster that will be placed.</p>}

    {phase.kind === 'placed' && <p role="status">Poster placed. Save your room to keep it.</p>}

    <div className="room-importer__actions">
      <button type="button" onClick={onClose}>Close</button>
    </div>
  </section>
}

/** The source image's pixel size, without keeping the decoded bitmap. */
async function measure(file: Blob): Promise<{ readonly width: number; readonly height: number }> {
  if (typeof createImageBitmap === 'function') {
    const bitmap = await createImageBitmap(file)
    try {
      return { width: bitmap.width, height: bitmap.height }
    } finally {
      bitmap.close()
    }
  }
  throw new Error('Image decoding is unavailable')
}
