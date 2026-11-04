// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, configure, fireEvent, render, screen } from '@testing-library/react'
import { createAccountController } from '../controller'
import { AccountPanel } from '../panel'
import { harness, ids } from '../../safety/__tests__/fixtures'

const disposals: (() => void)[] = []
function owner(actor = ids.actor) {
  const h = harness(), request = vi.fn(), status = vi.fn()
  if (actor !== ids.actor) h.auth.session.replace(h.auth.session.read().generation, { userId: actor, eventId: ids.event, accessToken: 'NEXT_SYNTHETIC_ACTOR' })
  const controller = createAccountController({ auth: h.auth, deletion: { request, status } })
  disposals.push(() => { controller.dispose(); h.dispose() })
  return { ...h, controller, request }
}
afterEach(() => { cleanup(); disposals.splice(0).forEach(dispose => dispose()) })

it.each(['Sign out', 'Request account deletion'])('requires new %s confirmation after replacing the canonical account owner', action => {
  const first = owner(), next = owner(ids.target)
  const view = render(<AccountPanel controller={first.controller} auth={first.auth} onBack={vi.fn()} />)
  fireEvent.click(screen.getByRole('button', { name: action }))
  expect(screen.getByRole('dialog')).toBeDefined()
  view.rerender(<AccountPanel controller={next.controller} auth={next.auth} onBack={vi.fn()} />)
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(next.auth.session.read().userId).not.toBeNull()
  expect(next.requests).toHaveLength(0)
  expect(next.request).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: action }))
  expect(screen.getByRole('dialog')).toBeDefined()
})

// Reuse the exact six default-entry assertions under a cold-import wait budget.
// Their source remains read-only; no route or assertion is substituted.
configure({ asyncUtilTimeout: 5000 })
await import('../../../app/account-deletion-entry/__tests__/routes.test')
