// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { act } from 'react'
import { BlockUserButton, BlocksPanel } from '../panels'
import { harness, ids } from './fixtures'

const owners: ReturnType<typeof harness>[] = []
const own = () => {
  const h = harness(() => Response.json({ items: [{ target_user_id: ids.target, blocked_by_me: true, revision: 1 }], next_cursor: null }))
  owners.push(h); return h
}
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()) })

it('requires fresh block consent after the visible target changes', () => {
  const h = own(), view = render(<BlockUserButton controller={h.controller} userId={ids.target} />)
  fireEvent.click(screen.getByRole('button', { name: 'Block user' }))
  expect(screen.getByRole('dialog', { name: 'Block this user?' })).toBeDefined()
  view.rerender(<BlockUserButton controller={h.controller} userId={ids.media} />)
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(h.requests).toHaveLength(0)
  view.rerender(<BlockUserButton controller={h.controller} userId={ids.target} />)
  expect(screen.queryByRole('dialog')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Block user' }))
  expect(screen.getByRole('dialog', { name: 'Block this user?' })).toBeDefined()
})

it('does not carry block consent into a replacement safety owner', () => {
  const first = own(), next = own(), view = render(<BlockUserButton controller={first.controller} userId={ids.target} />)
  fireEvent.click(screen.getByRole('button', { name: 'Block user' }))
  view.rerender(<BlockUserButton controller={next.controller} userId={ids.target} />)
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(next.requests).toHaveLength(0)
})

it('does not carry an unblock confirmation into a replacement account owner', async () => {
  const first = own(), next = own(), view = render(<BlocksPanel controller={first.controller} onBack={vi.fn()} />)
  await act(async () => { await first.controller.loadBlocks() })
  fireEvent.click(screen.getByRole('button', { name: 'Unblock user' }))
  expect(screen.getByRole('dialog', { name: 'Unblock this user?' })).toBeDefined()
  await act(async () => { view.rerender(<BlocksPanel controller={next.controller} onBack={vi.fn()} />) })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(next.requests.every(r => r.method === 'GET')).toBe(true)
})
