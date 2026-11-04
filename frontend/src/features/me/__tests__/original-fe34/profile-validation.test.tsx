// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { ProfileForm } from '../../ui/MePages'
import { participant, setup } from '../fixtures'

afterEach(cleanup)

describe('original FE34 Profile owner validation', () => {
  it.each([
    { patch: { display_name: '   ' }, message: 'non-space' },
    { patch: { display_name: '\u2003\u202f\u3000' }, message: 'non-space' },
    { patch: { display_name: 'Name\twith tab' }, message: 'display name' },
    { patch: { display_name: 'Name\u007f' }, message: 'display name' },
    { patch: { display_name: 'Name\u009f' }, message: 'display name' },
    { patch: { bio: 'Two\nlines' }, message: 'bio' },
    { patch: { bio: 'Bio\u0000' }, message: 'bio' },
    { patch: { bio: 'Bio\u0085' }, message: 'bio' },
  ])('keeps invalid draft local without attempting a PATCH: $patch', async ({ patch, message }) => {
    const fetcher = vi.fn(async () => Response.json(participant()))
    const { controller, dispose } = setup(fetcher)
    try {
      await controller.load()
      controller.editProfile(patch)
      await controller.save('profile')
      expect(fetcher).toHaveBeenCalledTimes(1)
      expect(controller.read().phase).toBe('ready')
      expect(controller.read().blocked).toBe(false)
      expect(controller.read().message).toContain(message)
      expect(controller.read().draft).toMatchObject(patch)
      expect(controller.dirty('profile')).toBe(true)
      controller.cancel('profile')
      expect(controller.dirty('profile')).toBe(false)
      expect(fetcher).toHaveBeenCalledTimes(1)
    } finally { dispose() }
  })

  it('explains a blank name and saves the corrected name exactly without normalization', async () => {
    const saved = participant(), writes: unknown[] = []
    const { controller, dispose } = setup(vi.fn(async input => {
      const request = input as Request
      if (request.method === 'PATCH') {
        const body = await request.json()
        writes.push(body)
        saved.profile.display_name = body.display_name
        saved.revision++
      }
      return Response.json(saved)
    }))
    try {
      await controller.load()
      render(<MemoryRouter><ProfileForm controller={controller} /></MemoryRouter>)
      fireEvent.change(screen.getByLabelText('Display name'), { target: { value: '\u3000' } })
      fireEvent.click(screen.getByRole('button', { name: 'Save profile' }))
      expect(screen.getByRole('status').textContent).toContain('non-space')
      expect(writes).toEqual([])
      const corrected = '  Listener 🎸  '
      fireEvent.change(screen.getByLabelText('Display name'), { target: { value: corrected } })
      await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Save profile' })) })
      await waitFor(() => expect(screen.getByRole('status').textContent).toBe('Profile saved.'))
      expect(writes).toEqual([{ expected_revision: 1, display_name: corrected, bio: null }])
      expect((screen.getByLabelText('Display name') as HTMLInputElement).value).toBe(corrected)
    } finally { act(dispose) }
  })

  it('explains a multiline bio and allows local cancellation without a server rollback', async () => {
    const fetcher = vi.fn(async () => Response.json(participant()))
    const { controller, dispose } = setup(fetcher)
    try {
      await controller.load()
      render(<MemoryRouter><ProfileForm controller={controller} /></MemoryRouter>)
      fireEvent.change(screen.getByLabelText('Bio (optional)'), { target: { value: 'First\nSecond' } })
      fireEvent.click(screen.getByRole('button', { name: 'Save profile' }))
      expect(screen.getByRole('status').textContent).toContain('Remove tabs, newlines and other control characters from your bio.')
      expect((screen.getByLabelText('Bio (optional)') as HTMLTextAreaElement).value).toBe('First\nSecond')
      fireEvent.click(screen.getByRole('button', { name: 'Cancel profile changes' }))
      expect((screen.getByLabelText('Bio (optional)') as HTMLTextAreaElement).value).toBe('')
      expect(fetcher).toHaveBeenCalledTimes(1)
    } finally { act(dispose) }
  })
})
