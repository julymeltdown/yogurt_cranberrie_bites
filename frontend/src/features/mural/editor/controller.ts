import { pixelOperationSchema, placePixelSchema, snapshotSchema } from './contract'
import type { Availability, CommandResult, MuralCommandPort, MuralSnapshot, PixelCommand, PrivacyPort } from './contract'
import type { PrivacyScope } from '../../../shared/query/privacy'

export interface Selection { readonly x: number; readonly y: number; readonly color_index: number }
export interface EditorState {
  readonly snapshot: MuralSnapshot | null
  readonly selection: Selection | null
  readonly color: number
  readonly availability: Availability
  readonly phase: 'idle' | 'sending' | 'checking' | 'unknown' | 'pending' | 'confirmed' | 'rejected'
  readonly command: PixelCommand | null
  readonly message: string
  readonly cooldownUntil: number
}
const initial = (): EditorState => ({ snapshot: null, selection: null, color: 0, availability: 'loading', phase: 'idle',
  command: null, message: 'Loading the confirmed wall.', cooldownUntil: 0 })
const definitiveRejections = new Set(['INVALID_INPUT', 'AUTHENTICATION_REQUIRED', 'PERMISSION_DENIED', 'ORIGIN_NOT_ALLOWED',
  'BROWSER_INTENT_REQUIRED', 'RESOURCE_UNAVAILABLE', 'RATE_LIMITED', 'MURAL_COOLDOWN', 'VERSION_CONFLICT', 'INVALID_TRANSITION',
  'EVENT_READ_ONLY', 'MURAL_PAUSED', 'MURAL_FROZEN', 'MURAL_REGION_LOCKED', 'PAYLOAD_TOO_LARGE', 'INVALID_MEDIA_TYPE'])

/** Shell-owned, in-memory, single-scope controller. Recreate after any privacy change. */
export function createMuralEditor({ privacy, commands, now = () => performance.now(), newKey = () => crypto.randomUUID(), timeoutMs = 15000 }: {
  privacy: PrivacyPort; commands: MuralCommandPort; now?: () => number; newKey?: () => string; timeoutMs?: number
}) {
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 60000) throw new Error('Invalid command timeout')
  const scope = Object.freeze({ ...privacy.capture() })
  let state: EditorState = Object.freeze(initial())
  let disposed = false, invalidated = false, readOnly = false, request: AbortController | null = null
  const listeners = new Set<() => void>()
  const notify = () => {
    for (const listener of Array.from(listeners)) {
      try { listener() } catch { /* A broken consumer must not prevent other views from scrubbing. */ }
    }
  }
  const current = () => !disposed && !invalidated && scope.userId !== null && scope.eventId !== null && privacy.isCurrent(scope)
  const publish = (patch: Partial<EditorState>) => {
    state = Object.freeze({ ...state, ...patch })
    notify()
  }
  function scrub() {
    invalidated = true
    request?.abort(); request = null
    state = Object.freeze({ ...initial(), availability: 'read-only', message: 'Wall unavailable. Reopen after your session is revalidated.' })
    notify()
  }
  const unregister = privacy.registerCleanup(scrub)
  const remaining = () => Math.max(0, state.cooldownUntil - now())
  const canEdit = () => current() && state.snapshot?.state === 'EDITABLE' && state.availability === 'writable'
    && !state.command && state.snapshot.revision < Number.MAX_SAFE_INTEGER
  const canConfirm = () => canEdit() && state.selection !== null && remaining() === 0
  const sameScope = (candidate: PrivacyScope) => candidate.userId === scope.userId && candidate.eventId === scope.eventId
    && candidate.generation === scope.generation && candidate.policyGeneration === scope.policyGeneration && candidate.gate === scope.gate

  function acceptSnapshot(expected: PrivacyScope, input: unknown, options: { reset?: boolean } = {}): boolean {
    if (!current() || !sameScope(expected)) return false
    const parsed = snapshotSchema.safeParse(input)
    if (!parsed.success || parsed.data.event_id !== scope.eventId) {
      publish({ snapshot: null, selection: null, availability: 'error', message: 'The wall could not be verified. Refresh the confirmed wall.' }); return false
    }
    const snapshot = parsed.data
    if (state.snapshot && snapshot.revision < state.snapshot.revision) return false
    Object.freeze(snapshot.pixels); Object.freeze(snapshot.palette); Object.freeze(snapshot)
    publish({ snapshot, availability: readOnly ? 'read-only' : 'writable', selection: options.reset || snapshot.state !== 'EDITABLE' || readOnly ? null : state.selection,
      cooldownUntil: now() + Math.max(0, snapshot.next_allowed_at ? Date.parse(snapshot.next_allowed_at) - Date.parse(snapshot.server_time) : 0),
      message: state.command ? state.message : readOnly ? 'This wall is read-only.' : snapshot.state === 'PAUSED' ? 'The wall is paused.' : snapshot.state === 'FROZEN'
        ? 'The final wall is frozen.' : 'Select a pixel and color. Confirm when ready.' })
    return true
  }
  function result(command: PixelCommand, response: CommandResult) {
    if (response.kind === 'acknowledged') {
      const parsed = pixelOperationSchema.safeParse(response.pixel)
      const p = parsed.data, b = command.body
      if (!parsed.success || !p || p.event_id !== scope.eventId || p.revision !== b.expected_revision + 1
        || p.x !== b.x || p.y !== b.y || p.color_index !== b.color_index || Date.parse(p.next_allowed_at) < Date.parse(p.committed_at)) {
        publish({ phase: 'unknown', message: 'Outcome unknown. Check the original command; do not place another pixel.' }); return
      }
      const snapshot = state.snapshot
      // Never overwrite a newer sync snapshot or jump across a missing revision.
      if (snapshot?.revision === b.expected_revision) {
        const pixels = [...snapshot.pixels]; pixels[p.y * 128 + p.x] = p.color_index; Object.freeze(pixels)
        publish({ snapshot: Object.freeze({ ...snapshot, pixels, revision: p.revision, server_time: p.committed_at, next_allowed_at: p.next_allowed_at }) })
      } else if (!snapshot || snapshot.revision < p.revision) {
        publish({ availability: 'error' })
      }
      publish({ command: null, selection: null, phase: 'confirmed', cooldownUntil: now() + Math.max(0, Date.parse(p.next_allowed_at) - Date.parse(p.committed_at)),
        message: 'Server confirmed the pixel. The server checks every cooldown and permission.' })
    } else if (response.kind === 'rejected') {
      if (!definitiveRejections.has(response.reason)) {
        publish({ phase: 'unknown', message: 'Outcome unknown. Check the original command; do not place another pixel.' }); return
      }
      // A rejection cancels the preview. Revalidation is required for changed policy/limits.
      const reason = response.reason
      const needsRefresh = reason !== 'INVALID_INPUT'
      publish({ command: null, selection: null, phase: 'rejected', availability: needsRefresh ? 'error' : state.availability,
        message: reason === 'MURAL_COOLDOWN' ? 'Server cooldown active. Refresh the confirmed wall before trying again.'
          : reason === 'MURAL_PAUSED' ? 'The wall is paused. Refresh for its current state.'
          : reason === 'MURAL_FROZEN' ? 'The final wall is frozen. Refresh for its current state.'
          : reason === 'MURAL_REGION_LOCKED' ? 'That region is locked. Refresh the wall before selecting another pixel.'
          : 'The server rejected the pixel. No preview was committed. Refresh if needed.' })
    } else publish({ phase: response.kind, message: response.kind === 'pending' ? 'Command pending. Check the original command; no new pixel can be placed.'
      : 'Outcome unknown. Check the original command; do not place another pixel.' })
  }
  async function run(command: PixelCommand, check: boolean) {
    const active = new AbortController(); request = active
    let timedOut = false
    let timer: ReturnType<typeof setTimeout> | undefined
    let onAbort = () => undefined as void
    publish({ command, phase: check ? 'checking' : 'sending', message: check ? 'Checking the original command.' : 'Waiting for server confirmation.' })
    try {
      if (!current() || active.signal.aborted) return
      const stopped = new Promise<CommandResult>(resolve => {
        onAbort = () => resolve({ kind: 'unknown' })
        active.signal.addEventListener('abort', onAbort, { once: true })
        timer = setTimeout(() => { timedOut = true; active.abort() }, timeoutMs)
      })
      const response = await Promise.race([check ? commands.check(command, active.signal) : commands.place(command, active.signal), stopped])
      if (current() && (!active.signal.aborted || timedOut)) result(command, response)
    } catch {
      if (current() && (!active.signal.aborted || timedOut)) result(command, { kind: 'unknown' })
    } finally { clearTimeout(timer); active.signal.removeEventListener('abort', onAbort); if (request === active) request = null }
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    scope, acceptSnapshot, remaining, canEdit, canConfirm,
    setAvailability(expected: PrivacyScope, availability: Availability) {
      if (!current() || !sameScope(expected)) return
      if (availability === 'read-only') readOnly = true
      if (availability === 'writable') readOnly = false
      publish({ availability, selection: null, message: availability === 'writable' ? 'Select a pixel and color. Confirm when ready.'
        : availability === 'read-only' ? 'This wall is read-only.' : availability === 'loading' ? 'Refreshing the confirmed wall.' : 'Wall connection unavailable. Refresh before editing.' })
    },
    select(x: number, y: number) {
      if (!canEdit() || !placePixelSchema.safeParse({ x, y, color_index: state.color, expected_revision: state.snapshot!.revision }).success) return false
      publish({ selection: Object.freeze({ x, y, color_index: state.color }) }); return true
    },
    chooseColor(color: number) {
      if (!canEdit() || !Number.isInteger(color) || color < 0 || color > 15) return false
      publish({ color, selection: state.selection ? Object.freeze({ ...state.selection, color_index: color }) : null }); return true
    },
    cancelSelection() { if (current()) publish({ selection: null }) },
    async confirm() {
      if (!canConfirm()) return
      const body = Object.freeze(placePixelSchema.parse({ ...state.selection, expected_revision: state.snapshot!.revision }))
      const key = newKey()
      if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(key)) throw new Error('Invalid command key')
      await run(Object.freeze({ scope, operation: 'placeConcertPixel', key, body }), false)
    },
    async check() { if (current() && state.command && !request) await run(state.command, true) },
    dispose() {
      if (disposed) return
      scrub(); disposed = true; unregister(); listeners.clear()
    },
  }
}
export type MuralEditorController = ReturnType<typeof createMuralEditor>
