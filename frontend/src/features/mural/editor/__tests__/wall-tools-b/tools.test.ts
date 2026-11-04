import { expect, it, vi } from 'vitest'
import { previewWallAction } from '../../tools'
import type { WallActionResult } from '../../tools'
import { actionAck, drain, ids, island, snapshot, time, toolContext, toolsFixture } from './fixture'

it('uses server tool availability, editable state and cooldown without inventing local permission', async () => {
  let context = toolContext({ capabilities: { pencil: false, erase: true, fill: true, undo: true, backgroundIndex: 0 },
    nextAllowedAt: '2026-11-04T00:00:05Z' })
  const f = toolsFixture({ context: () => context })
  try {
    await f.ready(); expect(f.controller.canUseTool('pencil')).toBe(false)
    f.controller.chooseTool('erase'); f.controller.select(10, 10)
    expect(f.controller.canConfirm()).toBe(false); expect(f.controller.remaining()).toBe(5000)
    context = toolContext({ editable: false }); await f.controller.refreshTools()
    expect(f.controller.canEdit()).toBe(false); expect(f.controller.canUseTool('fill')).toBe(false)
    await f.controller.confirm(); expect(f.actions).toHaveLength(0)
  } finally { f.dispose() }
})

it('previews four-connected cells without crossing diagonals or wrapping a row', () => {
  const value = snapshot({ pixels: Array.from({ length: 16384 }, () => 0) })
  value.pixels[0] = value.pixels[1] = value.pixels[128 + 2] = value.pixels[127] = value.pixels[128] = 1
  const result = previewWallAction(value, 'fill', 0, 0, 2)
  expect(result.ok && result.draft.cells).toEqual([0, 1, 128])
  const edge = previewWallAction(value, 'fill', 127, 0, 2)
  expect(edge.ok && edge.draft.cells).toEqual([127])
  expect(value.pixels[0]).toBe(1)
})

it('accepts exactly 256 changed cells and rejects all of a 257-cell fill', () => {
  const value = island(16, 16), exact = previewWallAction(value, 'fill', 10, 10, 2)
  expect(exact.ok && exact.draft.count).toBe(256)
  value.pixels[10 * 128 + 26] = 1
  expect(previewWallAction(value, 'fill', 10, 10, 2)).toEqual({ ok: false, reason: 'FILL_TOO_LARGE' })
  expect(value.pixels[10 * 128 + 10]).toBe(1)
})

it('rejects zero-change and invalid previews; erase changes exactly one opaque palette cell', () => {
  expect(previewWallAction(island(), 'fill', 10, 10, 1)).toEqual({ ok: false, reason: 'NO_CHANGE' })
  expect(previewWallAction(island(), 'fill', 128, 0, 2)).toEqual({ ok: false, reason: 'INVALID_INPUT' })
  const erase = previewWallAction(island(), 'erase', 10, 10, 0)
  expect(erase.ok && erase.draft.cells).toEqual([1290])
})

it('stages an explicit revision-pinned fill without modifying confirmed pixels', async () => {
  const f = toolsFixture()
  try {
    await f.ready()
    expect(f.controller.chooseTool('fill')).toBe(true)
    f.controller.chooseColor(2); f.controller.select(10, 10)
    expect(f.controller.read().draft?.count).toBe(16)
    expect(f.controller.read().snapshot?.pixels[1290]).toBe(1)
    expect(f.actions).toHaveLength(0)
    await f.controller.confirm()
    expect(f.actions).toHaveLength(1)
    expect(f.actions[0]?.body).toEqual({ expected_revision: 10, x: 10, y: 10, color_index: 2 })
    expect(f.controller.read().snapshot?.pixels[1290]).toBe(1)
    expect(f.controller.read().phase).toBe('confirmed')
    expect(f.controller.canEdit()).toBe(false)
  } finally { f.dispose() }
})

it('requires a qualified background role and fails closed per missing tool capability', async () => {
  const context = toolContext({ capabilities: { pencil: true, erase: true, fill: false, undo: true, backgroundIndex: null } })
  const f = toolsFixture({ context: () => context })
  try {
    await f.ready()
    expect(f.controller.canUseTool('erase')).toBe(false)
    expect(f.controller.canUseTool('fill')).toBe(false)
    expect(f.controller.chooseTool('erase')).toBe(false)
    expect(f.controller.chooseTool('pencil')).toBe(true)
  } finally { f.dispose() }
})

it('rejects wrong actor/event and unverified extra capability fields', async () => {
  for (const context of [toolContext({ actorId: ids.operation }), toolContext({ eventId: ids.operation }), { ...toolContext(), authorizeLocally: true }]) {
    const f = toolsFixture({ context: () => context })
    try { await f.ready(); expect(f.controller.read().toolStatus).toBe('unavailable'); expect(f.controller.canUseTool('fill')).toBe(false) }
    finally { f.dispose() }
  }
})

it('invalidates a staged fill after any global edit, including a disjoint cell', async () => {
  const f = toolsFixture()
  try {
    await f.ready(); f.controller.chooseTool('fill'); f.controller.chooseColor(2); f.controller.select(10, 10)
    f.context(toolContext({ revision: 11 }))
    const newer = island(); newer.revision = 11; newer.pixels[16383] = 3
    f.controller.acceptSnapshot(f.controller.scope, newer); await drain()
    expect(f.controller.read().draft).toBeNull(); expect(f.controller.read().selection).toBeNull()
    await f.controller.confirm(); expect(f.actions).toHaveLength(0)
    f.controller.chooseTool('fill'); f.controller.select(10, 10)
    expect(f.controller.read().draft?.revision).toBe(11)
  } finally { f.dispose() }
})

it('uses only the server-reported shared budget and never restores it with a local clock', async () => {
  let now = 0
  const f = toolsFixture({ snapshot: island(16, 16), now: () => now, context: () => toolContext({ budget: { limit: 512, windowMs: 600000, remaining: 255, nextCellAvailableAt: time } }) })
  try {
    await f.ready(); f.controller.chooseTool('fill'); f.controller.chooseColor(2); f.controller.select(10, 10)
    expect(f.controller.read().draft?.count).toBe(256); expect(f.controller.canConfirm()).toBe(false)
    now = 600001
    expect(f.controller.canConfirm()).toBe(false); await f.controller.confirm(); expect(f.actions).toHaveLength(0)
  } finally { f.dispose() }
})

it('previews own latest undo and charges its full changed-cell count against the shared budget', async () => {
  let now = 0
  const target = { actionId: ids.operation, revision: 10, changedCells: 16, expiresAt: '2026-11-04T00:10:00Z' }
  const f = toolsFixture({ now: () => now, context: () => toolContext({ undoTarget: target, budget: { limit: 512, windowMs: 600000, remaining: 15, nextCellAvailableAt: '2026-11-04T00:10:00Z' } }) })
  try {
    await f.ready(); expect(f.controller.previewUndo()).toBe(true)
    expect(f.controller.read().draft?.body).toEqual({ expected_revision: 10, target_action_id: ids.operation })
    expect(f.controller.canConfirm()).toBe(false)
    now = 600000
    expect(f.controller.canUseTool('undo')).toBe(false)
  } finally { f.dispose() }
})

it('keeps the exact original action identity across unknown, cancel-preview, refresh and check', async () => {
  const f = toolsFixture({ port: { submit: async () => ({ kind: 'unknown' }), check: async c => actionAck(c) } })
  try {
    await f.ready(); f.controller.chooseTool('fill'); f.controller.chooseColor(2); f.controller.select(10, 10)
    await f.controller.confirm()
    const original = f.controller.read().action!
    f.controller.cancelSelection()
    await f.controller.refreshTools()
    expect(f.controller.read().action).toBe(original); expect(f.controller.canUseTool('pencil')).toBe(false)
    await f.controller.check()
    expect(f.checks[0]).toBe(original); expect(f.actions).toHaveLength(1)
    expect(f.controller.read().phase).toBe('confirmed')
  } finally { f.dispose() }
})
it('recovers the server original only after the attempted key has a verified terminal failure', async () => {
  const other = { operation: 'fill' as const, key: ids.operation, body: { expected_revision: 9, x: 10, y: 10, color_index: 2 } }
  let failed = false
  const f = toolsFixture({ context: () => toolContext({ unresolved: failed ? other : null }), port: {
    submit: async () => ({ kind: 'unknown' }),
    check: async () => { failed = true; return { kind: 'rejected', reason: 'MURAL_ACTION_UNRESOLVED' } },
  } })
  try {
    await f.ready(); f.controller.chooseTool('erase'); f.controller.select(10, 10); await f.controller.confirm()
    const attempted = f.controller.read().action!
    await f.controller.refreshTools()
    expect(f.controller.read().action).toBe(attempted)
    await f.controller.check(); await drain()
    expect(f.controller.read().action).toMatchObject(other)
    expect(f.controller.read().action?.key).not.toBe(attempted.key)
    expect(f.controller.read().phase).toBe('unknown'); expect(f.controller.canEdit()).toBe(false)
    expect(f.actions).toHaveLength(1); expect(f.checks).toEqual([attempted])
  } finally { f.dispose() }
})

it('recovers an authenticated server-owned unresolved action on reload without replaying it', async () => {
  const original = { operation: 'fill' as const, key: ids.operation, body: { expected_revision: 9, x: 10, y: 10, color_index: 2 } }
  const f = toolsFixture({ context: () => toolContext({ unresolved: original }) })
  try {
    await f.ready()
    expect(f.controller.read().action?.key).toBe(original.key)
    expect(f.controller.read().action?.body).toEqual(original.body)
    expect(f.controller.read().phase).toBe('unknown'); expect(f.actions).toHaveLength(0)
    expect(f.controller.canEdit()).toBe(false)
    await f.controller.check(); expect(f.checks[0]?.key).toBe(original.key); expect(f.actions).toHaveLength(0)
  } finally { f.dispose() }
})

it('recovers the old strict pixel command through its incumbent read-only result path', async () => {
  const original = { operation: 'placeConcertPixel' as const, key: ids.operation, body: { expected_revision: 9, x: 10, y: 10, color_index: 2 } }
  const f = toolsFixture({ context: () => toolContext({ unresolved: original }) })
  try {
    await f.ready(); expect(f.controller.read().command?.body).toEqual(original.body)
    await f.controller.check(); expect(f.pixelChecks[0]?.key).toBe(original.key)
    expect(f.pixels).toHaveLength(0); expect(f.actions).toHaveLength(0)
  } finally { f.dispose() }
})

it('does not replace an incumbent unknown command with another server lookup identity', async () => {
  const f = toolsFixture({ port: { submit: async () => ({ kind: 'unknown' }) } })
  try {
    await f.ready(); f.controller.chooseTool('erase'); f.controller.select(10, 10); await f.controller.confirm()
    const original = f.controller.read().action
    f.context(toolContext({ unresolved: { operation: 'erase', key: ids.snapshot, body: { expected_revision: 10, x: 11, y: 11 } } }))
    await f.controller.refreshTools()
    expect(f.controller.read().toolStatus).toBe('unavailable'); expect(f.controller.read().action).toBe(original)
  } finally { f.dispose() }
})

it('treats malformed, foreign and mismatched receipts as unknown, never as optimistic success', async () => {
  for (const patch of [{ actorId: ids.operation }, { eventId: ids.operation }, { commandKey: ids.snapshot }, { revision: 20 }, { resetRequired: false }]) {
    const f = toolsFixture({ port: { submit: async c => actionAck(c, patch) } })
    try {
      await f.ready(); f.controller.chooseTool('fill'); f.controller.chooseColor(2); f.controller.select(10, 10); await f.controller.confirm()
      expect(f.controller.read().phase).toBe('unknown'); expect(f.controller.read().snapshot?.revision).toBe(10)
      expect(f.controller.canEdit()).toBe(false)
    } finally { f.dispose() }
  }
})

it('ignores late action success after privacy eviction and scrubs every private tool field', async () => {
  let resolve!: (result: WallActionResult) => void
  const f = toolsFixture({ port: { submit: () => new Promise(r => { resolve = r }) } })
  await f.ready(); f.controller.chooseTool('fill'); f.controller.chooseColor(2); f.controller.select(10, 10)
  const sending = f.controller.confirm(), original = f.actions[0]!
  f.session.replace(f.session.read().generation, null); await drain()
  resolve(actionAck(original)); await sending
  expect(f.controller.read().snapshot).toBeNull(); expect(f.controller.read().action).toBeNull()
  expect(f.controller.read().toolContext).toBeNull(); expect(f.signals.at(-1)?.aborted).toBe(true)
  f.dispose()
})

it('bounds a hung action and retains its original key for read-only recovery after timeout', async () => {
  vi.useFakeTimers()
  const f = toolsFixture({ timeoutMs: 20, port: { submit: () => new Promise(() => {}) } })
  try {
    await f.ready(); f.controller.chooseTool('erase'); f.controller.select(10, 10)
    const sending = f.controller.confirm()
    await vi.advanceTimersByTimeAsync(20); await sending
    expect(f.controller.read().phase).toBe('unknown')
    expect(f.controller.read().action?.key).toBe(f.actions[0]?.key)
    expect(f.controller.canEdit()).toBe(false)
    expect(f.signals.at(-1)?.aborted).toBe(true)
  } finally { f.dispose(); vi.useRealTimers() }
})
