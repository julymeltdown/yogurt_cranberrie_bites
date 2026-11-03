import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createMuralEditor } from '../controller'
import type { CommandResult, MuralCommandPort, MuralSnapshot, PixelCommand } from '../contract'

export const ids = { user: '00000000-0000-4000-8000-000000000001', event: '00000000-0000-4000-8000-000000000002',
  snapshot: '00000000-0000-4000-8000-000000000003', operation: '00000000-0000-4000-8000-000000000004' }
export const time = '2026-11-04T00:00:00Z'
export const palette = ['#FAF6ED', '#201D1D', '#A22635', '#605952', '#E5DFD4', '#713B70', '#244C69', '#42736B',
  '#7E8245', '#BD8E46', '#CF6845', '#D4A2AC', '#938EB0', '#8EBDD0', '#ACCB96', '#FFFFFF']
export function snapshot(patch: Partial<MuralSnapshot> = {}): MuralSnapshot {
  return { snapshot_id: ids.snapshot, event_id: ids.event, revision: 10, state: 'EDITABLE', width: 128, height: 128,
    palette: [...palette], pixels: Array.from({ length: 16384 }, (_, i) => {
      const x = i % 128, y = Math.floor(i / 128)
      return x >= 32 && x < 96 && y >= 32 && y < 96 ? (Math.floor(x / 8) + Math.floor(y / 8)) % 3 : 0
    }), created_at: time, server_time: time, cooldown_ms: 5000, next_allowed_at: null, ...patch }
}
export function ack(command: PixelCommand, patch: Record<string, unknown> = {}): CommandResult {
  return { kind: 'acknowledged', pixel: { operation_id: ids.operation, event_id: command.scope.eventId,
    revision: command.body.expected_revision + 1, x: command.body.x, y: command.body.y, color_index: command.body.color_index,
    committed_at: time, next_allowed_at: '2026-11-04T00:00:05Z', ...patch } }
}
export function fixture(port?: Partial<MuralCommandPort>, now: () => number = () => performance.now(), timeoutMs?: number) {
  const session = createMemorySession(), client = new QueryClient()
  session.replace(0, { userId: ids.user, eventId: ids.event, accessToken: 'fixture-memory-only' })
  const privacy = createQueryPrivacy({ client, session }), places: PixelCommand[] = [], checks: PixelCommand[] = [], signals: AbortSignal[] = []
  let key = 10
  const controller = createMuralEditor({ privacy, now, ...(timeoutMs === undefined ? {} : { timeoutMs }), newKey: () => `00000000-0000-4000-8000-${String(++key).padStart(12, '0')}`,
    commands: { place: async (c, s) => { places.push(c); signals.push(s); return port?.place ? port.place(c, s) : ack(c) },
      check: async (c, s) => { checks.push(c); signals.push(s); return port?.check ? port.check(c, s) : ack(c) } } })
  controller.acceptSnapshot(controller.scope, snapshot())
  return { controller, privacy, session, places, checks, signals,
    dispose() { controller.dispose(); privacy.dispose(); client.clear() } }
}
