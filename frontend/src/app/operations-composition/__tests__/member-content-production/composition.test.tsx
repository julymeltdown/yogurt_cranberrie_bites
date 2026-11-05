// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom/vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createProductionPorts } from '../../../ports'
import { createAppRoutes } from '../../../router'
import { MEMBER_REVIEW_LIMIT } from '../../member-content'
import type { ModerationController } from '../../../../features/admin/reports'
import { owners, setup } from './harness'
import { contentKinds, id, memberIds } from './server'
afterEach(() => { cleanup(); owners.splice(0).forEach(owner => owner.dispose()) })
async function member() {
  const h = setup(); await h.entry.start()
  const controller = h.entry.read().memberContent!.member(memberIds[0]!)!
  await controller.load(); expect(controller.read().phase).toBe('ready')
  return { ...h, controller }
}
async function command(controller: ModerationController, action: string) {
  controller.draft(action, 'Reviewed factual moderation reason.'); controller.review(); await controller.submit()
}
it('constructs Moderator members with the actual filtered list and minimal retained profile', async () => {
  const h = await member()
  expect(h.controller.read().selected).toMatchObject({ user_id: memberIds[0], profile: { display_name: 'Review member 1' } })
  expect(JSON.stringify(h.controller.read())).not.toMatch(/PRIVATE_PROFILE_BIO|avatar|item_ids/)
  const reads = h.server.requests.filter(r => new URL(r.url).pathname.includes('/admin/users'))
  expect(reads).toHaveLength(1); expect(new URL(reads[0]!.url).searchParams.get('user_id')).toBe(memberIds[0])
  expect(h.server.requests.some(r => new URL(r.url).pathname.endsWith('/admin/settings'))).toBe(false)
  expect(reads[0]!.cache).toBe('no-store'); expect(reads[0]!.headers.get('Authorization')).toContain('Bearer ')
})
it.each([403, 503])('keeps both review children when Operator settings independently returns %s', async status => {
  const h = setup('OPERATOR'); h.server.state.settingsStatus = status; await h.entry.start()
  const child = h.entry.read().memberContent!
  expect(child.isCurrent()).toBe(true); expect(child.member(memberIds[0]!)).not.toBeNull(); expect(child.content()).not.toBeNull()
  expect(h.entry.read().operations).toBeNull(); expect(await h.admin.authorize(h.scope, new AbortController().signal)).toBe(true)
})
it('rejects malformed targets before construction, private dispatch or consuming a slot', async () => {
  const h = setup(); await h.entry.start(); const child = h.entry.read().memberContent!, count = h.server.requests.length
  for (const value of ['bad', memberIds[0]!.toUpperCase(), memberIds[0]! + '/extra', '%2f']) {
    if (value !== memberIds[0]) expect(child.member(value)).toBeNull()
  }
  expect(h.server.requests).toHaveLength(count)
  for (let n = 0; n < MEMBER_REVIEW_LIMIT; n++) expect(child.member(id(1000 + n))).not.toBeNull()
  expect(child.member(id(2000))).toBeNull()
})
it('bounds the registry without evicting a pending member intent', async () => {
  const h = await member(); h.server.moderation.mode = 'pending'; await command(h.controller, 'SUSPEND')
  const child = h.entry.read().memberContent!
  for (let n = 1; n < MEMBER_REVIEW_LIMIT; n++) expect(child.member(id(1000 + n))).not.toBeNull()
  expect(child.member(id(2000))).toBeNull(); expect(child.member(memberIds[0]!)).toBe(h.controller)
  expect(h.controller.read().phase).toBe('pending'); expect(h.server.writes()).toHaveLength(1)
})
it('sends exact suspension and reverse action inputs only after separate review', async () => {
  const h = await member(); await command(h.controller, 'SUSPEND')
  expect(h.controller.read()).toMatchObject({ phase: 'applied', selected: { state: 'SUSPENDED', revision: 2 } })
  expect(await h.server.writes()[0]!.clone().json()).toEqual({ action: 'SUSPEND', reason: 'Reviewed factual moderation reason.', expected_revision: 1, expires_at: null })
  await h.controller.load(); await command(h.controller, 'UNSUSPEND')
  expect(h.controller.read()).toMatchObject({ phase: 'applied', selected: { state: 'ACTIVE', revision: 3 } })
  expect(h.server.writes()[0]!.headers.get('Idempotency-Key')).not.toBe(h.server.writes()[1]!.headers.get('Idempotency-Key'))
})
it.each(contentKinds)('uses metadata-only %s hide/restore with the frozen target and revision', async kind => {
  const h = setup(); await h.entry.start(); const controller = h.entry.read().memberContent!.content()!
  await controller.load(); const row = h.server.content.find(row => row.target_type === kind)!
  await controller.select(`${kind}:${row.target_id}`); await command(controller, 'HIDE')
  expect(controller.read()).toMatchObject({ phase: 'applied', selected: { target_type: kind, state: 'HIDDEN', revision: 2 } })
  expect(await h.server.writes()[0]!.clone().json()).toEqual({ target_type: kind, target_id: row.target_id, action: 'HIDE', reason: 'Reviewed factual moderation reason.', expected_revision: 1 })
  await controller.load(); await controller.select(`${kind}:${row.target_id}`); await command(controller, 'RESTORE')
  expect(controller.read()).toMatchObject({ phase: 'applied', selected: { state: 'VISIBLE', revision: 3 } })
})
it.each(['pending', 'lost'] as const)('retains the %s key through sibling borrowers and 404 status recovery', async mode => {
  const h = await member(); h.server.moderation.mode = mode; await command(h.controller, 'SUSPEND')
  const key = h.server.writes()[0]!.headers.get('Idempotency-Key')!
  const child = h.entry.read().memberContent!; await child.member(memberIds[1]!)!.load(); await child.content()!.load()
  expect(child.member(memberIds[0]!)).toBe(h.controller); await h.controller.load(); await h.controller.submit()
  h.server.moderation.statusHttp = 404; await h.controller.checkStatus(); expect(h.controller.read().phase).toBe('unknown')
  h.server.moderation.statusHttp = 200; h.server.settle(key, 'SUCCEEDED'); await h.controller.checkStatus()
  expect(h.controller.read()).toMatchObject({ phase: 'applied', selected: { state: 'SUSPENDED', revision: 2 } })
  const status = h.server.requests.filter(r => new URL(r.url).pathname.endsWith('/commands'))
  expect(status).toHaveLength(2)
  for (const request of status) expect(Object.fromEntries(new URL(request.url).searchParams)).toEqual({ operation_id: 'moderateConcertUser', idempotency_key: key })
  expect(h.server.writes()).toHaveLength(1)
})
it('never promotes a failed receipt to success or permits a new stale action', async () => {
  const h = await member(); h.server.moderation.mode = 'pending'; await command(h.controller, 'SUSPEND')
  h.server.settle(h.server.writes()[0]!.headers.get('Idempotency-Key')!, 'FAILED'); await h.controller.checkStatus()
  expect(h.controller.read()).toMatchObject({ phase: 'failed', selected: { state: 'ACTIVE', revision: 1 } })
  await command(h.controller, 'SUSPEND'); expect(h.server.writes()).toHaveLength(1)
})
it.each(['null-profile', 'foreign-member', 'extra-content-field'] as const)('fails closed for actual malformed %s producer data', async kind => {
  const h = setup(); await h.entry.start(); const child = h.entry.read().memberContent!
  if (kind === 'extra-content-field') {
    h.server.moderation.contentListOverride = { items: [{ ...h.server.content[0], title: 'PRIVATE invented title' }], next_cursor: null }
    const controller = child.content()!; await controller.load(); expect(controller.read()).toMatchObject({ phase: 'failed', rows: [] })
  } else {
    const row = h.server.members.get(memberIds[0]!)!
    h.server.moderation.memberListOverride = { items: [{ ...row, ...(kind === 'null-profile' ? { profile: null } : { user_id: memberIds[1], profile: { ...row.profile, user_id: memberIds[1] } }) }], next_cursor: null }
    const controller = child.member(memberIds[0]!)!; await controller.load(); expect(controller.read()).toMatchObject({ phase: 'failed', rows: [], selected: null })
  }
})
it.each(['replacement', 'expiry', 'logout', 'dispose'] as const)('withdraws every retained child on %s before owner callbacks', async kind => {
  const h = await member(), child = h.entry.read().memberContent!, content = child.content()!
  await content.load(); h.controller.draft('SUSPEND', 'Private factual reason')
  const observed: number[] = []
  h.entry.subscribe(() => { if (!child.isCurrent()) observed.push(h.controller.read().rows.length + content.read().rows.length) })
  if (kind === 'replacement') { h.server.replaceDecision(); await h.entry.refresh() }
  else if (kind === 'expiry') { h.advance(61_000); h.entry.read() }
  else if (kind === 'logout') h.auth.session.replace(h.auth.session.read().generation, null)
  else h.entry.dispose()
  expect(observed.length).toBeGreaterThan(0); expect(observed.every(count => count === 0)).toBe(true); expect(child.member(memberIds[0]!)).toBeNull(); expect(h.controller.read().reason).toBe('')
})
it('denies no-assignment access without constructing review children or private reads', async () => {
  const h = setup(null); await h.entry.start(); expect(h.entry.read().memberContent).toBeNull()
  expect(h.server.requests.some(r => new URL(r.url).pathname.includes('/admin/'))).toBe(false)
})
it('mounts the actual default routes and preserves member A through member B and Content', async () => {
  const h = setup(); const ports = createProductionPorts(h.auth); owners.push({ dispose: () => ports.dispose?.() })
  const ready = new Promise<void>(resolve => { const stop = h.auth.subscribe(() => { if (h.auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
  h.auth.start(); await ready; expect(await h.auth.login('review@example.com', 'TEST_ONLY_password')).toBe(true)
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [`/admin/users/${memberIds[0]}`] }); owners.push({ dispose: () => router.dispose() })
  render(<RouterProvider router={router}/>); await screen.findByRole('heading', { name: 'Review Review member 1' })
  expect(document.querySelector('main main')).toBeNull(); expect(screen.queryByText('PRIVATE_PROFILE_BIO')).toBeNull()
  fireEvent.change(screen.getByLabelText('Action'), { target: { value: 'SUSPEND' } }); fireEvent.change(screen.getByLabelText('Reason for this action'), { target: { value: 'A factual reason' } })
  fireEvent.click(screen.getByRole('button', { name: 'Review action' })); h.server.moderation.mode = 'pending'
  fireEvent.click(screen.getByRole('button', { name: 'Confirm action' })); await screen.findByRole('button', { name: 'Check original command' })
  await act(async () => { await router.navigate(`/admin/users/${memberIds[1]}`) }); await screen.findByRole('heading', { name: 'Review Review member 2' })
  await act(async () => { await router.navigate('/admin/content') }); await screen.findByRole('heading', { name: 'Content review' })
  await act(async () => { await router.navigate(`/admin/users/${memberIds[0]}`) }); await screen.findByRole('button', { name: 'Check original command' })
  expect(screen.getByLabelText('Action')).toBeDisabled(); expect(h.server.writes()).toHaveLength(1)
  await act(async () => { await router.navigate('/admin/users/bad-id') }); await screen.findByRole('heading', { name: 'Review unavailable' })
  await waitFor(() => expect(screen.queryByText('Review member 1')).toBeNull())
})
