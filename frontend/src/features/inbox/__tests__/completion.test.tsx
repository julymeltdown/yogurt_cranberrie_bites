// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { act } from 'react'
import { Inbox } from '../ui/Inbox'
import { setup } from './fixtures/data'

const owners: ReturnType<typeof setup>[] = []
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()) })

it.each(['logout', 'outage', 'dispose'] as const)('a broken earlier subscriber cannot retain mounted private inbox rows after %s', async mode => {
  const h = setup(); owners.push(h)
  h.controller.start()
  await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready'))
  h.controller.subscribe(() => { throw new Error('Synthetic broken observer') })
  render(<Inbox controller={h.controller} />)
  expect(screen.getByRole('article', { name: 'Party update 1' })).toBeDefined()
  act(() => {
    if (mode === 'logout') h.auth.session.replace(h.auth.session.read().generation, null)
    else if (mode === 'outage') h.setAvailable(false)
    else h.controller.dispose()
  })
  expect(h.controller.read().rows).toHaveLength(0)
  expect(screen.queryByRole('article')).toBeNull()
})
