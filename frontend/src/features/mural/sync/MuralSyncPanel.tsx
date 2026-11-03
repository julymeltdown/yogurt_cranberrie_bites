import { useSyncExternalStore } from 'react'
import { MuralEditor } from '../editor/MuralEditor'
import type { MuralSyncController } from './controller'

export function MuralSyncPanel({ sync }: { sync: MuralSyncController }) {
  const state = useSyncExternalStore(sync.subscribe, sync.read, sync.read)
  const editor = useSyncExternalStore(sync.editor.subscribe, sync.editor.read, sync.editor.read)
  return <div className="mcr-mural-sync"><MuralEditor controller={sync.editor} onRefresh={() => { void sync.refresh() }} synchronizationStatus={<>
    <p className="mcr-mural__sync-state" data-phase={state.phase} role="status" aria-label="Wall synchronization status">{state.phase === 'ready' ? `Wall synchronized at revision ${state.revision}.`
      : state.phase === 'gap' ? 'Wall updates arrived out of order. Editing is paused while the missing revision is recovered.'
      : state.phase === 'loading' ? 'Recovering the confirmed wall. No local preview has been committed.'
      : 'Wall synchronization unavailable. Reconnect the world and refresh before editing.'}</p>
    {editor.command && <p role="status" aria-label="Original placement status">{editor.phase === 'sending' || editor.phase === 'checking'
      ? 'Waiting for the original placement result.' : 'Original placement unresolved. Check its status; no new placement or automatic replay is allowed.'}</p>}
    </>} />
  </div>
}
