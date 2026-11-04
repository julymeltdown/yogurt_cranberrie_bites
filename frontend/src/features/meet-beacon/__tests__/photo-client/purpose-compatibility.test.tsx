// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { FeedImage } from '../../../event-feed/FeedImage'
import { createFeedThreadController } from '../../../feed-thread/controller'
import { setup, post, id } from '../../../event-feed/__tests__/fixtures'
import type { ImageUploadPort, ReadyMedia } from '../../../../shared/media/types'
const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
it('the compatibility-only type widening preserves actual Feed display and composer POST-only guards', async () => {
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  const h = setup(vi.fn(async () => Response.json(post))); stops.push(h.dispose)
  const mediaId = id(700), scope = h.auth.session.read()
  const attachment: ReadyMedia = { media_id: mediaId, state: 'READY', revision: 3, width: 1, height: 1, content_type: 'image/png',
    purpose: 'BEACON', read_path: `/api/concert/events/${scope.eventId}/media/${mediaId}/content` }
  const upload: ImageUploadPort = { read: () => ({ phase: 'ready', scope, previewUrl: 'blob:fixture', progress: 100, attachment, error: null }),
    subscribe: () => () => {}, select: async () => {}, upload: async () => {}, recover: async () => {}, retry: async () => {}, cancel() {}, dispose() {} }
  const composer = createFeedThreadController({ auth: h.auth, access: h.access, media: upload }); stops.push(composer.dispose); composer.start()
  expect(composer.attachReadyMedia()).toBe(false); expect(composer.read().draft.media_ids).toEqual([])
  const load = vi.fn(async () => {})
  const snapshot = { phase: 'ready' as const, scope, previewUrl: 'blob:fixture', media: attachment, error: null }
  render(<FeedImage createPreview={() => ({ read: () => snapshot,
    subscribe: () => () => {}, load, cancel() {}, dispose() {} })} privacy={h.auth.privacy} mediaId={mediaId} eventId={scope.eventId!} alt="Post attachment" />)
  await waitFor(() => expect(load).toHaveBeenCalledTimes(1))
  expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
