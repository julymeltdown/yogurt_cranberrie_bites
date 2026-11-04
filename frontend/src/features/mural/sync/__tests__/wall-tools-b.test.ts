import { expect, it } from 'vitest'
import { fixture, drain, delta, ids } from './fixture'
import { actionAck, island, toolContext } from '../../editor/__tests__/wall-tools-b/fixture'
import { previewWallAction } from '../../editor/tools'
import type { WallActionCommand, WallActionResult, WallToolPort } from '../../editor/tools'

it('projects a multi-cell action only through a fresh authoritative snapshot and one incumbent world sink', async () => {
  let server = island(), writes = 0
  const tools: WallToolPort = {
    load: async () => toolContext({ revision: server.revision }),
    submit: async command => {
      writes++
      if (command.operation !== 'fill') throw new Error('Expected fixture fill')
      const preview = previewWallAction(server, 'fill', command.body.x, command.body.y, command.body.color_index)
      if (!preview.ok) throw new Error('Fixture region invalid')
      const pixels = [...server.pixels]
      for (const index of preview.draft.cells) pixels[index] = command.body.color_index
      server = { ...server, pixels, revision: server.revision + 1 }
      return actionAck(command)
    },
    check: async () => ({ kind: 'unknown' }),
  }
  const f = fixture({ reads: { snapshot: async () => structuredClone(server), delta: async () => delta(server.revision, []) }, tools })
  try {
    await f.start(); await drain()
    const editor = f.sync.editor
    expect(editor.canUseTool('fill')).toBe(true)
    editor.chooseTool('fill'); editor.chooseColor(2); editor.select(10, 10)
    expect(f.sync.read().revision).toBe(10)
    await editor.confirm(); await drain(); f.scene.frame()
    expect(writes).toBe(1); expect(f.sync.editor).toBe(editor)
    expect(f.sync.read().revision).toBe(11)
    expect(editor.read().snapshot?.pixels[1290]).toBe(2)
    expect(f.requests.filter(r => r.kind === 'snapshot')).toHaveLength(2)
    expect(f.requests.filter(r => r.kind === 'place')).toHaveLength(0)
    expect(f.scene.read()).toMatchObject({ textures: 1, objects: 1, listeners: 1 })
    expect(f.scene.read().rgba?.[1290 * 4 + 3]).toBe(255)
  } finally { f.dispose() }
})

it('keeps an unknown original action across authority rebind and recovers it without a new submit', async () => {
  let server = island(), posts = 0, checks = 0
  let original: WallActionCommand | null = null
  const tools: WallToolPort = {
    load: async () => toolContext({ revision: server.revision }),
    submit: async command => { posts++; original = command; return { kind: 'unknown' } },
    check: async command => {
      checks++; expect(command).toBe(original)
      const value = island(); value.revision = 11; server = value
      return actionAck(command)
    },
  }
  const f = fixture({ reads: { snapshot: async () => structuredClone(server), delta: async () => delta(server.revision, []) }, tools })
  try {
    await f.start(); const editor = f.sync.editor
    editor.chooseTool('fill'); editor.chooseColor(2); editor.select(10, 10); await editor.confirm()
    f.authority.set(null); expect(editor.read().action).toBe(original)
    f.authority.set({ binding: { eventId: ids.event, roomId: ids.room, roomEpoch: ids.next, streamId: ids.next }, writable: true }); await drain()
    expect(editor.read().action).toBe(original); expect(editor.canEdit()).toBe(false)
    await editor.check(); await drain()
    expect(posts).toBe(1); expect(checks).toBe(1); expect(editor.read().action).toBeNull()
    expect(f.sync.read().revision).toBe(11)
  } finally { f.dispose() }
})

it('fences a late old-authority action acknowledgement without resetting a newer baseline', async () => {
  let resolve!: (result: WallActionResult) => void, sent!: WallActionCommand
  let server = island()
  const f = fixture({ reads: { snapshot: async () => structuredClone(server), delta: async () => delta(server.revision, []) }, tools: {
    load: async () => toolContext({ revision: server.revision }),
    submit: command => { sent = command; return new Promise(r => { resolve = r }) }, check: async () => ({ kind: 'unknown' }),
  } })
  try {
    await f.start(); const editor = f.sync.editor
    editor.chooseTool('erase'); editor.select(10, 10); const sending = editor.confirm()
    server = { ...island(), revision: 20 }
    f.authority.set({ binding: { eventId: ids.event, roomId: ids.room, roomEpoch: ids.next, streamId: ids.next }, writable: true }); await drain()
    resolve(actionAck(sent)); await sending; await drain()
    expect(f.sync.read().revision).toBe(20); expect(editor.read().snapshot?.revision).toBe(20)
    expect(editor.read().phase).toBe('unknown'); expect(editor.read().action?.key).toBe(sent.key)
  } finally { f.dispose() }
})

it('cleans tool reads and original actions with the same privacy owner and no new world subscription', async () => {
  const f = fixture({ tools: { load: async () => toolContext(), submit: async () => ({ kind: 'pending' }), check: async () => ({ kind: 'pending' }) } })
  await f.start(); f.sync.editor.chooseTool('erase'); f.sync.editor.select(10, 10)
  // This fixture's incumbent raster differs; an erase still remains a draft only.
  f.sync.dispose()
  expect(f.sync.editor.read().toolContext).toBeNull(); expect(f.sync.editor.read().draft).toBeNull()
  expect(f.sync.editor.read().action).toBeNull(); expect(f.authority.count()).toBe(0)
  expect(f.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  f.privacy.dispose()
})
