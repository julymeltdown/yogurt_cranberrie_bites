/**
 * WP-07 T-609/T-611: the room screen.
 *
 * One screen, two modes:
 *
 * * the **owner** sees their room with the editor, saves with the revision the
 *   server returned, resolves a conflict explicitly, and toggles room visibility;
 * * a **visitor** sees the saved room, read-only, and gets a neutral unavailable
 *   state for a `404` (a private room is indistinguishable from no room).
 *
 * The scene is scoped to this screen: it is created when the screen mounts and
 * destroyed when it unmounts, so leaving the room releases the Phaser instance and
 * every texture it acquired. Re-entering re-reads the saved room and draws exactly
 * that layout.
 *
 * There is no presence: the screen never asks whether the owner is connected and
 * renders no "online" affordance.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { initialLayout, type RoomLayout } from './model/layout'
import { reduce, type EditorState } from './model/editor'
import { RoomEditor } from './RoomEditor'
import { createRoomScene, type RoomScene } from './scene/room-scene'
import type { RoomClient, RoomVisibility } from './api/room'
import { createPosterConverter, type PosterConverter } from './worker/client'
import './profile-room.css'

export interface RoomPageProps {
  readonly client: RoomClient
  /** The profile whose room to show. Omitted means the signed-in owner. */
  readonly profileId?: string | undefined
  /** Loads poster bytes for the scene; injected so the screen owns no media policy. */
  readonly loadPoster: (mediaId: string) => Promise<HTMLImageElement>
  /** Overrides the conversion port in tests; production uses the worker. */
  readonly converter?: PosterConverter | undefined
  /** Where "back" goes. */
  readonly backTo?: string | undefined
}

type Load =
  | { readonly phase: 'loading' }
  | { readonly phase: 'ready'; readonly layout: RoomLayout; readonly revision: number; readonly visibility: RoomVisibility; readonly isOwner: boolean }
  | { readonly phase: 'unavailable' }
  | { readonly phase: 'failed' }

export function RoomPage({ client, profileId, loadPoster, converter, backTo = '/me' }: RoomPageProps) {
  const [load, setLoad] = useState<Load>({ phase: 'loading' })
  const [attempt, setAttempt] = useState(0)
  const [editor, setEditor] = useState<EditorState>(() => ({
    draft: initialLayout(), saved: null, revision: 1, message: null, selected: null, conflict: null, saving: false,
  }))
  const [visibility, setVisibility] = useState<RoomVisibility>('PUBLIC')
  const [visibilityMessage, setVisibilityMessage] = useState<string | null>(null)
  const host = useRef<HTMLDivElement | null>(null)
  const scene = useRef<RoomScene | null>(null)
  const [sceneState, setSceneState] = useState<'idle' | 'ready' | 'failed'>('idle')
  const converterPort = useMemo(() => converter ?? createPosterConverter(), [converter])
  const ownsConverter = converter === undefined

  // Load the room once per attempt and per target.
  useEffect(() => {
    let live = true
    setLoad({ phase: 'loading' })
    const read = profileId ? client.readProfile(profileId) : client.readOwn()
    void read.then(room => {
      if (!live) return
      setLoad({ phase: 'ready', layout: room.layout, revision: room.revision, visibility: room.visibility, isOwner: !profileId || ('isOwner' in room && room.isOwner === true) })
      setVisibility(room.visibility)
      setEditor(current => reduce(current, { type: 'loaded', layout: room.layout, revision: room.revision }))
    }, (error: { name?: string }) => {
      if (!live) return
      setLoad({ phase: error?.name === 'RoomUnavailableError' ? 'unavailable' : 'failed' })
    })
    return () => { live = false }
  }, [client, profileId, attempt])

  // One scoped Phaser instance per mount; released on unmount.
  useEffect(() => {
    const element = host.current
    if (!element || load.phase !== 'ready') return
    let live = true
    let created: RoomScene | null = null
    void createRoomScene({
      host: element,
      loadPoster,
      onReady: () => { if (live) setSceneState('ready') },
      onError: () => { if (live) setSceneState('failed') },
    }).then(async instance => {
      created = instance
      scene.current = instance
      if (!live) { instance.destroy(); return }
      await instance.render(load.layout)
    }).catch(() => { if (live) setSceneState('failed') })
    return () => {
      live = false
      const owned = created ?? scene.current
      scene.current = null
      owned?.destroy()
    }
  }, [load, loadPoster])

  // Redraw when the owner edits, so the preview is the draft.
  useEffect(() => {
    if (load.phase !== 'ready' || !scene.current || sceneState !== 'ready') return
    const instance = scene.current
    const layout = load.isOwner ? editor.draft : load.layout
    Promise.resolve(instance.render(layout)).catch(() => setSceneState('failed'))
  }, [editor.draft, load, sceneState])

  // The conversion worker lives for the screen's lifetime.
  useEffect(() => () => { if (ownsConverter) converterPort.dispose() }, [converterPort, ownsConverter])

  const onVisibility = useCallback(async (next: RoomVisibility) => {
    if (load.phase !== 'ready') return
    setVisibilityMessage(null)
    try {
      const room = await client.setVisibility(load.revision, next)
      setVisibility(room.visibility)
      setLoad({ phase: 'ready', layout: room.layout, revision: room.revision, visibility: room.visibility, isOwner: true })
      setEditor(current => reduce(current, { type: 'loaded', layout: current.saved ?? current.draft, revision: room.revision }))
      setVisibilityMessage(room.visibility === 'PUBLIC' ? 'Your room is visible to visitors.' : 'Your room is private. Visitors see it as unavailable.')
    } catch (error) {
      const conflict = error as { name?: string; current?: { layout: RoomLayout; revision: number } | null }
      if (conflict.name === 'RoomConflictError' && conflict.current) {
        setEditor(current => reduce(current, { type: 'conflict', current: conflict.current!.layout, currentRevision: conflict.current!.revision }))
        setVisibilityMessage('Your room changed. Your layout is kept; resolve the change before changing visibility again.')
      } else {
        setVisibilityMessage('Room visibility could not be changed.')
      }
    }
  }, [client, load])

  return <main className="room-screen">
    <header className="room-screen__header">
      <Link to={backTo}>Back to profile</Link>
      <h1>{load.phase === 'ready' && load.isOwner ? 'Your room' : 'Room'}</h1>
    </header>
    <div className="room-stage" ref={host} aria-label="Room scene">
      {load.phase !== 'ready' && <div className="room-stage__fallback" role="status">
        {load.phase === 'loading' ? 'Opening the room…' : load.phase === 'unavailable' ? 'This room is unavailable.' : 'The room could not be opened.'}
      </div>}
      {load.phase === 'ready' && sceneState === 'failed' && <div className="room-stage__fallback" role="status">The room picture could not be drawn.</div>}
    </div>
    {load.phase === 'failed' && <button type="button" onClick={() => setAttempt(value => value + 1)}>Try again</button>}
    {load.phase === 'ready' && !load.isOwner && <p className="room-screen__status">You are visiting this room. Only the owner can change it.</p>}
    {visibilityMessage && <p role="status" aria-live="polite">{visibilityMessage}</p>}
    {load.phase === 'ready' && load.isOwner && <RoomEditor client={client} converter={converterPort} state={editor}
      dispatch={action => setEditor(current => reduce(current, action))} visibility={visibility}
      revision={load.revision} onVisibility={next => { void onVisibility(next) }} />}
  </main>
}
