// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { EventFeed, type Post, type RenderPostAuthor } from '../../index'
import { deferred, post, setup } from '../fixtures'
import { rows } from './data'
const owned: ReturnType<typeof setup>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(f => f.dispose()) })
function fixture(options: { items?: readonly Post[]; response?: Promise<Response>; status?: number } = {}) {
  const requests: Request[] = []
  const f = setup(async input => { const r = input as Request; requests.push(r)
    if (new URL(r.url).pathname.endsWith('/notices')) return Response.json({ items: [], next_cursor: null })
    return options.response ?? Response.json({ items: options.items ?? rows, next_cursor: null }, { status: options.status ?? 200 })
  }); owned.push(f); return { ...f, requests }
}
function props(f: ReturnType<typeof fixture>, renderAuthor?: RenderPostAuthor): ComponentProps<typeof EventFeed> {
  return { controller: f.controller, onPost: vi.fn(), onCompose: vi.fn(), onLink: vi.fn(), renderAuthor }
}
it('public props retain the existing port; only loaded canonical1.1 authors reach it, unchanged and immutable', async () => {
  const pending = deferred<Response>(), f = fixture({ response: pending.promise }), portrait = vi.fn<RenderPostAuthor>(a => <span role="img" aria-label={`Portrait ${a.display_name}`}>{a.avatar.config.contract_version}</span>)
  render(<EventFeed {...props(f, portrait)} />); expect(portrait).not.toHaveBeenCalled()
  act(() => f.controller.start()); expect(portrait).not.toHaveBeenCalled()
  await act(async () => pending.resolve(Response.json({ items: rows, next_cursor: null })))
  await screen.findByRole('img', { name: 'Portrait Fixture author 6' })
  const loaded = f.controller.read().feed.data!.items
  for (const item of loaded) {
    expect(portrait.mock.calls.some(([author]) => author === item.author)).toBe(true)
    expect(item.author.avatar).toEqual(rows.find(r => r.post_id === item.post_id)!.author.avatar)
    expect(Object.isFrozen(item.author.avatar.config)).toBe(true)
    expect(screen.getByText(item.author.display_name)).toBeInTheDocument()
  }
  expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
it('preserves exact no-callback initial/unavailable markup and adds no image/resource reads', async () => {
  const f = fixture({ items: [post] }); act(() => f.controller.start()); render(<EventFeed {...props(f)} />)
  const fallback = await screen.findByRole('img', { name: `${post.author.display_name}: avatar unavailable` })
  expect(fallback.outerHTML).toBe(`<span class="feed-initial" role="img" aria-label="${post.author.display_name}: avatar unavailable">${Array.from(post.author.display_name.trim())[0]?.toLocaleUpperCase() ?? '?'}</span>`)
  expect(document.querySelector('.feed-portrait img,.feed-portrait canvas')).toBeNull(); expect(f.requests).toHaveLength(2)
})
it('passes retained1.0 author configs through the same union callback without rewriting them as whole characters', async () => {
  const f = fixture({ items: [post] }), portrait = vi.fn<RenderPostAuthor>(a => <span>{a.avatar.config.contract_version} caller fallback</span>)
  act(() => f.controller.start()); render(<EventFeed {...props(f, portrait)} />)
  await screen.findByText('1.0.0 caller fallback'); expect(portrait.mock.calls.at(-1)?.[0].avatar).toEqual(post.author.avatar)
})
it('does not substitute any character or initial when an installed callback intentionally returns null', async () => {
  const f = fixture({ items: [rows[0]!] }), portrait = vi.fn<RenderPostAuthor>(() => null)
  act(() => f.controller.start()); render(<EventFeed {...props(f, portrait)} />); await screen.findByText(rows[0]!.text)
  expect(portrait).toHaveBeenCalled(); expect(document.querySelector('.feed-portrait')).toBeEmptyDOMElement()
  expect(screen.queryByRole('img')).toBeNull()
})
it('uses new loaded author revision/config after explicit refresh and callback replacement without stale fallback', async () => {
  let items = [rows[0]!]
  const f = setup(async input => Response.json({ items: new URL((input as Request).url).pathname.endsWith('/notices') ? [] : items, next_cursor: null })); owned.push(f)
  const first = vi.fn<RenderPostAuthor>(a => <span>{JSON.stringify(a.avatar)}</span>), second = vi.fn<RenderPostAuthor>(a => <span>{a.display_name} replacement</span>)
  act(() => f.controller.start()); const view = render(<EventFeed controller={f.controller} onPost={vi.fn()} onCompose={vi.fn()} onLink={vi.fn()} renderAuthor={first} />)
  await screen.findByText(JSON.stringify(rows[0]!.author.avatar))
  items = [{ ...rows[0]!, revision: 2, author: { ...rows[0]!.author, avatar: rows[5]!.author.avatar } }]
  await act(async () => f.controller.loadFeed()); expect(screen.queryByText(JSON.stringify(rows[0]!.author.avatar))).toBeNull(); expect(screen.getByText(JSON.stringify(rows[5]!.author.avatar))).toBeInTheDocument()
  view.rerender(<EventFeed controller={f.controller} onPost={vi.fn()} onCompose={vi.fn()} onLink={vi.fn()} renderAuthor={second} />)
  expect(second.mock.calls.at(-1)?.[0]).toBe(f.controller.read().feed.data!.items[0]!.author)
})
it.each(['unavailable', 'privacy', 'identity'] as const)('evicts portrait output on %s', async mode => {
  const f = fixture({ items: [rows[0]!] })
  function Borrowed() { return <span>Borrowed portrait</span> }
  const portrait = vi.fn<RenderPostAuthor>(() => <Borrowed />), view = render(<EventFeed {...props(f, portrait)} />)
  act(() => f.controller.start()); await screen.findByText('Borrowed portrait')
  act(() => { if (mode === 'unavailable') f.setAccess({ available: false, readOnly: true }); else if (mode === 'privacy') f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'suspended'); else f.auth.session.replace(f.auth.session.read().generation, null) })
  await waitFor(() => expect(screen.queryByText('Borrowed portrait')).toBeNull())
  expect(screen.queryByRole('button', { name: 'Write a post' })).toBeNull()
  view.unmount()
})
it.each(['denied', 'invalid'] as const)('rejects %s response before invoking the portrait callback', async mode => {
  const bad = { ...rows[0]!, author: { ...rows[0]!.author, avatar: { ...rows[0]!.author.avatar, config: { ...rows[0]!.author.avatar.config, item_ids: {} } } } }
  const response = Promise.resolve(Response.json({ items: mode === 'invalid' ? [bad] : rows, next_cursor: null }, { status: mode === 'denied' ? 403 : 200 }))
  const f = fixture({ response }), portrait = vi.fn<RenderPostAuthor>(() => <span>Hidden portrait</span>)
  act(() => f.controller.start()); render(<EventFeed {...props(f, portrait)} />); await screen.findByRole('alert'); expect(portrait).not.toHaveBeenCalled()
})
it('read-only loaded data keeps authorized portraits and removes compose', async () => {
  const f = fixture({ items: [rows[1]!] }), portrait = vi.fn<RenderPostAuthor>(() => <span>Read-only portrait</span>)
  f.setAccess({ available: true, readOnly: true }); act(() => f.controller.start()); render(<EventFeed {...props(f, portrait)} />)
  await screen.findByText('Read-only portrait'); expect(screen.queryByRole('button', { name: 'Write a post' })).toBeNull()
})
it('keeps thread/link/compose callbacks separate from portrait rendering and sends no business writes', async () => {
  const f = fixture({ items: [rows[0]!] }), p = props(f, a => <span>{a.display_name} portrait</span>)
  act(() => f.controller.start()); render(<EventFeed {...p} />); await screen.findByText(rows[0]!.text)
  fireEvent.click(screen.getByRole('button', { name: 'View 42 reactions in thread' })); expect(p.onPost).toHaveBeenCalledWith(rows[0]!.post_id)
  fireEvent.click(screen.getByRole('button', { name: 'Open party' })); expect(p.onLink).toHaveBeenCalledWith(rows[0]!.links[0])
  fireEvent.click(screen.getByRole('button', { name: 'Write a post' })); expect(p.onCompose).toHaveBeenCalledOnce(); expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
