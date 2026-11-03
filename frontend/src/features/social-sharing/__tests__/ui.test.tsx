// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { act, StrictMode } from 'react'
import { HandleSettings, SharingOffer, SharingRequest, SharingRequests } from '../ui/SocialSharing'
import { handleStates, id, pending, recipientId, request, setup } from './fixtures/data'
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
const disposals: (() => void)[] = []
afterEach(() => { cleanup(); disposals.splice(0).forEach(dispose => dispose()) })
function fixture(handler: (r: Request) => Response | Promise<Response>, userId = request.requester_id) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request)), f = setup(fetch, userId); disposals.push(f.dispose); return { ...f, fetch }
}
it('requires two explicit recipient choices and sends only the chosen platform', async () => {
  let data = pending(), sent: unknown
  const f = fixture(async r => {
    if (r.method === 'POST') { sent = await r.json(); data = { ...data, state: 'APPROVED', revision: 2, approved_platforms: ['INSTAGRAM'] } }
    return Response.json(data)
  }, recipientId)
  await f.controller.loadRequest(request.request_id)
  render(<SharingRequest controller={f.controller} retry={vi.fn()} />)
  const approve = screen.getByRole('button', { name: 'Approve selected platforms' }) as HTMLButtonElement
  expect(approve.disabled).toBe(true)
  fireEvent.click(screen.getByRole('checkbox', { name: 'Exchange Instagram account names' })); expect(approve.disabled).toBe(true)
  fireEvent.click(screen.getByRole('checkbox', { name: 'I explicitly consent to share my selected account names with the requester.' })); expect(approve.disabled).toBe(false)
  await act(async () => { fireEvent.click(approve) })
  expect(sent).toEqual({ expected_revision: 1, decision: 'APPROVED', approved_platforms: ['INSTAGRAM'], recipient_consents: true })
  expect(screen.getByRole('button', { name: 'Reveal Instagram' })).toBeDefined(); expect(screen.queryByLabelText('Revealed account name')).toBeNull()
})
it('makes approved reveal explicit, hides on unmount and never restores on StrictMode remount', async () => {
  const f = fixture(r => Response.json(r.url.includes('/reveal') ? { request_id: request.request_id, platform: 'INSTAGRAM', owner_id: recipientId, handle_version: 3, handle: 'synthetic_private_name', grant_revision: 1, expires_at: request.expires_at } : request))
  await f.controller.loadRequest(request.request_id)
  let view = render(<StrictMode><SharingRequest controller={f.controller} retry={vi.fn()} /></StrictMode>)
  expect(f.fetch).toHaveBeenCalledTimes(1)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Reveal Instagram' })) })
  expect(screen.getByText('@synthetic_private_name')).toBeDefined()
  view.unmount(); expect(f.controller.read().secret.data).toBeNull()
  view = render(<StrictMode><SharingRequest controller={f.controller} retry={vi.fn()} /></StrictMode>)
  expect(screen.queryByText('@synthetic_private_name')).toBeNull(); expect(f.fetch).toHaveBeenCalledTimes(2); view.unmount()
})
it('requires removal confirmation; read-only keeps safety action and logout clears private drafts', async () => {
  let configured = true, version = 2
  const f = fixture(r => { if (r.method === 'DELETE') { configured = false; version++; return Response.json({ user_id: request.requester_id, platform: 'INSTAGRAM', version, configured }) } return Response.json({ items: [{ user_id: request.requester_id, platform: 'INSTAGRAM', version, configured }, handleStates().items[1]] }) })
  await f.controller.loadHandles(); render(<HandleSettings controller={f.controller} />)
  fireEvent.change(screen.getByRole('textbox', { name: 'Replacement Instagram account name' }), { target: { value: 'synthetic_draft' } })
  fireEvent.click(screen.getByRole('button', { name: 'Remove Instagram' })); expect(f.fetch).toHaveBeenCalledTimes(1)
  fireEvent.click(screen.getByRole('button', { name: 'Keep handle' })); expect(f.fetch).toHaveBeenCalledTimes(1)
  await act(async () => { f.setAccess({ available: true, readOnly: true }) })
  expect((screen.getByRole('button', { name: 'Replace privately' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'Remove Instagram' })); await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm removal' })) })
  expect(screen.queryByRole('button', { name: 'Remove Instagram' })).toBeNull()
  await act(async () => { f.auth.session.replace(f.auth.session.read().generation, null) }); expect(document.body.textContent).not.toContain('synthetic_draft')
})
it('offer and list show metadata with no prechecked consent or automatic reveal', async () => {
  const f = fixture(r => Response.json(r.url.includes('/social-handles') ? handleStates() : r.url.includes('/sharing-offer') ? { intro_id: request.intro_id, intro_revision: 1, recipient_id: recipientId, available_handles: [{ platform: 'INSTAGRAM', version: 3 }] } : { items: [request], next_cursor: null }))
  await f.controller.loadHandles(); await f.controller.loadOffer(request.intro_id); f.controller.setDirection('OUTGOING')
  await vi.waitFor(() => expect(f.controller.read().requests.phase).toBe('ready'))
  render(<><SharingOffer controller={f.controller} retry={vi.fn()} /><SharingRequests controller={f.controller} onRequest={vi.fn()} /></>)
  expect((screen.getByRole('checkbox') as HTMLInputElement).checked).toBe(false)
  expect((screen.getByRole('button', { name: 'Send sharing request' }) as HTMLButtonElement).disabled).toBe(true)
  expect(screen.getByRole('button', { name: 'Open request' })).toBeDefined(); expect(f.fetch.mock.calls.every(([r]) => !(r as Request).url.includes('/reveal'))).toBe(true)
})
it('policy revocation scrubs a visible account name and the pending input immediately', async () => {
  const f = fixture(r => Response.json(r.url.endsWith('/social-handles') ? handleStates() : r.url.includes('/reveal') ? { request_id: request.request_id, platform: 'INSTAGRAM', owner_id: recipientId, handle_version: 3, handle: 'synthetic_private_name', grant_revision: 1, expires_at: request.expires_at } : request))
  await f.controller.loadHandles(); await f.controller.loadRequest(request.request_id)
  render(<><HandleSettings controller={f.controller} /><SharingRequest controller={f.controller} retry={vi.fn()} /></>)
  fireEvent.change(screen.getByRole('textbox', { name: 'Replacement Instagram account name' }), { target: { value: 'synthetic_draft' } })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Reveal Instagram' })) })
  expect(screen.getByText('@synthetic_private_name')).toBeDefined()
  await act(async () => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block') })
  expect(screen.queryByText('@synthetic_private_name')).toBeNull(); expect(f.controller.read().handleDrafts.INSTAGRAM).toBe('')
})
it('keeps status recovery visible on the request list after navigation from an uncertain offer', async () => {
  const f = fixture(r => r.method === 'POST' ? Promise.reject(new Error('lost')) : Response.json(r.url.endsWith('/social-handles') ? handleStates() : { intro_id: request.intro_id, intro_revision: 1, recipient_id: recipientId, available_handles: [{ platform: 'INSTAGRAM', version: 3 }] }))
  await f.controller.loadHandles(); await f.controller.loadOffer(request.intro_id); f.controller.setRequesterConsent('INSTAGRAM', true); await f.controller.createRequest()
  render(<SharingRequests controller={f.controller} onRequest={vi.fn()} />)
  expect(screen.getByRole('button', { name: 'Check change status' })).toBeDefined(); expect((screen.getByRole('combobox', { name: 'Direction' }) as HTMLSelectElement).disabled).toBe(true)
})
it('throwing earlier observer cannot prevent mounted secret cleanup on block or dispose', async () => {
  const f = fixture(r => Response.json(r.url.includes('/reveal') ? { request_id: request.request_id, platform: 'INSTAGRAM', owner_id: recipientId, handle_version: 3, handle: 'synthetic_private_name', grant_revision: 1, expires_at: request.expires_at } : request))
  await f.controller.loadRequest(request.request_id); await f.controller.revealHandle('INSTAGRAM')
  const unsubscribe = f.controller.subscribe(() => { throw new Error('observer') })
  render(<SharingRequest controller={f.controller} retry={vi.fn()} />)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Reveal Instagram' })) }); expect(screen.getByText('@synthetic_private_name')).toBeDefined()
  await act(async () => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block') }); expect(screen.queryByText('@synthetic_private_name')).toBeNull()
  await act(async () => { await f.controller.loadRequest(request.request_id); await f.controller.revealHandle('INSTAGRAM') }); expect(screen.getByText('@synthetic_private_name')).toBeDefined()
  await act(async () => { f.controller.dispose() }); expect(screen.queryByText('@synthetic_private_name')).toBeNull(); unsubscribe()
})
it.each(['account', 'policy', 'version', 'access'] as const)('handle removal confirmation is discarded after %s changes', async change => {
  let version = 2
  const f = fixture(() => Response.json({ items: handleStates(f.auth.session.read().userId ?? request.requester_id).items.map(h => ({ ...h, ...(h.platform === 'INSTAGRAM' ? { version } : {}) })) }))
  await f.controller.loadHandles(); render(<HandleSettings controller={f.controller} />); fireEvent.click(screen.getByRole('button', { name: 'Remove Instagram' })); expect(screen.getByRole('button', { name: 'Confirm removal' })).toBeDefined()
  await act(async () => { if (change === 'account') f.auth.session.replace(f.auth.session.read().generation, { userId: id(99), eventId: request.event_id, accessToken: 'new-user-token' }); if (change === 'policy') f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block'); if (change === 'version') version++; if (change === 'access') { f.setAccess({ available: false, readOnly: true }); f.setAccess({ available: true, readOnly: false }) } await f.controller.loadHandles() })
  expect(screen.queryByRole('button', { name: 'Confirm removal' })).toBeNull()
})
it('revoke confirmation is discarded when the same request is reloaded after policy invalidation', async () => {
  const f = fixture(() => Response.json(request)); await f.controller.loadRequest(request.request_id); render(<SharingRequest controller={f.controller} retry={vi.fn()} />)
  fireEvent.click(screen.getByRole('button', { name: 'Revoke sharing' })); expect(screen.getByRole('button', { name: 'Confirm revocation' })).toBeDefined()
  await act(async () => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block'); await f.controller.loadRequest(request.request_id) }); expect(screen.queryByRole('button', { name: 'Confirm revocation' })).toBeNull()
})
