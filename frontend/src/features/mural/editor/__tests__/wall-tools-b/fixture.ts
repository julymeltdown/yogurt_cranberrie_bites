import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../../shared/query/privacy'
import { createMuralEditor } from '../../controller'
import type { CommandResult, MuralCommandPort, MuralSnapshot, PixelCommand } from '../../contract'
import type { WallActionCommand, WallActionResult, WallToolContext, WallToolPort, WallToolTimers } from '../../tools'
import { ids, snapshot, time } from '../fixture'

export { ids, snapshot, time }
export const drain = async () => { for (let i = 0; i < 30; i++) await Promise.resolve() }
export function toolContext(patch: Partial<WallToolContext> = {}): WallToolContext {
  return { version: 1, actorId: ids.user, eventId: ids.event, revision: 10, serverTime: time, editable: true, nextAllowedAt: null,
    capabilities: { pencil: true, erase: true, fill: true, undo: true, backgroundIndex: 0 }, fillLimit: 256, undoWindowMs: 600000,
    budget: { limit: 512, windowMs: 600000, remaining: 512, nextCellAvailableAt: '2026-11-04T00:10:00Z' },
    undoTarget: null, unresolved: null, ...patch }
}
export function island(width = 4, height = 4): MuralSnapshot {
  const value = snapshot({ pixels: Array.from({ length: 16384 }, () => 0), cooldown_ms: 0 })
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) value.pixels[(y + 10) * 128 + x + 10] = 1
  return value
}
export function actionAck(command: WallActionCommand, patch: Record<string, unknown> = {}): WallActionResult {
  return { kind: 'acknowledged', receipt: { actionId: ids.operation, eventId: ids.event, actorId: ids.user, commandKey: command.key,
    operation: command.operation, revision: command.body.expected_revision + 1, changedCells: command.operation === 'erase' ? 1 : 16,
    committedAt: time, nextAllowedAt: time, resetRequired: true, ...patch } }
}
export function toolsFixture(options: { port?: Partial<WallToolPort>; commands?: Partial<MuralCommandPort>;
  context?: () => unknown; snapshot?: MuralSnapshot; now?: () => number; timeoutMs?: number; clock?: WallToolTimers & { now(): number } } = {}) {
  const session = createMemorySession(), client = new QueryClient()
  session.replace(0, { userId: ids.user, eventId: ids.event, accessToken: 'synthetic-fixture-memory-only' })
  const privacy = createQueryPrivacy({ session, client }), actions: WallActionCommand[] = [], checks: WallActionCommand[] = [],
    pixels: PixelCommand[] = [], pixelChecks: PixelCommand[] = [], signals: AbortSignal[] = []
  let context: unknown = toolContext(), key = 400
  const controller = createMuralEditor({ privacy, now: options.clock?.now ?? options.now ?? (() => 0), timeoutMs: options.timeoutMs ?? 15000,
    ...(options.clock ? { toolTimers: options.clock } : {}),
    newKey: () => `00000000-0000-4000-8000-${String(++key).padStart(12, '0')}`,
    commands: { place: async (c, s): Promise<CommandResult> => { pixels.push(c); return options.commands?.place?.(c, s) ?? { kind: 'unknown' } },
      check: async (c, s): Promise<CommandResult> => { pixelChecks.push(c); return options.commands?.check?.(c, s) ?? { kind: 'unknown' } } },
    tools: { load: async (scope, signal) => { signals.push(signal); return options.port?.load ? options.port.load(scope, signal) : structuredClone(options.context?.() ?? context) },
      submit: async (c, s) => { actions.push(c); signals.push(s); return options.port?.submit ? options.port.submit(c, s) : actionAck(c) },
      check: async (c, s) => { checks.push(c); signals.push(s); return options.port?.check ? options.port.check(c, s) : actionAck(c) } } })
  controller.acceptSnapshot(controller.scope, options.snapshot ?? island())
  return { controller, privacy, session, actions, checks, pixels, pixelChecks, signals,
    context(value: unknown) { context = value }, ready: drain,
    dispose() { controller.dispose(); privacy.dispose(); client.clear() } }
}
