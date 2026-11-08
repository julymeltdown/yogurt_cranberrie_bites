// @vitest-environment jsdom
/**
 * WP-07 T-607: the poster importer.
 *
 * The acceptance cases of the task are the first two tests:
 *
 * * `there_is_no_separate_workshop_screen` — exactly three steps, two presets, no
 *   colour/dithering/layer control;
 * * `the_preview_matches_the_placed_result` — the preview is the actual
 *   conversion, and the bytes that are uploaded are exactly the previewed ones.
 *
 * The conversion port is a fixture, so these tests assert the *flow*: what is
 * shown, when bytes are uploaded, and what is placed. The conversion itself is
 * proved in `pixelize.test.ts`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { IMPORT_STEPS, PosterImporter } from '../PosterImporter'
import { POSTER_PRESETS, PRESET_SIDE } from '../pixelize'
import { createFixtureConverter } from './fixtures'
import type { PosterMedia } from '../api/room'

const uploads: { preset: string; width: number; height: number; bytesBase64: string }[] = []
const placed: PosterMedia[] = []

beforeEach(() => {
  uploads.length = 0
  placed.length = 0
  vi.stubGlobal('ImageData', class {
    readonly data: Uint8ClampedArray
    constructor(data: Uint8ClampedArray) { this.data = data }
  })
  vi.stubGlobal('createImageBitmap', async () => ({
    width: 120,
    height: 80,
    close: vi.fn(),
  }))
  // jsdom has no canvas: the importer paints the preview and decodes the source
  // through it, so the double returns a full-size blank pixel grid.
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement) {
    const canvas = this
    return {
      imageSmoothingEnabled: true,
      putImageData: vi.fn(),
      drawImage: vi.fn(),
      clearRect: vi.fn(),
      getImageData: () => ({ data: new Uint8ClampedArray(canvas.width * canvas.height * 4), width: canvas.width, height: canvas.height }),
    } as unknown as CanvasRenderingContext2D
  })
})
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function renderImporter(options: { readonly full?: boolean; readonly onClose?: () => void } = {}) {
  const converter = createFixtureConverter()
  render(<PosterImporter converter={converter} full={options.full ?? false}
    onClose={options.onClose ?? (() => undefined)}
    upload={async input => {
      uploads.push(input)
      return { media_id: '11111111-1111-4111-8111-111111111111', state: 'READY', revision: 1, width: input.width, height: input.height, content_type: 'image/png', purpose: 'POST', read_path: '/api/profiles/room/media/1/content' }
    }}
    place={media => placed.push(media)} />)
  return converter
}

const chooseFile = () => {
  const input = screen.getByLabelText('Image file') as HTMLInputElement
  const file = new File(['fake-image-bytes'], 'photo.png', { type: 'image/png' })
  fireEvent.change(input, { target: { files: [file] } })
}

describe('poster importer steps', () => {
  it('there_is_no_separate_workshop_screen', () => {
    renderImporter()
    // Exactly three steps, in the frozen order.
    expect(IMPORT_STEPS).toEqual(['Import image', 'Crop and preview', 'Place on the wall'])
    const steps = screen.getAllByRole('listitem').filter(item => item.closest('.room-importer__steps'))
    expect(steps.map(step => step.textContent)).toEqual([...IMPORT_STEPS])
    // Only the import step is offered first.
    expect(screen.getByLabelText('Image file')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Place on the wall' })).toBeNull()
    // No colour-count, dithering or layer control exists anywhere.
    for (const forbidden of [/colou?r count/i, /dither/i, /layer/i, /palette/i, /brightness/i, /contrast/i]) {
      expect(screen.queryByText(forbidden)).toBeNull()
    }
  })

  it('offers exactly two presets with Fine as the default', async () => {
    renderImporter()
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    const radios = screen.getAllByRole('radio') as HTMLInputElement[]
    expect(radios.map(radio => radio.value)).toEqual([...POSTER_PRESETS])
    expect(radios[0]!.checked).toBe(true)
    // The labels spell the pixel size, so the user knows what they get.
    expect(screen.getByText(new RegExp(`Fine pixels · ${PRESET_SIDE.FINE} × ${PRESET_SIDE.FINE}`))).toBeTruthy()
    expect(screen.getByText(new RegExp(`Chunky pixels · ${PRESET_SIDE.CHUNKY} × ${PRESET_SIDE.CHUNKY}`))).toBeTruthy()
  })

  it('nothing is uploaded before placement', async () => {
    const converter = renderImporter()
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Preview pixels' }))
    await waitFor(() => expect(converter.calls).toBe(1))
    // The preview ran, and no bytes were sent.
    expect(uploads).toHaveLength(0)
    expect(placed).toHaveLength(0)
  })
})

describe('poster importer preview and placement', () => {
  it('the_preview_matches_the_placed_result', async () => {
    const converter = renderImporter()
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Preview pixels' }))
    await waitFor(() => expect(screen.getByText(/The preview is the exact 64 × 64 pixel poster/)).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Place on the wall' }))
    await waitFor(() => expect(placed).toHaveLength(1))
    // The uploaded bytes are the previewed conversion: same preset, same size,
    // and the base64 of exactly the blob the converter returned.
    expect(uploads).toHaveLength(1)
    expect(uploads[0]!.preset).toBe('FINE')
    expect(uploads[0]!.width).toBe(PRESET_SIDE.FINE)
    expect(uploads[0]!.height).toBe(PRESET_SIDE.FINE)
    expect(atob(uploads[0]!.bytesBase64)).toBe(`PNG:FINE:${PRESET_SIDE.FINE}`)
    // The placed media is the stored media the upload returned.
    expect(placed[0]!.media_id).toBe('11111111-1111-4111-8111-111111111111')
    expect(converter.calls).toBe(1)
  })

  it('a preset change re-runs the conversion with the other preset', async () => {
    const converter = renderImporter()
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    fireEvent.click(screen.getByLabelText(/Chunky pixels/))
    fireEvent.click(screen.getByRole('button', { name: 'Preview pixels' }))
    await waitFor(() => expect(screen.getByText(/The preview is the exact 32 × 32 pixel poster/)).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Place on the wall' }))
    await waitFor(() => expect(placed).toHaveLength(1))
    expect(uploads[0]!.preset).toBe('CHUNKY')
    expect(uploads[0]!.width).toBe(PRESET_SIDE.CHUNKY)
    expect(converter.calls).toBe(1)
  })

  it('placement is impossible without a preview', async () => {
    renderImporter()
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    const place = screen.getByRole('button', { name: 'Place on the wall' }) as HTMLButtonElement
    expect(place.disabled).toBe(true)
    expect(uploads).toHaveLength(0)
  })

  it('a full room refuses the import rather than uploading into nothing', async () => {
    renderImporter({ full: true })
    const input = screen.getByLabelText('Image file') as HTMLInputElement
    expect(input.disabled).toBe(true)
    expect(screen.getByRole('status').textContent).toContain('All four poster slots are taken')
  })

  it('a failed upload places nothing and says so', async () => {
    renderImporter()
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Preview pixels' }))
    await waitFor(() => expect(screen.getByText(/The preview is the exact/)).toBeTruthy())
    // Replace the upload port with one that fails.
    cleanup()
    const converter = createFixtureConverter()
    render(<PosterImporter converter={converter} full={false} onClose={() => undefined}
      upload={async () => { throw new Error('invalid') }} place={media => placed.push(media)} />)
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Preview pixels' }))
    await waitFor(() => expect(screen.getByText(/The preview is the exact/)).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Place on the wall' }))
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('could not be uploaded'))
    expect(placed).toHaveLength(0)
  })

  it('closing the importer places nothing', async () => {
    const onClose = vi.fn()
    renderImporter({ onClose })
    chooseFile()
    await waitFor(() => expect(screen.getByLabelText('Frame size')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(onClose).toHaveBeenCalled()
    expect(uploads).toHaveLength(0)
    expect(placed).toHaveLength(0)
  })
})
