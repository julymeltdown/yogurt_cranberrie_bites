// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { act } from 'react'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { ReportDialog } from '../../panels'
import { ReportPanel } from '../../../../app/safety-entry/panels'
import { SafetyOwnership } from '../../../../app/safety-entry/context'
import { PortsProvider } from '../../../../app/access'
import { createFixturePorts } from '../../../../app/__tests__/fixtures/ports'
import { reportFixture } from './fixture'

const owners: Awaited<ReturnType<typeof reportFixture>>[] = []
async function own(mode?: Parameters<typeof reportFixture>[0]) { const h = await reportFixture(mode); owners.push(h); return h }
afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()) })

it('retains the native report name, initial close focus and draft dismissal without a write', async () => {
  const h = await own(), dismissed = vi.fn()
  render(<ReportDialog controller={h.controller} search={h.search} onDismiss={dismissed} />)
  await screen.findByRole('textbox', { name: /What happened/ })
  expect(screen.getByRole('dialog', { name: 'Report a concern' })).toBeDefined()
  expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Close report' }))
  fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Private unsent draft' } })
  fireEvent.click(screen.getByRole('button', { name: 'Close report' }))
  expect(dismissed).toHaveBeenCalledTimes(1)
  expect(h.controller.read().text).toBe('')
  expect(h.requests.filter(r => r.method === 'POST' && new URL(r.url).pathname.endsWith('/reports'))).toHaveLength(0)
})

it('keeps a pending receipt uncertain and checks its original key without replaying submission', async () => {
  const h = await own('pending')
  render(<ReportDialog controller={h.controller} search={h.search} onDismiss={() => {}} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Submit report' }))
  await screen.findByRole('button', { name: 'Check original request status' })
  expect(screen.queryByRole('button', { name: 'Submit report' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Check original request status' }))
  await waitFor(() => expect(h.requests.some(r => new URL(r.url).pathname.endsWith('/commands'))).toBe(true))
  expect(h.requests.filter(r => r.method === 'POST' && new URL(r.url).pathname.endsWith('/reports'))).toHaveLength(1)
  expect(h.controller.read().phase).toBe('pending')
  expect(screen.queryByText(/Report submitted\./)).toBeNull()
})

it('withdraws the private draft and submission controls when canonical privacy changes', async () => {
  const h = await own()
  render(<ReportDialog controller={h.controller} search={h.search} onDismiss={() => {}} />)
  fireEvent.change(await screen.findByRole('textbox'), { target: { value: 'PRIVATE_DRAFT' } })
  await act(async () => { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block') })
  expect(screen.queryByRole('textbox')).toBeNull()
  expect(screen.queryByRole('button', { name: 'Submit report' })).toBeNull()
  expect(document.body.textContent).not.toContain('PRIVATE_DRAFT')
})

it('renders truthful unavailable ReportPanel through retained SafetyOwnership without a fabricated receipt', async () => {
  const h = await own('unavailable'), base = createFixturePorts()
  render(<MemoryRouter initialEntries={['/report' + h.search]}><PortsProvider ports={{ ...base.ports, auth: h.auth, session: h.auth.session }}>
    <SafetyOwnership><ReportPanel /></SafetyOwnership>
  </PortsProvider></MemoryRouter>)
  await screen.findByText('This safety service is unavailable. No action has been sent.')
  expect(screen.queryByRole('button', { name: 'Submit report' })).toBeNull()
  expect(document.body.textContent).not.toContain('PRIVATE_HTTP_DETAIL')
  expect(h.requests.findIndex(r => new URL(r.url).pathname === '/api/auth/enrollment/admission')).toBeLessThan(h.requests.findIndex(r => new URL(r.url).pathname.endsWith('/me/reports')))
})
