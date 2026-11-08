// @vitest-environment jsdom
/**
 * WP-07 T-610: the profile's Room entry.
 *
 * The acceptance cases of the task are the first two tests:
 *
 * * `no_intro_post_component_renders_a_room_preview` — structural: no intro post
 *   component imports from `features/profile-room`, and no intro post component
 *   renders a room;
 * * `the_room_opens_only_from_the_profile_entry` — the entry links to the room
 *   route, and the room screen is a subordinate screen, not a panel of the profile.
 */
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { RoomEntry, roomPath } from '../RoomEntry'

const ROOT = resolve(process.cwd(), 'src')

function files(directory: string, predicate: (name: string) => boolean, found: string[] = []): string[] {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry)
    if (statSync(path).isDirectory()) files(path, predicate, found)
    else if (predicate(path)) found.push(path)
  }
  return found
}

const source = (path: string) => readFileSync(path, 'utf8')
/** Source with comments removed: prose may name what the code deliberately does not do. */
const codeOnly = (text: string) => text.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
  .filter(line => !line.trimStart().startsWith('//') && !line.trimStart().startsWith('*')).join('\n')

afterEach(cleanup)

describe('room entry', () => {
  it('opens the room as a subordinate screen', () => {
    render(<MemoryRouter><RoomEntry profileId="00000000-0000-4000-8000-0000000007a1" available /></MemoryRouter>)
    const link = screen.getByRole('link', { name: 'Open room' })
    expect(link.getAttribute('href')).toBe(roomPath('00000000-0000-4000-8000-0000000007a1'))
    expect(roomPath('00000000-0000-4000-8000-0000000007a1')).toBe('/people/00000000-0000-4000-8000-0000000007a1/room')
    // The entry is a link, not a preview: no canvas, no image, no scene host.
    expect(document.querySelector('canvas')).toBeNull()
    expect(document.querySelector('img')).toBeNull()
    expect(screen.queryByLabelText('Room scene')).toBeNull()
  })

  it('a room that is not available is not offered as a link', () => {
    render(<MemoryRouter><RoomEntry profileId="00000000-0000-4000-8000-0000000007a1" available={false} /></MemoryRouter>)
    expect(screen.queryByRole('link', { name: 'Open room' })).toBeNull()
    expect(screen.getByRole('status').textContent).toBe('Unavailable')
    expect(screen.getByText('This room is not available.')).toBeTruthy()
  })

  it('the owner sees their own wording and a visitor sees the visiting wording', () => {
    render(<MemoryRouter><RoomEntry profileId="00000000-0000-4000-8000-0000000007a1" available isOwner /></MemoryRouter>)
    expect(screen.getByText(/Your personal room/)).toBeTruthy()
    cleanup()
    render(<MemoryRouter><RoomEntry profileId="00000000-0000-4000-8000-0000000007a1" available /></MemoryRouter>)
    expect(screen.getByText(/Open this person's personal room/)).toBeTruthy()
  })

  it('the room is never embedded in the profile header', () => {
    // The profile's own header component renders no room entry: the entry is a
    // separate section the profile composes, never part of the header markup.
    const header = source(join(ROOT, 'features/profile/PublicProfile.tsx'))
    expect(codeOnly(header)).not.toContain('RoomEntry')
    expect(codeOnly(header)).not.toContain('profile-room')
    const entry = codeOnly(source(join(ROOT, 'features/profile/RoomEntry.tsx')))
    expect(entry).not.toContain('canvas')
    expect(entry).not.toContain('RoomPage')
  })
})

describe('intro posts never render a room', () => {
  it('no_intro_post_component_renders_a_room_preview', () => {
    // Every intro post component: the intro feature and the social composition
    // that renders intro-shaped posts.
    const introFiles = files(join(ROOT, 'features/intro'), name => /\.(ts|tsx)$/.test(name))
    expect(introFiles.length).toBeGreaterThan(0)
    for (const path of introFiles) {
      const code = codeOnly(source(path))
      expect(code, `${path} must not import features/profile-room`).not.toMatch(/from ['"][^'"]*profile-room/)
      expect(code, `${path} must not render a room`).not.toMatch(/RoomPage|RoomEntry|RoomScene|createRoomScene/)
    }
    // The feed and thread components that render posts are checked the same way:
    // a room preview inside a post would be a room embedded in an intro post.
    const postFiles = [
      ...files(join(ROOT, 'features/event-feed'), name => /\.(ts|tsx)$/.test(name) && !name.includes('__tests__')),
      ...files(join(ROOT, 'features/feed-thread'), name => /\.(ts|tsx)$/.test(name) && !name.includes('__tests__')),
    ]
    expect(postFiles.length).toBeGreaterThan(0)
    for (const path of postFiles) {
      const code = codeOnly(source(path))
      expect(code, `${path} must not import features/profile-room`).not.toMatch(/from ['"][^'"]*profile-room/)
      expect(code, `${path} must not render a room`).not.toMatch(/RoomPage|RoomEntry|RoomScene|createRoomScene/)
    }
  })

  it('the room feature exports nothing an intro post could embed', () => {
    // The feature's public surface is the screen and its parts; there is no
    // "preview" or "thumbnail" export that a post could render inline.
    const index = codeOnly(source(join(ROOT, 'features/profile-room/index.ts')))
    for (const forbidden of [/RoomPreview/, /RoomThumbnail/, /RoomCard/, /RoomInline/]) {
      expect(index).not.toMatch(forbidden)
    }
    expect(index).toContain('RoomPage')
  })

  it('the room screen is a route, not a panel of the profile', () => {
    // The room opens from its own entry link; the profile panel composition has no
    // room case, so the room cannot be shown inside the profile screen.
    const panels = codeOnly(source(join(ROOT, 'app/social-composition/panels.tsx')))
    expect(panels).not.toMatch(/profile-room/)
    expect(panels).not.toMatch(/RoomPage/)
    expect(panels).not.toMatch(/RoomEntry/)
  })
})
