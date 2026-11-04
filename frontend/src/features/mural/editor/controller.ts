import { pixelOperationSchema, placePixelSchema, snapshotSchema } from './contract'
import type { Availability, CommandResult, MuralCommandPort, MuralSnapshot, PixelCommand, PrivacyPort } from './contract'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { matchActionReceipt, previewWallAction, toolContextSchema, toolRejections, WALL_TOOL_FRESH_MS, WALL_TOOL_REFRESH_MS } from './tools'
import type { DrawingTool, WallActionCommand, WallActionResult, WallDraft, WallToolContext, WallToolPort, WallToolTimers } from './tools'

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
  readonly tool: DrawingTool
  readonly draft: WallDraft | null
  readonly action: WallActionCommand | null
  readonly toolContext: WallToolContext | null
  readonly toolStatus: 'loading' | 'ready' | 'unavailable'
}
const initial = (): EditorState => ({ snapshot: null, selection: null, color: 0, availability: 'loading', phase: 'idle',
  command: null, message: 'Loading the confirmed wall.', cooldownUntil: 0, tool: 'pencil', draft: null, action: null, toolContext: null, toolStatus: 'unavailable' })
const definitiveRejections = new Set(['INVALID_INPUT', 'AUTHENTICATION_REQUIRED', 'PERMISSION_DENIED', 'ORIGIN_NOT_ALLOWED',
  'BROWSER_INTENT_REQUIRED', 'RESOURCE_UNAVAILABLE', 'RATE_LIMITED', 'MURAL_COOLDOWN', 'VERSION_CONFLICT', 'INVALID_TRANSITION',
  'EVENT_READ_ONLY', 'MURAL_PAUSED', 'MURAL_FROZEN', 'MURAL_REGION_LOCKED', 'PAYLOAD_TOO_LARGE', 'INVALID_MEDIA_TYPE'])

/** Shell-owned, in-memory, single-scope controller. Recreate after any privacy change. */
export function createMuralEditor({ privacy, commands, tools, now = () => performance.now(), newKey = () => crypto.randomUUID(), timeoutMs = 15000,
  toolTimers = { set: (ms, run) => setTimeout(run, ms), clear: timer => clearTimeout(timer as ReturnType<typeof setTimeout>) } }: {
  privacy: PrivacyPort; commands: MuralCommandPort; tools?: WallToolPort; now?: () => number; newKey?: () => string; timeoutMs?: number; toolTimers?: WallToolTimers
}) {
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 60000) throw new Error('Invalid command timeout')
  const scope = Object.freeze({ ...privacy.capture() })
  let state: EditorState = Object.freeze(initial())
  let disposed = false, invalidated = false, readOnly = false, request: AbortController | null = null
  let toolRead: AbortController | null = null, toolGeneration = 0, toolAnchor = 0, toolRevision: number | null = null
  let visible = true, toolDeadline = 0, toolExpiry: unknown = null, toolRefresh: unknown = null
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
  function stopToolTimers() {
    if (toolExpiry !== null) toolTimers.clear(toolExpiry)
    if (toolRefresh !== null) toolTimers.clear(toolRefresh)
    toolExpiry = toolRefresh = null; toolDeadline = 0
  }
  function withdrawTools() {
    stopToolTimers()
    publish({ toolContext: null, toolStatus: 'unavailable', draft: null, selection: null })
  }
  function scrub() {
    invalidated = true
    request?.abort(); request = null
    toolRead?.abort(); toolRead = null; ++toolGeneration
    stopToolTimers()
    state = Object.freeze({ ...initial(), availability: 'read-only', message: 'Wall unavailable. Reopen after your session is revalidated.' })
    notify()
  }
  const unregister = privacy.registerCleanup(scrub)
  const remaining = () => Math.max(0, state.cooldownUntil - now())
  const canEdit = () => current() && state.snapshot?.state === 'EDITABLE' && state.availability === 'writable'
    && !state.command && !state.action && state.toolStatus !== 'loading' && (!tools || visible && !!qualifiedTools())
    && state.toolContext?.editable !== false && state.snapshot.revision < Number.MAX_SAFE_INTEGER
  const qualifiedTools = () => state.toolStatus === 'ready' && now() < toolDeadline && state.toolContext?.revision === state.snapshot?.revision ? state.toolContext : null
  const serverNow = () => state.toolContext ? Date.parse(state.toolContext.serverTime) + Math.max(0, now() - toolAnchor) : Number.POSITIVE_INFINITY
  const canUseTool = (tool: DrawingTool | 'undo') => {
    if (!canEdit()) return false
    if (tool === 'pencil') return qualifiedTools()?.capabilities.pencil !== false
    const context = qualifiedTools()
    if (!context || !tools || !context.capabilities[tool]) return false
    if (tool === 'erase') return context.capabilities.backgroundIndex === 0
    if (tool === 'undo') return !!context.undoTarget && context.undoTarget.revision === state.snapshot?.revision
      && Date.parse(context.undoTarget.expiresAt) > serverNow()
    return true
  }
  const canConfirm = () => {
    if (!canEdit() || remaining() > 0) return false
    const context = qualifiedTools()
    if (state.draft) return canUseTool(state.draft.operation) && state.draft.revision === state.snapshot?.revision
      && !!context && context.budget.remaining >= state.draft.count
      && (state.draft.operation !== 'undo' || 'target_action_id' in state.draft.body && state.draft.body.target_action_id === context.undoTarget?.actionId)
    return state.tool === 'pencil' && canUseTool('pencil') && state.selection !== null && (!context || context.budget.remaining >= 1)
  }
  const sameScope = (candidate: PrivacyScope) => candidate.userId === scope.userId && candidate.eventId === scope.eventId
    && candidate.generation === scope.generation && candidate.policyGeneration === scope.policyGeneration && candidate.gate === scope.gate

  function acceptSnapshot(expected: PrivacyScope, input: unknown, options: { reset?: boolean } = {}): boolean {
    if (!current() || !sameScope(expected)) return false
    const parsed = snapshotSchema.safeParse(input)
    if (!parsed.success || parsed.data.event_id !== scope.eventId) {
      toolRead?.abort(); toolRead = null; ++toolGeneration; toolRevision = null
      stopToolTimers()
      publish({ snapshot: null, selection: null, draft: null, toolContext: null, toolStatus: 'unavailable', availability: 'error',
        message: 'The wall could not be verified. Refresh the confirmed wall.' }); return false
    }
    const snapshot = parsed.data
    if (state.snapshot && snapshot.revision < state.snapshot.revision) return false
    const changed = !!state.snapshot && snapshot.revision !== state.snapshot.revision
    if (changed || options.reset) stopToolTimers()
    const invalidateDraft = !!state.draft && (changed || options.reset || snapshot.state !== 'EDITABLE' || readOnly)
    Object.freeze(snapshot.pixels); Object.freeze(snapshot.palette); Object.freeze(snapshot)
    publish({ snapshot, availability: readOnly ? 'read-only' : 'writable', selection: options.reset || snapshot.state !== 'EDITABLE' || readOnly || invalidateDraft ? null : state.selection,
      draft: invalidateDraft ? null : state.draft,
      ...(changed || options.reset ? { toolContext: null, toolStatus: 'unavailable' as const } : {}),
      cooldownUntil: now() + Math.max(0, snapshot.next_allowed_at ? Date.parse(snapshot.next_allowed_at) - Date.parse(snapshot.server_time) : 0),
      message: state.command || state.action ? state.message : invalidateDraft ? 'The wall changed. Preview the action again before confirming.' : readOnly ? 'This wall is read-only.' : snapshot.state === 'PAUSED' ? 'The wall is paused.' : snapshot.state === 'FROZEN'
        ? 'The final wall is frozen.' : 'Select a pixel and color. Confirm when ready.' })
    if (tools && (toolRevision !== snapshot.revision || options.reset)) void refreshTools()
    return true
  }

  async function refreshTools(): Promise<boolean> {
    if (!tools || !current() || !visible || !state.snapshot) return false
    if (toolRefresh !== null) toolTimers.clear(toolRefresh)
    toolRefresh = null
    if (toolRead) { toolRead.abort(); withdrawTools() }
    const owner = new AbortController(), generation = ++toolGeneration, revision = state.snapshot.revision, started = now()
    toolRead = owner; toolRevision = revision
    if (!qualifiedTools()) publish({ toolStatus: 'loading', toolContext: null, draft: null, selection: state.tool === 'pencil' ? state.selection : null })
    let timer: unknown = null
    let onAbort = () => {}
    try {
      const stopped = new Promise<never>((_, reject) => {
        onAbort = () => reject(new Error('Tool context stopped'))
        owner.signal.addEventListener('abort', onAbort, { once: true })
        timer = toolTimers.set(Math.min(timeoutMs, WALL_TOOL_FRESH_MS), () => owner.abort())
      })
      const input = await Promise.race([tools.load(scope, owner.signal), stopped])
      if (!current() || !visible || owner.signal.aborted || generation !== toolGeneration || state.snapshot?.revision !== revision) return false
      if (now() >= started + WALL_TOOL_FRESH_MS) throw new Error('Expired point-in-time tool state')
      const parsed = toolContextSchema.safeParse(input), context = parsed.data
      if (!parsed.success || !context || context.actorId !== scope.userId || context.eventId !== scope.eventId || context.revision !== revision
        || context.undoTarget && Date.parse(context.undoTarget.expiresAt) - Date.parse(context.serverTime) > 600000) throw new Error('Unqualified tool state')
      const original = context.unresolved
      // A late lookup may never replace an existing original command identity.
      const incumbent = state.command ?? state.action
      if (original && incumbent && (original.key !== incumbent.key || original.operation !== incumbent.operation
        || JSON.stringify(original.body) !== JSON.stringify(incumbent.body))) throw new Error('Conflicting original command identity')
      toolAnchor = now()
      toolDeadline = started + WALL_TOOL_FRESH_MS
      const deadline = toolDeadline
      if (toolExpiry !== null) toolTimers.clear(toolExpiry)
      toolExpiry = toolTimers.set(toolDeadline - now(), () => {
        toolExpiry = null
        if (current() && toolDeadline === deadline && now() >= deadline) withdrawTools()
      })
      if (original) Object.freeze(original.body)
      const frozen = Object.freeze({ ...context, capabilities: Object.freeze(context.capabilities), budget: Object.freeze(context.budget),
        undoTarget: context.undoTarget ? Object.freeze(context.undoTarget) : null, unresolved: original ? Object.freeze(original) : null })
      publish({ toolContext: frozen, toolStatus: 'ready',
        cooldownUntil: Math.max(state.cooldownUntil, now() + Math.max(0, context.nextAllowedAt ? Date.parse(context.nextAllowedAt) - Date.parse(context.serverTime) : 0)), ...(!incumbent && original ? {
        ...(original.operation === 'placeConcertPixel' ? { command: Object.freeze({ ...original, scope, body: Object.freeze(original.body) }) }
          : { action: Object.freeze({ ...original, scope }) }),
        phase: 'unknown' as const, draft: null, selection: null,
        message: 'An original command was recovered from the server. Check its result before editing.',
      } : {}) })
      return true
    } catch {
      if (current() && generation === toolGeneration) withdrawTools()
      return false
    } finally {
      if (timer !== null) toolTimers.clear(timer)
      owner.signal.removeEventListener('abort', onAbort)
      if (toolRead === owner) toolRead = null
      if (current() && visible && generation === toolGeneration && state.snapshot) toolRefresh = toolTimers.set(Math.max(0, started + WALL_TOOL_REFRESH_MS - now()), () => {
        toolRefresh = null; void refreshTools()
      })
    }
  }

  function preview(x: number, y: number): boolean {
    if (!canUseTool(state.tool)) return false
    if (state.tool === 'pencil') {
      if (!placePixelSchema.safeParse({ x, y, color_index: state.color, expected_revision: state.snapshot!.revision }).success) return false
      publish({ selection: Object.freeze({ x, y, color_index: state.color }), draft: null }); return true
    }
    const color = state.tool === 'erase' ? 0 : state.color
    const result = previewWallAction(state.snapshot!, state.tool, x, y, color)
    if (!result.ok) {
      publish({ selection: null, draft: null, message: result.reason === 'FILL_TOO_LARGE' ? 'That connected region exceeds 256 changed cells. Nothing will be sent.'
        : result.reason === 'NO_CHANGE' ? 'This action would change no cells. Choose another pixel or color.' : 'Choose coordinates from 0 to 127.' })
      return false
    }
    publish({ selection: Object.freeze({ x, y, color_index: color }), draft: result.draft,
      message: `Preview only: ${result.draft.count} changed ${result.draft.count === 1 ? 'cell' : 'cells'}. Confirm to ask the server.` })
    return true
  }

  function actionResult(command: WallActionCommand, response: WallActionResult) {
    if (response.kind === 'acknowledged') {
      const receipt = matchActionReceipt(response.receipt, command)
      if (!receipt) { publish({ phase: 'unknown', message: 'Outcome unknown. Check the original action before editing.' }); return }
      publish({ action: null, selection: null, draft: null, phase: 'confirmed', toolContext: null, toolStatus: 'unavailable',
        availability: state.snapshot && state.snapshot.revision >= receipt.revision ? state.availability : 'loading',
        cooldownUntil: Math.max(state.cooldownUntil, now() + Math.max(0, Date.parse(receipt.nextAllowedAt) - Date.parse(receipt.committedAt))),
        message: 'Server confirmed the action. Refreshing the authoritative wall; no local pixels were committed.' })
    } else if (response.kind === 'rejected' && toolRejections.has(response.reason)) {
      publish({ action: null, selection: null, draft: null, phase: 'rejected', availability: 'error', toolContext: null, toolStatus: 'unavailable',
        message: response.reason === 'VERSION_CONFLICT' ? 'The wall changed. Refresh and preview again.'
          : response.reason === 'MURAL_ACTION_UNRESOLVED' ? 'This attempt failed because another original action is unresolved. Recovering its server identity.'
          : response.reason === 'MURAL_BUDGET_EXCEEDED' ? 'The server reports that your shared 512-cell budget is exhausted. Refresh for its current limit.'
          : response.reason === 'MURAL_UNDO_EXPIRED' || response.reason === 'MURAL_UNDO_NOT_LATEST' ? 'That action can no longer be undone. Refresh the wall.'
          : 'The server rejected the action. Refresh before previewing again.' })
    } else publish({ phase: response.kind === 'pending' ? 'pending' : 'unknown',
      message: response.kind === 'pending' ? 'Action pending. Check the original command before editing.' : 'Outcome unknown. Check the original action before editing.' })
  }

  async function runAction(command: WallActionCommand, check: boolean) {
    if (!tools || !current() || request) return
    const owner = new AbortController(); request = owner
    let timedOut = false, timer: ReturnType<typeof setTimeout> | undefined, onAbort = () => {}
    publish({ action: command, phase: check ? 'checking' : 'sending', message: check ? 'Checking the original action.' : 'Waiting for the server to confirm the action.' })
    try {
      const stopped = new Promise<WallActionResult>(resolve => {
        onAbort = () => resolve({ kind: 'unknown' })
        owner.signal.addEventListener('abort', onAbort, { once: true })
        timer = setTimeout(() => { timedOut = true; owner.abort() }, timeoutMs)
      })
      const response = await Promise.race([check ? tools.check(command, owner.signal) : tools.submit(command, owner.signal), stopped])
      if (current() && (!owner.signal.aborted || timedOut)) actionResult(command, response)
    } catch { if (current() && (!owner.signal.aborted || timedOut)) actionResult(command, { kind: 'unknown' }) }
    finally {
      clearTimeout(timer); owner.signal.removeEventListener('abort', onAbort)
      if (request === owner) request = null
      if (current() && !state.action && !state.command && state.snapshot) void refreshTools()
    }
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
      // A delayed command result must not shorten a newer snapshot's cooldown.
      publish({ command: null, selection: null, phase: 'confirmed', cooldownUntil: Math.max(state.cooldownUntil, now() + Math.max(0, Date.parse(p.next_allowed_at) - Date.parse(p.committed_at))),
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
    } finally { clearTimeout(timer); active.signal.removeEventListener('abort', onAbort); if (request === active) request = null
      if (tools && current() && !state.command && !state.action && state.snapshot) void refreshTools() }
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    scope, acceptSnapshot, remaining, canEdit, canConfirm, canUseTool, refreshTools,
    setVisible(value: boolean) {
      if (!tools || !current() || value === visible) return
      visible = value
      if (!value) {
        ++toolGeneration; toolRead?.abort(); toolRead = null; withdrawTools()
        if (state.command || state.action) {
          request?.abort()
          publish({ phase: 'unknown', message: 'Outcome unknown. Check the original command after returning to the wall.' })
        }
      } else void refreshTools()
    },
    chooseTool(tool: DrawingTool) {
      if (!canUseTool(tool)) return false
      if (state.tool === tool) return true
      publish({ tool, selection: null, draft: null }); return true
    },
    previewUndo() {
      if (!canUseTool('undo')) return false
      const target = qualifiedTools()!.undoTarget!
      publish({ selection: null, draft: Object.freeze({ operation: 'undo', revision: state.snapshot!.revision,
        count: target.changedCells, cells: Object.freeze([]), body: Object.freeze({ expected_revision: state.snapshot!.revision, target_action_id: target.actionId }) }),
        message: `Preview undo of your last action: ${target.changedCells} changed cells. It uses the same cooldown and shared budget.` })
      return true
    },
    setAvailability(expected: PrivacyScope, availability: Availability) {
      if (!current() || !sameScope(expected)) return
      if (availability === 'read-only') readOnly = true
      if (availability === 'writable') readOnly = false
      publish({ availability, selection: null, draft: null, message: state.command || state.action ? state.message : availability === 'writable' ? 'Select a pixel and color. Confirm when ready.'
        : availability === 'read-only' ? 'This wall is read-only.' : availability === 'loading' ? 'Refreshing the confirmed wall.' : 'Wall connection unavailable. Refresh before editing.' })
    },
    select(x: number, y: number) {
      return preview(x, y)
    },
    chooseColor(color: number) {
      if (!canEdit() || !Number.isInteger(color) || color < 0 || color > 15) return false
      const selection = state.selection
      publish({ color, selection: selection && state.tool === 'pencil' ? Object.freeze({ ...selection, color_index: color }) : selection })
      if (selection && state.tool !== 'pencil') return preview(selection.x, selection.y)
      return true
    },
    cancelSelection() { if (current()) publish({ selection: null, draft: null }) },
    async confirm() {
      if (!canConfirm()) return
      if (state.draft) {
        const draft = state.draft, key = newKey()
        if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(key)) throw new Error('Invalid command key')
        const command = Object.freeze({ scope, operation: draft.operation, key, body: draft.body }) as WallActionCommand
        await runAction(command, false); return
      }
      const body = Object.freeze(placePixelSchema.parse({ ...state.selection, expected_revision: state.snapshot!.revision }))
      const key = newKey()
      if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(key)) throw new Error('Invalid command key')
      await run(Object.freeze({ scope, operation: 'placeConcertPixel', key, body }), false)
    },
    async check() {
      if (!current() || request) return
      if (state.command) await run(state.command, true)
      else if (state.action) await runAction(state.action, true)
    },
    dispose() {
      if (disposed) return
      scrub(); disposed = true; unregister(); listeners.clear()
    },
  }
}
export type MuralEditorController = ReturnType<typeof createMuralEditor>
