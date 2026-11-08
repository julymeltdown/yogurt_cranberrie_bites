// @vitest-environment jsdom
/**
 * WP-07 T-609: the room editor screen.
 *
 * The acceptance cases of the task are the first two tests:
 *
 * * `a_conflict_never_silently_discards_the_users_layout` — the draft survives a
 *   `409` and the resolution is an explicit choice;
 * * `every_placement_is_reachable_by_keyboard` — furniture, posters, the character,
 *   wall and floor all have a keyboard path.
 *
 * The scene is replaced by a recording double (Phaser cannot run in jsdom) so the
 * tests assert what the screen asks the scene to draw, and the room server is the
 * fixture that speaks the five frozen operations.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'

const drawn: unknown[] = []
const scenes: { destroyed: boolean }[] = []

vi.mock('../scene/room-scene', async () => {
  const actual = await vi.importActual<typeof import('../scene/room-scene')>('../scene/room-scene')
  return {
    ...actual,
    createRoomScene: async (options: { onReady?: () => void }) => {
      const record = { destroyed: false }
      scenes.push(record)
      options.onReady?.()
      return {
        async render(layout: unknown) { drawn.push(structuredClone(layout)) },
        read: () => ({ textures: 0, objects: 0, destroyed: record.destroyed }),
        destroy() { record.destroyed = true },
        closed: Promise.resolve(),
      }
    },
  }
})

import { RoomPage } from '../RoomPage'
import { createFixtureConverter, createRoomServer, POSTER_MEDIA } from './fixtures'
import { initialLayout, type RoomLayout } from '../model/layout'

beforeEach(() => {
  drawn.length = 0
  scenes.length = 0
  vi.stubGlobal('ImageData', class {
    readonly data: Uint8ClampedArray
    constructor(data: Uint8ClampedArray) { this.data = data }
  })
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

const loadPoster = async () => ({ width: 64, height: 64 } as unknown as HTMLImageElement)

function renderRoom(server = createRoomServer()) {
  const converter = createFixtureConverter()
  const view = render(<MemoryRouter><RoomPage client={server.client} loadPoster={loadPoster} converter={converter} /></MemoryRouter>)
  return { server, converter, view }
}

const savedLayout = (): RoomLayout => ({
  ...initialLayout(),
  wall: 'brick',
  floor: 'tile',
  furniture: [{ item_id: 'sofa', cell_x: 3, cell_y: 18, layer: 0 }],
  character: { cell_x: 9, cell_y: 27, facing: 'LEFT' },
})

describe('room editor', () => {
  it('loads the saved room and draws it', async () => {
    const server = createRoomServer()
    server.set(savedLayout(), 4, 'PUBLIC')
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(drawn.at(-1)).toEqual(savedLayout())
    expect(screen.getByRole('heading', { name: 'Your room' })).toBeTruthy()
    // The revision the server returned is the one the editor will save against.
    expect(screen.getByText(/Room revision 4/)).toBeTruthy()
  })

  it('a_conflict_never_silently_discards_the_users_layout', async () => {
    const server = createRoomServer()
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    // The user edits the wall.
    fireEvent.click(screen.getByLabelText('Brick'))
    await waitFor(() => expect(drawn.at(-1)).toMatchObject({ wall: 'brick' }))
    // Someone else saves the room first.
    server.set(savedLayout(), 2, 'PUBLIC')
    server.mode('conflict')
    fireEvent.click(screen.getByRole('button', { name: 'Save room' }))
    // The conflict is shown, and the user's layout is still the draft.
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('This room changed while you were editing'))
    expect(drawn.at(-1)).toMatchObject({ wall: 'brick' })
    expect(screen.getByRole('status').textContent).toContain('Your layout is kept')
    // The resolution is explicit: keeping the draft rebases it on the newer room.
    fireEvent.click(screen.getByRole('button', { name: 'Keep my layout' }))
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull())
    expect(drawn.at(-1)).toMatchObject({ wall: 'brick' })
    // The next save now succeeds against the newer revision.
    server.mode('ok')
    fireEvent.click(screen.getByRole('button', { name: 'Save room' }))
    await waitFor(() => expect(server.read().layout.wall).toBe('brick'))
    expect(server.read().revision).toBe(3)
  })

  it('using the saved room replaces the draft only when the user chooses it', async () => {
    const server = createRoomServer()
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    fireEvent.click(screen.getByLabelText('Panel'))
    server.set(savedLayout(), 2, 'PUBLIC')
    server.mode('conflict')
    fireEvent.click(screen.getByRole('button', { name: 'Save room' }))
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Use the saved room' }))
    await waitFor(() => expect(drawn.at(-1)).toEqual(savedLayout()))
    expect(server.read().layout.wall).toBe('brick')
  })

  it('every_placement_is_reachable_by_keyboard', async () => {
    const server = createRoomServer()
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    const before = drawn.at(-1) as RoomLayout

    // Wall and floor are ordinary radio groups.
    fireEvent.click(screen.getByLabelText('Wallpaper'))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).wall).toBe('wallpaper'))
    fireEvent.click(screen.getByLabelText('Rug'))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).floor).toBe('rug'))

    // Furniture is added with a button and moved with the arrow buttons.
    fireEvent.click(screen.getByRole('button', { name: 'Add desk' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).furniture.length).toBe(before.furniture.length + 1))
    const desk = (drawn.at(-1) as RoomLayout).furniture.at(-1)!
    fireEvent.click(within(screen.getByRole('group', { name: 'Move Desk' })).getByRole('button', { name: 'Right' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).furniture.at(-1)!.cell_x).toBe(desk.cell_x + 1))
    fireEvent.click(screen.getByRole('button', { name: 'Bring Desk forward a layer' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).furniture.at(-1)!.layer).toBe(1))
    fireEvent.click(screen.getByRole('button', { name: 'Remove Desk' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).furniture.length).toBe(before.furniture.length))

    // The character moves and turns with buttons.
    fireEvent.click(screen.getByRole('button', { name: 'Face left' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).character.facing).toBe('LEFT'))
    const character = (drawn.at(-1) as RoomLayout).character
    fireEvent.click(within(screen.getByRole('group', { name: 'Move character' })).getByRole('button', { name: 'Right' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).character.cell_x).toBe(character.cell_x + 1))

    // A selected item also moves with the arrow keys.
    fireEvent.click(screen.getByRole('button', { name: /Plant at/ }))
    const plant = (drawn.at(-1) as RoomLayout).furniture[0]!
    fireEvent.keyDown(screen.getByRole('button', { name: /Plant at/ }), { key: 'ArrowUp' })
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).furniture[0]!.cell_y).toBe(plant.cell_y - 1))
  })

  it('posters are placed in a free slot and can be moved and removed by keyboard', async () => {
    const server = createRoomServer()
    server.set({ ...initialLayout(), posters: [{ poster_media_id: POSTER_MEDIA, slot: 0, x: 3, y: 2, width: 8, height: 8 }] }, 2, 'PUBLIC')
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(screen.getByText('Posters (1 of 4)')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: /Poster in slot 1 at/ }))
    fireEvent.keyDown(screen.getByRole('button', { name: /Poster in slot 1 at/ }), { key: 'ArrowRight' })
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).posters[0]!.x).toBe(4))
    fireEvent.click(screen.getByRole('button', { name: 'Remove poster in slot 1' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).posters).toEqual([]))
  })

  it('the draft never exceeds the contract bounds', async () => {
    const server = createRoomServer()
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    // Adding 32 items is allowed; the 33rd is refused with a message.
    for (let index = 0; index < 32; index++) fireEvent.click(screen.getByRole('button', { name: 'Add chair' }))
    await waitFor(() => expect((drawn.at(-1) as RoomLayout).furniture.length).toBe(32))
    expect((screen.getByRole('button', { name: 'Add chair' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('a failed save keeps the layout and says so', async () => {
    const server = createRoomServer()
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    fireEvent.click(screen.getByLabelText('Brick'))
    server.mode('failed')
    fireEvent.click(screen.getByRole('button', { name: 'Save room' }))
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('could not be saved'))
    expect((drawn.at(-1) as RoomLayout).wall).toBe('brick')
  })
})

describe('room visibility', () => {
  it('toggles visibility through the frozen operation and never touches the layout', async () => {
    const server = createRoomServer()
    server.set(savedLayout(), 4, 'PUBLIC')
    renderRoom(server)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    const layoutBefore = server.read().layout
    fireEvent.click(screen.getByLabelText('Private'))
    await waitFor(() => expect(server.read().visibility).toBe('PRIVATE'), { timeout: 2000 })
    // The visibility request carries the revision and the visibility only.
    const patch = server.calls.find(call => call.method === 'PATCH')!
    expect(patch.path).toBe('/api/me/room/visibility')
    expect(patch.body).toEqual({ expected_revision: 4, visibility: 'PRIVATE' })
    // The layout is unchanged.
    expect(server.read().layout).toEqual(layoutBefore)
    await waitFor(() => expect(screen.getAllByRole('status').map(node => node.textContent).join(' ')).toContain('private'), { timeout: 2000 })
  })

  it('the editor states that room visibility is independent of the intro audience', async () => {
    renderRoom()
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(screen.getByText(/Room visibility is separate from your intro post audience/)).toBeTruthy()
  })
})

describe('visitor view', () => {
  it('a visitor sees the saved room read-only', async () => {
    const server = createRoomServer()
    server.set(savedLayout(), 3, 'PUBLIC')
    render(<MemoryRouter><RoomPage client={server.client} profileId="00000000-0000-4000-8000-0000000007a1" loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    expect(drawn.at(-1)).toEqual(savedLayout())
    expect(screen.getByText(/You are visiting this room/)).toBeTruthy()
    // No editor controls are offered to a visitor.
    expect(screen.queryByRole('button', { name: 'Save room' })).toBeNull()
    expect(screen.queryByLabelText('Private')).toBeNull()
  })

  it('a private room shows a neutral unavailable state', async () => {
    const server = createRoomServer()
    server.mode('unavailable')
    render(<MemoryRouter><RoomPage client={server.client} profileId="00000000-0000-4000-8000-0000000007a1" loadPoster={loadPoster} converter={createFixtureConverter()} /></MemoryRouter>)
    await waitFor(() => expect(screen.getByText('This room is unavailable.')).toBeTruthy())
    expect(drawn).toHaveLength(0)
  })
})

describe('scene ownership on the screen', () => {
  it('leaving the screen destroys the scene', async () => {
    const { view } = renderRoom()
    await waitFor(() => expect(scenes).toHaveLength(1))
    expect(scenes[0]!.destroyed).toBe(false)
    act(() => view.unmount())
    expect(scenes[0]!.destroyed).toBe(true)
  })

  it('the room never offers a presence affordance', async () => {
    renderRoom()
    await waitFor(() => expect(drawn.length).toBeGreaterThan(0))
    // The editor explains that the character never shows whether the owner is
    // online; that sentence is the only place the word may appear, and it is a
    // negation, not an affordance.
    const text = document.body.textContent ?? ''
    expect(text).toContain('It never shows whether you are online.')
    const withoutTheDisclaimer = text.replace('It never shows whether you are online.', '')
    for (const forbidden of [/online/i, /offline/i, /last seen/i, /active now/i, /currently connected/i]) {
      expect(withoutTheDisclaimer).not.toMatch(forbidden)
    }
    expect(within(document.body).queryByRole('img', { name: /status/i })).toBeNull()
  })
})
