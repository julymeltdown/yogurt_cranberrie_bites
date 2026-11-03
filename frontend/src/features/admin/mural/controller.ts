import type { AdminCommand, MuralAdminState, MuralIntent, OperatorContextPort, PrivacyPort } from './contract'
import { controlSchema, lockSchema, restoreSchema, uuid } from './contract'
import type { MuralAdminApi } from './http'
import { createAdminOwner } from './owner'

export interface MuralControlState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error' | 'unavailable' | 'disposed'
  readonly mural: MuralAdminState | null
  readonly draft: MuralIntent | null
  readonly command: AdminCommand | null
  readonly commandPhase: 'idle' | 'sending' | 'checking' | 'pending' | 'unknown' | 'confirmed' | 'rejected'
  readonly message: string
  readonly version: number
}
const empty = (): MuralControlState => ({ phase: 'idle', mural: null, draft: null, command: null, commandPhase: 'idle', message: 'Load the authorized wall controls.', version: 0 })
export function createMuralControl({ privacy, context, api, newKey = () => crypto.randomUUID() }: {
  privacy: PrivacyPort; context: OperatorContextPort; api: MuralAdminApi; newKey?: () => string
}) {
  let state = Object.freeze(empty()), disposed = false, floor = 1, frozen = false, latest: MuralAdminState | null = null
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<MuralControlState>) => { state = Object.freeze({ ...state, ...patch, version: state.version + 1 }); for (const f of Array.from(listeners)) { try { f() } catch { /* Independent views. */ } } }
  const owner = createAdminOwner(privacy, context, security => {
    if (security) { floor = 1; frozen = false; latest = null }
    publish({ phase: 'unavailable', mural: null, draft: null, ...(security ? { command: null, commandPhase: 'idle' as const } : state.command ? { commandPhase: 'unknown' as const } : {}),
      message: 'Operator context changed. Reload authorized wall state; previous previews are invalid.' })
  })
  function accept(data: MuralAdminState) {
    if (data.revision < floor || frozen && data.state !== 'FROZEN' || latest && data.revision === floor && JSON.stringify(data) !== JSON.stringify(latest)) throw new Error('Stale mural state')
    floor = data.revision; frozen ||= data.state === 'FROZEN'; latest = data
    publish({ mural: data, phase: 'ready', draft: null })
  }
  async function load() {
    if (disposed) return
    const lease = owner.begin(); if (!lease) { if (!owner.capture()) publish({ phase: 'unavailable', mural: null, draft: null }); return }
    publish({ phase: 'loading', draft: null, mural: null })
    try { const data = await api.read(lease.context, lease.signal); if (owner.valid(lease)) { accept(data); publish({ message: state.command ? 'Original command remains unresolved. Check its status before another action.' : 'Server wall state loaded.' }) } }
    catch { if (owner.valid(lease)) publish({ phase: 'error', mural: null, message: 'Wall controls unavailable. Retry the authorized read.' }) }
    finally { owner.finish(lease) }
  }
  function prepare(intent: MuralIntent) {
    const current = owner.capture(), wall = state.mural
    if (!current || disposed || state.command || !wall || state.phase !== 'ready' || wall.state === 'FROZEN' || intent.body.expected_revision !== wall.revision || wall.revision === Number.MAX_SAFE_INTEGER) return false
    const parsed = intent.operation === 'controlConcertMural' ? controlSchema.safeParse(intent.body) : intent.operation === 'lockConcertMuralRegion' ? lockSchema.safeParse(intent.body) : restoreSchema.safeParse(intent.body)
    if (!parsed.success || intent.operation === 'restoreConcertMural' && wall.state !== 'PAUSED'
      || intent.operation === 'controlConcertMural' && (intent.body.action === 'PAUSE' && wall.state !== 'EDITABLE' || intent.body.action === 'RESUME' && wall.state !== 'PAUSED')) return false
    publish({ draft: Object.freeze({ ...intent, body: Object.freeze({ ...parsed.data }) }) as MuralIntent, message: 'Review the exact action before confirming.' }); return true
  }
  async function confirm() {
    const draft = state.draft
    if (!draft || state.command || disposed) return
    const lease = owner.begin(); if (!lease) return
    const key = newKey()
    if (!uuid.safeParse(key).success) { owner.finish(lease); throw new Error('Invalid command key') }
    const command = Object.freeze({ ...draft, key, scope: lease.context.scope, roleRevision: lease.context.roleRevision })
    publish({ command, commandPhase: 'sending', draft: null, message: 'Waiting for the server. Do not repeat this action.' })
    try {
      const result = await api.send(lease.context, lease.signal, command)
      if (!owner.valid(lease)) return
      if (result.kind === 'confirmed') { accept(result.data); publish({ command: null, commandPhase: 'confirmed', message: 'Server confirmed the wall action. Clients must resynchronize.' }) }
      else if (result.kind === 'rejected') publish({ command: null, commandPhase: 'rejected', mural: null, phase: 'error', message: `Action rejected (${result.reason}). Reload before preparing another action.` })
      else publish({ commandPhase: result.kind, message: 'Original wall action unresolved. Check original command; no new wall action is allowed.' })
    } catch { if (owner.valid(lease)) publish({ commandPhase: 'unknown', message: 'Outcome unknown. Check original command; do not repeat the action.' }) }
    finally { owner.finish(lease) }
  }
  async function check() {
    const command = state.command; if (!command || disposed) return
    const lease = owner.begin(); if (!lease) return
    publish({ commandPhase: 'checking' })
    try {
      const receipt = await api.status(lease.context, lease.signal, command)
      if (!owner.valid(lease)) return
      if (receipt.state === 'SUCCEEDED') {
        const data = await api.read(lease.context, lease.signal)
        if (!owner.valid(lease)) return
        if (data.revision < receipt.resource_revision!) throw new Error('Result reread behind receipt')
        accept(data); publish({ command: null, commandPhase: 'confirmed', message: 'Original action succeeded. Current authorized wall state reloaded.' })
      } else if (receipt.state === 'FAILED') publish({ command: null, commandPhase: 'rejected', mural: null, phase: 'error', message: `Original action failed (${receipt.failure_reason}). Reload current state.` })
      else publish({ commandPhase: receipt.state === 'PENDING' ? 'pending' : 'unknown', message: 'Original action is pending or unavailable. No replay is permitted.' })
    } catch { if (owner.valid(lease)) publish({ commandPhase: 'unknown', message: 'Original action could not be verified. No replay is permitted.' }) }
    finally { owner.finish(lease) }
  }
  return { read: () => state, subscribe(f: () => void) { listeners.add(f); return () => { listeners.delete(f) } },
    start() { owner.start(); return load() }, load, prepare, confirm, check,
    cancel() { publish({ draft: null }) },
    dispose() { if (!disposed) { disposed = true; latest = null; floor = 1; frozen = false; owner.dispose(); publish({ ...empty(), phase: 'disposed' }); listeners.clear() } },
  }
}
export type MuralControl = ReturnType<typeof createMuralControl>
