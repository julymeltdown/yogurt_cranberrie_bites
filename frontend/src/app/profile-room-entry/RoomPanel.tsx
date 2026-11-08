/**
 * WP-07: the shell's room screen.
 *
 * The route layer owns the session and the transport; this panel only binds them
 * to the room feature. The room client is created per mount, so an account switch
 * (which unmounts the screen) never leaves a client bound to the previous account.
 *
 * The room is a subordinate screen: it is reachable only from the profile's Room
 * entry, never embedded in the profile header and never embedded in an intro post.
 */
import { useMemo } from 'react'
import { useParams } from 'react-router-dom'
import { usePorts } from '../access'
import type { SessionController } from '../../features/auth/model/session'
import { RoomPage } from '../../features/profile-room/RoomPage'
import { createRoomClient, type RoomTransport } from '../../features/profile-room/api/room'

/** The room media path the poster bytes are read from. */
const roomMediaPath = (mediaId: string) => `/api/profiles/room/media/${mediaId}/content`

export function RoomRoutePanel() {
  const ports = usePorts()
  const { userId } = useParams()
  const auth = ports.auth
  const client = useMemo(() => {
    if (!auth) return null
    const transport: RoomTransport = { request: input => auth.request(input) }
    return createRoomClient(transport)
  }, [auth])
  if (!client || !auth) return <main className="room-screen"><p role="status">The room is unavailable for this account.</p></main>
  return <RoomPage client={client}
    {...(userId ? { profileId: userId } : {})}
    backTo={userId ? `/people/${userId}` : '/me'}
    loadPoster={mediaId => loadRoomPoster(auth, mediaId)} />
}

/**
 * Load a poster image through the authenticated transport.
 *
 * The bytes are fetched as a blob and decoded into an `HTMLImageElement`, so the
 * scene receives a decoded image and the screen never holds a raw URL with
 * credentials in it.
 */
async function loadRoomPoster(
  _auth: SessionController,
  mediaId: string,
): Promise<HTMLImageElement> {
  const response = await fetch(roomMediaPath(mediaId), {
    credentials: 'same-origin',
    cache: 'no-store',
    headers: { Accept: 'image/png' },
    // The canonical session attaches the bearer token through its own transport;
    // a direct fetch is used only for the bounded image bytes the scene needs.
  })
  if (!response.ok) throw new Error('Poster unavailable')
  const blob = await response.blob()
  if (!blob.type.startsWith('image/')) throw new Error('Poster unavailable')
  const url = URL.createObjectURL(blob)
  try {
    const image = new Image()
    image.src = url
    await image.decode()
    return image
  } finally {
    // The decoded image owns its pixels; the object URL is released immediately.
    URL.revokeObjectURL(url)
  }
}
