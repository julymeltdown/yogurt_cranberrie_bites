// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { createRef, useState } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { PortraitFrame } from '../../parade-ink/PortraitFrame'
import { AvatarPopover } from '../../parade-ink/AvatarPopover'
import { AvatarCategoryTabs } from '../../parade-ink/AvatarCategoryTabs'

afterEach(cleanup)

it('retains the supplied portrait node across frame rerenders and removes it on withdrawal', () => {
  const { container, rerender } = render(<PortraitFrame label="Current portrait" size={128}><canvas data-testid="renderer" width={64} height={64} /></PortraitFrame>)
  const canvas = screen.getByTestId('renderer')
  rerender(<PortraitFrame label="Updated portrait" size={128}><canvas data-testid="renderer" width={64} height={64} /></PortraitFrame>)
  expect(screen.getByTestId('renderer')).toBe(canvas)
  expect(canvas).toHaveAttribute('width', '64')
  expect(screen.getByRole('group', { name: 'Updated portrait' })).toContainElement(canvas)
  rerender(<PortraitFrame label="Unavailable portrait" size={128}><span role="status">Character unavailable</span></PortraitFrame>)
  expect(container.querySelector('canvas')).toBeNull()
  expect(screen.getByRole('status')).toHaveTextContent('Character unavailable')
})

it('provides only the supplied profile action, an accessible name and caller-owned close focus', () => {
  const close = vi.fn(), ref = createRef<HTMLButtonElement>()
  render(<AvatarPopover displayName="A very long current participant name" portrait={<span>Authorized portrait</span>}
    profileAction={<a href="/people/current-user">Profile</a>} onClose={close} closeButtonRef={ref} />)
  const dialog = screen.getByRole('dialog', { name: 'A very long current participant name' })
  expect(within(dialog).getAllByRole('link')).toHaveLength(1)
  expect(screen.getByRole('link', { name: 'Profile' })).toHaveAttribute('href', '/people/current-user')
  ref.current!.focus(); expect(screen.getByRole('button', { name: 'Close avatar details' })).toHaveFocus()
  fireEvent.keyDown(dialog, { key: 'Escape' }); expect(close).toHaveBeenCalledOnce()
  fireEvent.click(ref.current!); expect(close).toHaveBeenCalledTimes(2)
})

it('shows an honest missing portrait and immediately drops withdrawn participant data', () => {
  const { rerender } = render(<AvatarPopover displayName="Participant" portrait={null} profileAction={null} onClose={() => {}} />)
  expect(screen.getByRole('status')).toHaveTextContent('Portrait unavailable')
  rerender(<></>); expect(screen.queryByRole('dialog')).toBeNull(); expect(screen.queryByText('Participant')).toBeNull()
})

it('reaches every category using horizontal keys with one selected tab and linked visible panel', () => {
  const names = ['Hair', 'Eyes', 'Outfit', 'Body', 'Accessory', 'Tones', 'Presets']
  function Tabs() {
    const [value, setValue] = useState('hair')
    return <AvatarCategoryTabs items={names.map(name => ({ value: name.toLowerCase(), label: name, content: <span>{name} choices</span> }))} value={value} onChange={setValue} />
  }
  render(<Tabs />)
  screen.getByRole('tab', { name: 'Hair' }).focus()
  for (const [index, name] of names.entries()) {
    const tab = screen.getByRole('tab', { name })
    expect(tab).toHaveFocus(); expect(tab).toHaveAttribute('aria-selected', 'true')
    const panel = screen.getByRole('tabpanel', { name })
    expect(tab).toHaveAttribute('aria-controls', panel.id)
    expect(screen.getAllByRole('tabpanel')).toHaveLength(1)
    expect(screen.getAllByRole('tab').filter(node => node.tabIndex === 0)).toHaveLength(1)
    if (index < names.length - 1) fireEvent.keyDown(tab, { key: 'ArrowRight' })
  }
  fireEvent.keyDown(screen.getByRole('tab', { name: 'Presets' }), { key: 'ArrowRight' })
  expect(screen.getByRole('tab', { name: 'Hair' })).toHaveFocus()
  fireEvent.keyDown(screen.getByRole('tab', { name: 'Hair' }), { key: 'End' })
  expect(screen.getByRole('tab', { name: 'Presets' })).toHaveFocus()
  fireEvent.keyDown(screen.getByRole('tab', { name: 'Presets' }), { key: 'Home' })
  fireEvent.keyDown(screen.getByRole('tab', { name: 'Hair' }), { key: 'ArrowLeft' })
  expect(screen.getByRole('tab', { name: 'Presets' })).toHaveFocus()
})
