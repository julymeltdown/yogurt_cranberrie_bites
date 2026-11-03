import { QueryClient } from '@tanstack/react-query'
import { vi } from 'vitest'
import { createHttpClient, createMemorySession } from '../../../shared/api/http'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { createMediaOwner, type MediaOwnerOptions } from '../index'
export const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
export const eventId = id(2), mediaId = id(3)
// Synthetic 1×1 PNG, the same public test bytes used by shared reader regressions.
export const bytes = () => Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGr8AAAAASUVORK5CYII='), c => c.charCodeAt(0))
export const metadata = (media = mediaId, purpose = 'POST') => ({ media_id: media, state: 'READY', revision: 1,
  width: 1, height: 1, content_type: 'image/png', purpose, read_path: `/api/concert/events/${eventId}/media/${media}/content` })
export function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
export function setup(options: Pick<MediaOwnerOptions, 'maxEntries' | 'maxConsumers'> = {}, handler?: (r: Request) => Response | Promise<Response>) {
  const session = createMemorySession(); session.replace(0, { userId: id(1), eventId, accessToken: 'memory' })
  const client = new QueryClient(), privacy = createQueryPrivacy({ client, session })
  const fetch = vi.fn<typeof globalThis.fetch>(async input => {
    const request = input as Request, path = new URL(request.url).pathname
    return handler ? handler(request) : path.endsWith('/content') ? new Response(bytes(), { headers: { 'Content-Type': 'image/png' } }) : Response.json(metadata(path.split('/').at(-1)))
  })
  const request = createHttpClient({ session, fetch }).request
  let nextUrl = 0
  const previews = { create: vi.fn(() => `blob:media-owner-${++nextUrl}`), revoke: vi.fn() }
  const auth = { session, privacy, request }, scope = privacy.capture(), owner = createMediaOwner({ auth, scope, previews, ...options })
  return { auth, owner, session, privacy, fetch, previews, scope, cleanup() { owner.dispose(); privacy.dispose(); client.clear() } }
}
