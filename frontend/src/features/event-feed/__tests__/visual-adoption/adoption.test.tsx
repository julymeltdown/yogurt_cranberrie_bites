// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { EventFeed } from '../../EventFeed'
import { post, setup, id } from '../fixtures'
let f: ReturnType<typeof setup> | undefined
afterEach(() => { cleanup(); f?.dispose() })
it('keeps one public post/media consumer and maps semantic filters, thread and compose to existing callbacks', async () => {
  const mediaPost = { ...post, media_ids: [id(75)] }
  f = setup(async input => Response.json({ items: new URL((input as Request).url).pathname.endsWith('/feed') ? [mediaPost] : [], next_cursor: null }))
  await act(async () => { f!.controller.start() })
  const onPost = vi.fn(), onCompose = vi.fn(), renderMedia = vi.fn(() => <span>Authorized media seam</span>)
  render(<EventFeed controller={f.controller} onPost={onPost} onCompose={onCompose} onLink={vi.fn()} renderMedia={renderMedia} />)
  expect(renderMedia).toHaveBeenCalledTimes(1)
  expect(screen.getAllByText('Authorized media seam')).toHaveLength(1)
  expect(screen.getAllByRole('tab').map(tab => tab.textContent)).toEqual(['ALL', 'MEETUP', 'GIVEAWAY', 'NOTICE'])
  expect(document.querySelector('.concert-window,h1')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: /Open thread/ })); expect(onPost).toHaveBeenCalledWith(post.post_id)
  fireEvent.click(screen.getByRole('button', { name: 'Write a post' })); expect(onCompose).toHaveBeenCalledOnce()
  const images = screen.getByRole('checkbox', { name: 'Images only', hidden: true }); fireEvent.click(images)
  await waitFor(() => expect((images as HTMLInputElement).checked).toBe(true))
  expect(f.controller.read().filter).toBe('WITH_MEDIA')
  expect(document.querySelectorAll('[role=tabpanel]:not([hidden])')).toHaveLength(1)
  expect(document.querySelector('img[src],canvas')).toBeNull()
})
