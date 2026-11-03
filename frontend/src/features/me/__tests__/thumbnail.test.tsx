// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { AvatarThumbnail, type ReviewedThumbnailSource } from '../ui/AvatarThumbnail'
import { privacyOnly, participant } from './fixtures'
import { createReviewedAvatarCompositor } from '../../../entities/avatar/runtime'
vi.mock('../../../entities/avatar/runtime', () => ({ createReviewedAvatarCompositor: vi.fn() }))
afterEach(()=>{cleanup();vi.resetAllMocks()})
it('uses the shared reviewed builder and removes owned thumbnail immediately on privacy change', async () => {
  const auth=privacyOnly(), canvas=document.createElement('canvas'), thumbnailDispose=vi.fn(()=>{canvas.width=canvas.height=0}), dispose=vi.fn()
  vi.mocked(createReviewedAvatarCompositor).mockResolvedValue({thumbnail:vi.fn(()=>({canvas,fallback:false,dispose:thumbnailDispose})),dispose} as never)
  const source={scope:auth.privacy.capture(),options:{document:{retained:true},pin:{manifest_id:'test',manifest_version:1,content_hash:'sha256:test'}}} as ReviewedThumbnailSource
  render(<AvatarThumbnail auth={auth} config={participant().profile.avatar.config} source={source} />)
  await screen.findByRole('img',{name:'Your saved avatar, front view'})
  expect(createReviewedAvatarCompositor).toHaveBeenCalledWith(expect.objectContaining({document:source.options.document,pin:source.options.pin,signal:expect.any(AbortSignal)}))
  act(()=>{auth.privacy.changePolicy(auth.privacy.capture(),'consent-revoked')})
  expect(screen.queryByRole('img')).toBeNull();expect(thumbnailDispose).toHaveBeenCalled();expect(dispose).toHaveBeenCalled();expect(canvas.width).toBe(0)
  auth.privacy.dispose();auth.client.clear()
})
it('does not invoke the asset builder for an old bootstrap lease or fabricate default art', async () => {
  const auth=privacyOnly(), stale=auth.privacy.capture();auth.privacy.changePolicy(stale,'handle-replaced')
  render(<AvatarThumbnail auth={auth} config={participant().profile.avatar.config} source={{scope:stale,options:{} as never}} />)
  await waitFor(()=>expect(screen.getByText(/Avatar preview unavailable/)).toBeTruthy())
  expect(createReviewedAvatarCompositor).not.toHaveBeenCalled();expect(screen.queryByRole('img')).toBeNull()
  auth.privacy.dispose();auth.client.clear()
})
it('reports unavailable after removing a previously available source and disposes an ignored-abort late builder', async () => {
  const auth=privacyOnly(), canvas=document.createElement('canvas'),dispose=vi.fn(),thumbnailDispose=vi.fn()
  vi.mocked(createReviewedAvatarCompositor).mockResolvedValueOnce({thumbnail:()=>({canvas,fallback:false,dispose:thumbnailDispose}),dispose} as never)
  const config=participant().profile.avatar.config,source={scope:auth.privacy.capture(),options:{} as never}
  const view=render(<AvatarThumbnail auth={auth} config={config} source={source} />)
  await screen.findByRole('img')
  view.rerender(<AvatarThumbnail auth={auth} config={config} />)
  await screen.findByText(/Avatar preview unavailable/);expect(dispose).toHaveBeenCalled();expect(thumbnailDispose).toHaveBeenCalled()
  let resolve!:(v:never)=>void;const lateDispose=vi.fn()
  vi.mocked(createReviewedAvatarCompositor).mockImplementationOnce(()=>new Promise(r=>{resolve=r}))
  view.rerender(<AvatarThumbnail auth={auth} config={config} source={source} />)
  await waitFor(()=>expect(createReviewedAvatarCompositor).toHaveBeenCalledTimes(2))
  view.unmount();await act(async()=>resolve({dispose:lateDispose} as never));expect(lateDispose).toHaveBeenCalled()
  auth.privacy.dispose();auth.client.clear()
})
