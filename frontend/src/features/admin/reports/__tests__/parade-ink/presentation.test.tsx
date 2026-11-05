// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom/vitest'
import { ModerationReview } from '../../ui/ModerationReview'
import { setup, owners } from '../../../../../app/operations-composition/__tests__/reports-production/harness'
import { id } from '../../../../../app/operations-composition/__tests__/reports-production/server'

afterEach(() => { cleanup(); owners.splice(0).forEach(owner => owner.dispose()) })
async function open(empty = false) {
  const h = setup()
  if (empty) h.server.reports.list = { items: [], next_cursor: null }
  await h.entry.start()
  const controller = h.entry.read().reports!.borrow()!
  render(<ModerationReview controller={controller}/> )
  if (!empty) {
    fireEvent.click(await screen.findByRole('button', { name: `Review ${id(301)}` }))
    await screen.findByText('Private test statement')
  }
  return { ...h, controller }
}
function prepare() {
  fireEvent.change(screen.getByLabelText('Action'), { target: { value: 'RESOLVED' } })
  fireEvent.change(screen.getByLabelText('Reason for this action'), { target: { value: 'Reviewed the factual report.' } })
  fireEvent.click(screen.getByRole('button', { name: 'Review action' }))
}
it('renders actual retained report data and focuses the selected heading without reporter identity', async () => {
  await open()
  expect(screen.getByRole('main', { name: 'Report review' })).toHaveClass('reports-ink')
  await waitFor(() => expect(screen.getByRole('heading', { name: 'Review POST report' })).toHaveFocus())
  expect(screen.getByRole('button', { name: `Review ${id(301)}` })).toHaveAttribute('aria-pressed', 'true')
  expect(document.body.textContent).not.toContain(id(999))
  expect(screen.getByRole('textbox', { name: 'Reason for this action' })).toHaveAttribute('aria-describedby', 'reports-reason-hint')
})
it('confirmation and cancellation preserve draft and owner focus without a command', async () => {
  const h = await open(); prepare()
  await waitFor(() => expect(screen.getByRole('button', { name: 'Confirm action' })).toHaveFocus())
  fireEvent.click(screen.getByRole('button', { name: 'Cancel confirmation' }))
  expect(screen.getByRole('button', { name: 'Review action' })).toHaveFocus()
  expect(screen.getByLabelText('Reason for this action')).toHaveValue('Reviewed the factual report.')
  expect(h.server.requests.filter(request => request.method === 'POST')).toHaveLength(0)
})
it('shows evidence unavailability without replacing the current detail or dispatching bytes', async () => {
  const h = await open(), before = h.server.requests.length
  fireEvent.click(screen.getByRole('button', { name: 'Open private evidence 1' }))
  await screen.findByText('Evidence is unavailable. No public fallback is used.')
  expect(screen.getByText('Private test statement')).toBeVisible()
  expect(h.server.requests).toHaveLength(before)
  expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
it('keeps uncertain actions disabled and the same original command check available', async () => {
  const h = await open(); h.server.reports.mode = 'pending'; prepare()
  fireEvent.click(screen.getByRole('button', { name: 'Confirm action' }))
  await screen.findByRole('button', { name: 'Check original command' })
  h.server.reports.statusHttp = 404
  fireEvent.click(screen.getByRole('button', { name: 'Check original command' }))
  await screen.findByText(/Its outcome remains unknown/)
  expect(screen.getByRole('button', { name: 'Refresh records' })).toBeDisabled()
  expect(screen.getByLabelText('Reason for this action')).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Review action' })).toBeDisabled()
  expect(h.server.requests.filter(request => request.method === 'POST')).toHaveLength(1)
})
it.each(['conflict', 'failure'] as const)('renders %s with refresh and no editable stale draft', async kind => {
  const h = await open(); h.server.reports.mode = kind === 'conflict' ? 'conflict' : 'pending'; prepare()
  fireEvent.click(screen.getByRole('button', { name: 'Confirm action' }))
  if (kind === 'failure') {
    await screen.findByRole('button', { name: 'Check original command' })
    h.server.reports.status = 'FAILED'
    fireEvent.click(screen.getByRole('button', { name: 'Check original command' }))
  }
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent(kind === 'conflict' ? 'record changed' : 'server recorded failure'))
  expect(screen.getByLabelText('Action')).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Refresh records' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: 'Refresh records' }))
  await waitFor(() => expect(screen.getByRole('button', { name: `Review ${id(301)}` })).toBeEnabled())
  expect(screen.queryByLabelText('Action')).not.toBeInTheDocument()
})
it('uses the server-confirmed terminal projection and retires the action form', async () => {
  await open(); prepare(); fireEvent.click(screen.getByRole('button', { name: 'Confirm action' }))
  await screen.findByText('The server confirmed this action.')
  expect(screen.queryByRole('button', { name: 'Review action' })).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Refresh records' })).toBeEnabled()
  expect(screen.queryByRole('button', { name: 'Check original command' })).not.toBeInTheDocument()
})
it('renders the actual empty response, then scrubs private presentation on authority withdrawal', async () => {
  const h = await open(true)
  await screen.findByRole('heading', { name: 'No reports on this page' })
  await act(async () => { h.server.state.role = null; h.server.replaceDecision(); await h.entry.refresh() })
  expect(screen.getByRole('heading', { name: 'Moderator access unavailable' })).toBeVisible()
  expect(screen.queryByRole('heading', { name: 'Records on this page' })).not.toBeInTheDocument()
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
})
