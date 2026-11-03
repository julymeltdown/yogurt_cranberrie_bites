import { useEffect, useState, useSyncExternalStore } from 'react'
import { WholeCharacterView, type WholeCharacterRuntime } from '../../../entities/avatar/whole-character'
import { ConcertAction, ConcertWindow } from '../../../shared/ui/concert-window'
import type { WholeCharacterEditorController, WholeEditorNotice } from './controller'
import { sameWholeConfig } from './model'
import './whole-character.css'

const messages: Record<Exclude<WholeEditorNotice, null>, string> = {
  'load-failed': 'The saved character could not be verified. Reload to try again.',
  'art-unavailable': 'Reviewed character artwork is unavailable. Reload the collection to try again.',
  'invalid-selection': 'That character is unavailable in the current collection.',
  'save-failed': 'Your character was not saved. Your draft is still here.',
  saved: 'Character saved.', 'saved-refresh-needed': 'Character saved. Reload the world and profile to refresh their display.',
  conflict: 'Your saved character changed elsewhere. Your draft is still here. Check the latest saved character.',
  uncertain: 'The save result is unknown. Check the latest saved character before another save.',
  'review-server': 'Latest saved character loaded. Choose whether to keep your draft or use the saved character.',
}
export function WholeCharacterEditor({ controller }: { readonly controller: WholeCharacterEditorController<WholeCharacterRuntime> }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), [page, setPage] = useState(0)
  useEffect(() => { controller.start(); return () => { controller.stop() } }, [controller])
  const last = Math.max(0, Math.ceil(state.choices.length / 3) - 1), current = Math.min(page, last)
  const choices = state.choices.slice(current * 3, current * 3 + 3)
  const editable = state.phase === 'ready' && !!state.draft
  const show = !!state.draft && !['idle', 'unavailable', 'disposed'].includes(state.phase)
  const selected = state.choices.find(choice => state.draft && sameWholeConfig(choice.config, state.draft))
  const dirty = !!state.draft && !!state.persisted && !sameWholeConfig(state.draft, state.persisted.config)
  const randomAvailable = editable && state.choices.some(choice => !sameWholeConfig(choice.config, state.draft!))
  const busy = ['loading', 'saving', 'reconciling'].includes(state.phase)
  return <section className="mcr-whole-editor" aria-label="Whole character editor" aria-busy={busy}>
    <ConcertWindow title="CUSTOMIZE" headingLevel={1} icon="me" footer={<>
      <div className="mcr-whole-actions">
        <ConcertAction tone="paper" aria-label="Random character" disabled={!randomAvailable} onClick={() => controller.random()}>Random</ConcertAction>
        <ConcertAction tone="paper" aria-label="Reset draft" disabled={!editable || !!state.candidate} onClick={() => controller.reset()}>Reset</ConcertAction>
        <ConcertAction aria-label="Save character" pending={state.phase === 'saving'} disabled={!editable || !dirty || state.needsReconciliation} onClick={() => void controller.save()}>Save</ConcertAction>
      </div>
      {state.needsReconciliation && !state.candidate && <p className="mcr-whole-note">Reset restores the local draft. It cannot undo an unknown save result.</p>}
      {state.candidate && <p className="mcr-whole-note">Choose Keep my draft or Use saved character before resetting.</p>}
    </>}>
      <div className="mcr-whole-preview">
        {show && state.draft ? <WholeCharacterView runtime={controller.runtime} config={state.draft} size={256} label={`Draft character: ${selected?.label ?? state.draft.preset_id}`} />
          : <p>{state.phase === 'loading' ? 'Loading your character…' : 'Character preview unavailable'}</p>}
      </div>
      {!!state.choices.length && show && <section className="mcr-whole-choices" aria-label="Character choices">
        <header><h2>Characters</h2><span aria-live="polite">{current * 3 + 1}–{Math.min(current * 3 + 3, state.choices.length)} / {state.choices.length}</span></header>
        <div className="mcr-whole-choice-row">
          <button type="button" aria-label="Previous characters" aria-disabled={current === 0} onClick={() => { if (current > 0) setPage(current - 1) }}>‹</button>
          <div className="mcr-whole-grid" role="group" aria-label="Whole character choices">
            {choices.map(choice => <button key={choice.id} type="button" className="mcr-whole-choice" aria-label={`Choose ${choice.label}`}
              aria-pressed={!!state.draft && sameWholeConfig(choice.config, state.draft)} disabled={!editable} onClick={() => controller.select(choice.id)}>
              <WholeCharacterView runtime={controller.runtime} config={choice.config} size={64} label={choice.label} />
              <span className="mcr-whole-sr-only">{choice.label}</span>
            </button>)}
          </div>
          <button type="button" aria-label="Next characters" aria-disabled={current === last} onClick={() => { if (current < last) setPage(current + 1) }}>›</button>
        </div>
      </section>}
      <div className={`mcr-whole-status${state.phase === 'ready' && !state.notice && !state.needsReconciliation ? ' mcr-whole-sr-only' : ''}`}>
        <p role="status" aria-live="polite">{state.phase === 'saving' ? 'Saving character…' : state.phase === 'reconciling' ? 'Checking latest saved character…'
          : state.phase === 'loading' ? 'Loading your character…' : state.notice ? messages[state.notice]
            : ['idle', 'unavailable', 'disposed'].includes(state.phase) ? 'Character editing is unavailable for this session.' : 'Changes are a draft until you save.'}</p>
        {state.notice === 'art-unavailable' && <button type="button" disabled={busy} onClick={() => controller.retry()}>Reload character collection</button>}
        {state.notice === 'load-failed' && !state.needsReconciliation && <button type="button" disabled={busy} onClick={() => void controller.reload()}>Reload saved character</button>}
        {state.needsReconciliation && !state.candidate && <button type="button" disabled={busy} onClick={() => void controller.reload()}>Check latest saved character</button>}
        {state.candidate && <section aria-label="Saved character reconciliation"><h2>Saved character · revision {state.candidate.revision}</h2>
          <p>Keeping your draft uses this revision for your next explicit save.</p>
          <button type="button" disabled={busy} onClick={() => controller.resolveConflict('keep-draft')}>Keep my draft</button>
          <button type="button" disabled={busy} onClick={() => controller.resolveConflict('use-server')}>Use saved character</button>
        </section>}
      </div>
    </ConcertWindow>
  </section>
}
