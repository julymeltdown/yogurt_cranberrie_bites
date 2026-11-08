// @vitest-environment jsdom
/**
 * WP-07 T-611: room persistence across sessions.
 *
 * The acceptance cases of the task are the first two tests:
 *
 * * `layout_equality_after_reload` — save, reload, and the layout is identical;
 * * `a_visitor_sees_the_saved_layout_while_the_owner_is_offline` — the visitor's
 *   view equals the owner's for a public room, with no owner session anywhere.
 *
 * "Reload" is modelled honestly: the server fixture keeps the stored room, and the
 * screen is unmounted and mounted again from scratch. Nothing is carried over in
 * memory, so what the second mount draws came from the server.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'

const drawn: unknown[] = []

vi.mock('../scene/room-scene', async () => {
  const actual = await vi.importActual<typeof import('../scene/room-scene')>('../scene/room-scene')
  return {
    ...actual,
    createRoomScene: async (options: { onReady?: () => void }) => {
      options.onReady?.()
      return {
        async render(layout: unknown) { drawn.push(structuredClone(layout)) },
        read: () => ({ textures: 0, objects: 0, destroyed: false }),
        destroy() { /* no resources in the double */ },
        closed: Promise.resolve(),
      }
    },
  }
})

import { RoomPage } from '../RoomPage'
import { createFixtureConverter, createRoomServer, OWNER, POSTER_MEDIA } from './fixtures'
import type { RoomLayout } from '../model/layout'

beforeEach(() => {
  drawn.length = 0
  vi.stubGlobal('ImageData', class {
    readonly data: Uint8ClampedArray
    constructor(data: Uint8ClampedArray) { this.data = data }
  })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement) {
    const canvas = this
    return {
      imageSmoothingEnabled: true, putImageData: vi.fn(), drawImage: vi.fn(), clearRect: vi.fn(),
      getImageData: () => ({ data: new Uint8ClampedArray(canvas.width * canvas.height * 4), width: canvas.width, height: canvas.height }),
    } as unknown as CanvasRenderingContext2D
  })
})
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const loadPoster = async () => ({ width: 64, height: 64 } as unknown as HTMLImageElement)

const editedLayout = (): RoomLayout => ({
  version: 1,
  wall: 'wallpaper',
  floor: 'rug',
  furniture: [
    { item_id: 'shelf', cell_x: 6, cell_y: 14, layer: 0 },
    { item_id: 'lamp', cell_x: 6, cell_y: 14, layer: 1 },
  ],
  posters: [{ poster_media_id: POSTER_MEDIA, slot: 0, x: 3, y: 2, width: 8, height: 8 }],
  character: { cell_x: 18, cell_y: 25, facing: 'LEFT' },
})

describe('room persistence', () => {
  it('layout_equality_after_reload', async () => {
    const server = createRoomServer()
    const first = render(<MemoryRouter><RoomPage client={server.client} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    // The owner edits the room and saves it.
    fireEvent.click(screen.getByLabelText('Wallpaper'))
    fireEvent.click(screen.getByLabelText('Rug'))
    fireEvent.click(screen.getByRole('button', { name: 'Add shelf' }))
    fireEvent.click(screen.getByRole('button', { name: 'Face left' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Save room' })).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Save room' }))
    await waitFor(() => expect(server.read().revision).toBe(2))
    const saved = server.read().layout
    expect(saved.wall).toBe('wallpaper')
    expect(saved.floor).toBe('rug')
    // Leaving the screen and coming back is a reload: the screen is gone, and the
    // next mount reads the room from the server again.
    first.unmount()
    drawn.length = 0
    render(<MemoryRouter><RoomPage client={server.client} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(drawn.at(-1)).toEqual(saved)
    // The editor's own draft is the stored layout too, and it is clean.
    expect(screen.getByText('All changes saved.')).toBeTruthy()
    expect(screen.getByText(/Room revision 2/)).toBeTruthy()
  })

  it('a_visitor_sees_the_saved_layout_while_the_owner_is_offline', async () => {
    const server = createRoomServer()
    server.set(editedLayout(), 5, 'PUBLIC')
    // The owner's session ends: the owner screen is never mounted, and there is no
    // presence, heartbeat or connection anywhere in this test.
    render(<MemoryRouter><RoomPage client={server.client} profileId={OWNER} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    const visitorView = drawn.at(-1) as RoomLayout
    expect(visitorView).toEqual(editedLayout())
    // The owner's view of the same stored room is identical.
    cleanup()
    drawn.length = 0
    render(<MemoryRouter><RoomPage client={server.client} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(drawn.at(-1)).toEqual(visitorView)
    // The visitor read the visitor path; the owner read the owner path.
    const paths = server.calls.map(call => `${call.method} ${call.path}`)
    expect(paths).toContain(`GET /api/profiles/${OWNER}/room`)
    expect(paths).toContain('GET /api/me/room')
  })

  it('a private room is not shown to a visitor at all', async () => {
    const server = createRoomServer()
    server.set(editedLayout(), 5, 'PRIVATE')
    server.mode('unavailable')
    render(<MemoryRouter><RoomPage client={server.client} profileId={OWNER} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(screen.getByText('This room is unavailable.')).toBeTruthy())
    expect(drawn).toHaveLength(0)
  })

  it('the saved layout survives a failed save attempt', async () => {
    const server = createRoomServer()
    server.set(editedLayout(), 5, 'PUBLIC')
    render(<MemoryRouter><RoomPage client={server.client} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    fireEvent.click(screen.getByLabelText('Brick'))
    server.mode('save-failed')
    fireEvent.click(screen.getByRole('button', { name: 'Save room' }))
    await waitFor(() => expect(screen.getAllByRole('status').map(node => node.textContent).join(' ')).toContain('could not be saved'))
    // The stored room is untouched; only the draft changed.
    expect(server.read().layout).toEqual(editedLayout())
    // Reloading shows the stored room, not the failed draft.
    cleanup()
    drawn.length = 0
    render(<MemoryRouter><RoomPage client={server.client} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(drawn.at(-1)).toEqual(editedLayout())
  })

  it('the room is stored per profile and a second profile never reads it', async () => {
    const server = createRoomServer()
    server.set(editedLayout(), 5, 'PUBLIC')
    const other = '00000000-0000-4000-8000-0000000007a9'
    render(<MemoryRouter><RoomPage client={server.client} profileId={other} loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    // The fixture server answers every profile with its own stored room; the point
    // asserted here is that the request names the profile being viewed.
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(server.calls[0]!.path).toBe(`/api/profiles/${other}/room`)
  })
})
