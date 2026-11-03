import type { AdminCommand, OperatorContextPort, PrivacyPort } from '../mural/contract'
import { uuid } from '../mural/contract'
import { createAdminOwner } from '../mural/owner'
import type { ClosureApi } from './http'
import { closeEventSchema, resumeClosureSchema } from './contract'
import type { ClosureDraft, ClosureJob } from './contract'

export interface ClosureState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error' | 'unavailable' | 'disposed'
  readonly job: ClosureJob | null
  readonly jobId: string | null
  readonly draft: ClosureDraft | null
  readonly command: AdminCommand | null
  readonly commandPhase: 'idle' | 'sending' | 'checking' | 'unknown' | 'pending' | 'confirmed' | 'rejected'
  readonly message: string
  readonly version: number
}
const empty = (): ClosureState => ({ phase: 'idle', job: null, jobId: null, draft: null, command: null, commandPhase: 'idle', message: 'Closing makes the event read-only and starts durable cleanup.', version: 0 })
export function createEventClosure({ privacy, context, api, newKey = () => crypto.randomUUID() }: {
  privacy: PrivacyPort; context: OperatorContextPort; api: ClosureApi; newKey?: () => string
}) {
  let state = Object.freeze(empty()), disposed = false, latest: ClosureJob | null = null, rejectedRevision = 0
  const issuedKeys = new Set<string>()
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<ClosureState>) => { state = Object.freeze({ ...state, ...patch, version: state.version + 1 }); for (const f of Array.from(listeners)) { try { f() } catch { /* Independent views. */ } } }
  const owner = createAdminOwner(privacy, context, security => {
    if (security) { latest = null; rejectedRevision = 0; issuedKeys.clear() }
    publish({ phase: 'unavailable', draft: null, job: null, ...(security ? { command: null, jobId: null, commandPhase: 'idle' as const } : state.command ? { commandPhase: 'unknown' as const } : {}),
      message: 'Operator context changed. Reauthorize and reload the exact closure job.' })
  })
  function accept(job: ClosureJob) {
    if (latest && (job.job_id !== latest.job_id || job.revision < latest.revision || job.revision === latest.revision && JSON.stringify(job) !== JSON.stringify(latest)
      || latest.state === 'COMPLETED' && job.state !== 'COMPLETED' || latest.steps.some(old => old.state === 'SUCCEEDED' && job.steps.find(s => s.name === old.name)?.state !== 'SUCCEEDED'))) throw new Error('Closure progress cannot rewind')
    latest = job
    publish({ phase: 'ready', job, jobId: job.job_id, draft: null, message: job.state === 'PARTIAL_FAILURE' ? 'Closure partially failed. Completed steps remain complete; inspect failed steps before recovery.'
      : job.state === 'COMPLETED' ? 'Server reports all closure steps complete.' : 'Closure is in progress. Refresh this job to observe durable progress.' })
  }
  async function refresh(jobId = state.jobId) {
    if (disposed) return false
    if (!uuid.safeParse(jobId).success || latest && latest.job_id !== jobId) { publish({ message: 'Use the exact authorized closure job ID. Another job cannot replace the observed job.' }); return false }
    const lease = owner.begin(); if (!lease) { if (!owner.capture()) publish({ phase: 'unavailable', job: null, message: 'Operator authority unavailable. Revalidate access before loading closure progress.' }); return false }
    publish({ phase: 'loading', draft: null, job: null })
    try { const job = await api.read(lease.context, lease.signal, jobId!); if (!owner.valid(lease)) return false; accept(job); return true }
    catch { if (owner.valid(lease)) publish({ phase: 'error', job: null, message: 'Closure progress unavailable. Retry this authorized job read; it does not repeat closure.' }); return false }
    finally { owner.finish(lease) }
  }
  function prepare(text: string) {
    const active = owner.capture()
    if (!active || disposed || state.command || state.jobId || active.eventRevision <= rejectedRevision || !['OPEN', 'LIVE'].includes(active.phase)) return false
    const draft = closeEventSchema.safeParse({ expected_revision: active.eventRevision, confirmation: 'CLOSE_EVENT', reason: text })
    if (!draft.success) return false
    publish({ draft: Object.freeze(draft.data), message: 'Confirm closure of this exact event revision. This starts read-only archival and cleanup.' }); return true
  }
  function canResume() {
    const active = owner.capture()
    return !disposed && !!active && active.phase === 'READ_ONLY' && state.phase === 'ready' && !state.command
      && state.job?.state === 'PARTIAL_FAILURE' && state.job.job_id === state.jobId && state.job.revision < Number.MAX_SAFE_INTEGER
  }
  function prepareResume(text: string) {
    if (!canResume() || !state.job) return false
    const draft = resumeClosureSchema.safeParse({ job_id: state.job.job_id, expected_revision: state.job.revision, confirmation: 'RESUME_CLOSURE', reason: text })
    if (!draft.success) return false
    publish({ draft: Object.freeze(draft.data), message: 'Review retry of this exact failed job revision. Completed steps will remain complete.' }); return true
  }
  async function confirm() {
    const draft = state.draft
    if (!draft || state.command || disposed) return
    const lease = owner.begin(); if (!lease) return
    const resume = draft.confirmation === 'RESUME_CLOSURE'
    if (resume ? !canResume() || state.job?.job_id !== draft.job_id || state.job.revision !== draft.expected_revision
      : lease.context.eventRevision !== draft.expected_revision || !['OPEN', 'LIVE'].includes(lease.context.phase)) { owner.finish(lease); publish({ draft: null }); return }
    const key = newKey()
    if (!uuid.safeParse(key).success || issuedKeys.has(key)) { owner.finish(lease); publish({ message: 'A fresh command identity is unavailable. No closure action was sent.' }); return }
    issuedKeys.add(key)
    const command: AdminCommand = Object.freeze({ operation: 'closeConcertEvent', key, body: draft, scope: lease.context.scope, roleRevision: lease.context.roleRevision })
    publish({ draft: null, command, commandPhase: 'sending', message: resume ? 'Waiting for resume acceptance. Do not retry this boundary again.' : 'Waiting for closure acceptance. Do not start another job.' })
    try {
      const result = await api.send(lease.context, lease.signal, command)
      if (!owner.valid(lease)) return
      if (result.kind === 'confirmed') { accept(result.data); publish({ command: null, commandPhase: 'confirmed' }) }
      else if (result.kind === 'rejected') {
        if (!resume) rejectedRevision = draft.expected_revision
        publish({ phase: 'error', job: null, command: null, commandPhase: 'rejected', message: `Closure ${resume ? 'resume' : 'start'} rejected (${result.reason}). Reload the job or event context before reviewing another action.` })
      }
      else publish({ commandPhase: result.kind, message: 'Original closure command unresolved. Check it; do not repeat closure or create a new key.' })
    } catch { if (owner.valid(lease)) publish({ commandPhase: 'unknown', message: 'Closure outcome unknown. Check original command; no replay is allowed.' }) }
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
        const job = await api.read(lease.context, lease.signal, receipt.resource_id!)
        if (!owner.valid(lease)) return
        if (job.revision < receipt.resource_revision!) throw new Error('Closure behind receipt')
        accept(job); publish({ command: null, commandPhase: 'confirmed' })
      } else if (receipt.state === 'FAILED') {
        if (command.body.confirmation === 'CLOSE_EVENT') rejectedRevision = Number(command.body.expected_revision)
        publish({ command: null, job: null, commandPhase: 'rejected', phase: 'error', message: `Original closure command failed (${receipt.failure_reason}). Reload current authorized state before another action.` })
      }
      else publish({ commandPhase: receipt.state === 'PENDING' ? 'pending' : 'unknown', message: 'Original closure pending or unavailable. No destructive retry is permitted.' })
    } catch { if (owner.valid(lease)) publish({ commandPhase: 'unknown', message: 'Original closure could not be verified. No replay is permitted.' }) }
    finally { owner.finish(lease) }
  }
  function revalidate() {
    if (!disposed && state.commandPhase !== 'sending' && state.commandPhase !== 'checking' && state.phase !== 'loading') {
      const current = owner.capture()
      publish({ phase: current ? 'ready' : 'unavailable', draft: null, message: current ? 'Operator access checked. Load an existing job or review an eligible event closure.' : 'Operator authority unavailable. Revalidate your event access before continuing.' })
    }
  }
  return { read: () => state, subscribe(f: () => void) { listeners.add(f); return () => { listeners.delete(f) } },
    start() { if (!disposed) { owner.start(); revalidate() } }, revalidate, prepare, prepareResume, canResume, confirm, check, refresh,
    cancel() { publish({ draft: null }) },
    dispose() { if (!disposed) { disposed = true; latest = null; issuedKeys.clear(); owner.dispose(); publish({ ...empty(), phase: 'disposed' }); listeners.clear() } },
  }
}
export type EventClosure = ReturnType<typeof createEventClosure>
