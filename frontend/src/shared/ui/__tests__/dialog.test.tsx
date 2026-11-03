// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode, useRef, useState } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { Button, Dialog, Field, type DialogCloseReason } from '../index'

afterEach(cleanup)

function Example({ onClose = () => undefined, backdrop = true }: {
  onClose?: (reason: DialogCloseReason) => void
  backdrop?: boolean
}) {
  const [open, setOpen] = useState(false)
  const [value, setValue] = useState('')
  const initial = useRef<HTMLInputElement>(null)
  return <>
    <Button onClick={() => setOpen(true)}>Open plans</Button>
    <Dialog open={open} title="Your plans" description="Review your draft." initialFocusRef={initial}
      dismissOnBackdrop={backdrop} onClose={reason => { onClose(reason); setOpen(false) }}>
      <Field label="Name" value={value} onChange={event => setValue(event.target.value)} ref={initial} />
      <button disabled>Disabled</button>
      <div hidden><button>Hidden</button></div>
      <div style={{ display: 'none' }}><button>CSS hidden</button></div>
      <Button>Save draft</Button>
    </Dialog>
  </>
}

it('moves actual DOM focus, isolates the background and traps Tab in both directions', () => {
  const { container } = render(<Example />)
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  const opener = screen.getByText('Open plans')
  opener.focus()
  fireEvent.click(opener)
  const dialog = screen.getByRole('dialog', { name: 'Your plans' })
  const input = screen.getByRole('textbox', { name: 'Name' })
  expect(dialog).toHaveAttribute('aria-modal', 'true')
  expect(dialog).toHaveAccessibleDescription('Review your draft.')
  expect(container).toHaveAttribute('inert')
  expect(document.body.style.overflow).toBe('hidden')
  expect(input).toHaveFocus()
  fireEvent.change(input, { target: { value: 'Draft' } })
  expect(input).toHaveFocus() // callback identity and content rerenders must not reset focus
  fireEvent.keyDown(input, { key: 'Tab' })
  expect(screen.getByText('Save draft')).toHaveFocus()
  fireEvent.keyDown(document.activeElement!, { key: 'Tab' })
  expect(screen.getByRole('button', { name: 'Close dialog' })).toHaveFocus()
  fireEvent.keyDown(document.activeElement!, { key: 'Tab', shiftKey: true })
  expect(screen.getByText('Save draft')).toHaveFocus()
  opener.focus()
  expect(screen.getByText('Save draft')).toHaveFocus() // programmatic focus cannot escape either
})

it('dismisses on Escape and restores the invoking element without mocking focus', () => {
  const close = vi.fn()
  render(<StrictMode><Example onClose={close} /></StrictMode>)
  const opener = screen.getByText('Open plans')
  opener.focus()
  fireEvent.click(opener)
  expect(fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Escape' })).toBe(false)
  expect(close).toHaveBeenCalledExactlyOnceWith('escape')
  expect(opener).toHaveFocus()
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  expect(document.body.style.overflow).toBe('')
})

it('requires the gesture to start and end on the backdrop, and supports explicit close', () => {
  const close = vi.fn()
  render(<Example onClose={close} />)
  fireEvent.click(screen.getByText('Open plans'))
  const panel = screen.getByRole('dialog')
  const overlay = panel.parentElement!
  fireEvent.pointerDown(panel)
  fireEvent.click(overlay)
  expect(close).not.toHaveBeenCalled()
  fireEvent.pointerDown(overlay)
  fireEvent.pointerCancel(overlay)
  fireEvent.click(overlay)
  expect(close).not.toHaveBeenCalled()
  fireEvent.pointerDown(overlay)
  fireEvent.click(overlay)
  expect(close).toHaveBeenLastCalledWith('backdrop')
  fireEvent.click(screen.getByText('Open plans'))
  fireEvent.click(screen.getByRole('button', { name: 'Close dialog' }))
  expect(close).toHaveBeenLastCalledWith('close-button')
})

it('can disable backdrop dismissal while keeping a working keyboard and close button', () => {
  const close = vi.fn()
  render(<Example backdrop={false} onClose={close} />)
  fireEvent.click(screen.getByText('Open plans'))
  const overlay = screen.getByRole('dialog').parentElement!
  fireEvent.pointerDown(overlay)
  fireEvent.click(overlay)
  expect(close).not.toHaveBeenCalled()
  fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Escape', isComposing: true })
  expect(close).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Close dialog' }))
  expect(close).toHaveBeenCalledExactlyOnceWith('close-button')
})

it('only dismisses the top modal and restores nested focus while preserving isolation', () => {
  function Nested() {
    const [outer, setOuter] = useState(false)
    const [inner, setInner] = useState(false)
    return <><Button onClick={() => setOuter(true)}>Open sheet</Button>
      <Dialog open={outer} presentation="sheet" title="Sheet" onClose={() => setOuter(false)}>
        <Button onClick={() => setInner(true)}>Open confirmation</Button>
        <Dialog open={inner} title="Confirmation" onClose={() => setInner(false)}><Button>Confirm</Button></Dialog>
      </Dialog>
    </>
  }
  const { container } = render(<Nested />)
  const opener = screen.getByText('Open sheet')
  opener.focus()
  fireEvent.click(opener)
  const nestedOpener = screen.getByText('Open confirmation')
  nestedOpener.focus()
  fireEvent.click(nestedOpener)
  expect(screen.getAllByRole('dialog', { hidden: true })).toHaveLength(2)
  fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
  expect(screen.queryByRole('dialog', { name: 'Confirmation' })).not.toBeInTheDocument()
  expect(nestedOpener).toHaveFocus()
  expect(container).toHaveAttribute('inert')
  expect(document.body.style.overflow).toBe('hidden')
  fireEvent.keyDown(nestedOpener, { key: 'Escape' })
  expect(opener).toHaveFocus()
  expect(container).not.toHaveAttribute('inert')
})

it('restores pre-existing inert/scroll state and isolates a newly added background root', async () => {
  document.body.style.overflow = 'auto'
  const existing = document.createElement('div')
  existing.setAttribute('inert', '')
  document.body.append(existing)
  const { unmount } = render(<Example />)
  fireEvent.click(screen.getByText('Open plans'))
  const added = document.createElement('div')
  document.body.append(added)
  await waitFor(() => expect(added).toHaveAttribute('inert'))
  unmount()
  expect(existing).toHaveAttribute('inert')
  expect(added).not.toHaveAttribute('inert')
  expect(document.body.style.overflow).toBe('auto')
  existing.remove()
  added.remove()
  document.body.style.overflow = ''
})

it('uses an explicit return target if the original trigger disappears', () => {
  function RemovedTrigger() {
    const [open, setOpen] = useState(false)
    const target = useRef<HTMLButtonElement>(null)
    return <>{!open && <Button onClick={() => setOpen(true)}>Open</Button>}
      <Button ref={target}>Back to plans</Button>
      <Dialog open={open} title="Plan" returnFocusRef={target} onClose={() => setOpen(false)}>
        <p>Details</p>
      </Dialog></>
  }
  render(<RemovedTrigger />)
  fireEvent.click(screen.getByText('Open'))
  fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
  expect(screen.getByText('Back to plans')).toHaveFocus()
})

it('respects a child-handled Escape and treats a radio group as one tab stop', () => {
  const close = vi.fn()
  render(<Dialog open title="Options" onClose={close}>
    <input aria-label="Editor" onKeyDown={event => { if (event.key === 'Escape') event.preventDefault() }} />
    <input type="radio" name="era" aria-label="First era" />
    <input type="radio" name="era" aria-label="Second era" defaultChecked />
    <Button>Finish</Button>
  </Dialog>)
  const editor = screen.getByRole('textbox')
  editor.focus()
  fireEvent.keyDown(editor, { key: 'Escape' })
  expect(close).not.toHaveBeenCalled()
  fireEvent.keyDown(editor, { key: 'Tab' })
  expect(screen.getByRole('radio', { name: 'Second era' })).toHaveFocus()
  fireEvent.keyDown(document.activeElement!, { key: 'Tab' })
  expect(screen.getByText('Finish')).toHaveFocus()
})

it('restores the page trigger when a parent and its child modal are removed together', () => {
  const { container } = render(<Button>Page trigger</Button>)
  const trigger = screen.getByText('Page trigger')
  trigger.focus()
  function Child() {
    const [open, setOpen] = useState(false)
    return <><Button onClick={() => setOpen(true)}>Open child</Button>
      <Dialog open={open} title="Child" onClose={() => setOpen(false)}>Child content</Dialog></>
  }
  const modal = render(<Dialog open title="Parent" onClose={() => undefined}><Child /></Dialog>)
  const child = screen.getByText('Open child')
  child.focus()
  fireEvent.click(child)
  modal.unmount()
  expect(trigger).toHaveFocus()
  expect(container).not.toHaveAttribute('inert')
})

it('recomputes the tab order after controls are removed or disabled', () => {
  function Changing() {
    const [removed, setRemoved] = useState(false)
    return <Dialog open title="Changing" onClose={() => undefined}>
      <Button onClick={() => setRemoved(true)}>Change fields</Button>
      {!removed && <Field label="Temporary" />}
      <Button disabled={removed}>Temporarily enabled</Button>
      <Button>Last action</Button>
    </Dialog>
  }
  render(<Changing />)
  const change = screen.getByText('Change fields')
  change.focus()
  fireEvent.click(change)
  fireEvent.keyDown(change, { key: 'Tab' })
  expect(screen.getByText('Last action')).toHaveFocus()
})

it('keeps the child on top when parent and child dialogs mount together', () => {
  function Together() {
    const [parent, setParent] = useState(false)
    const [child, setChild] = useState(true)
    return <><Button onClick={() => setParent(true)}>Open both</Button>
    <Dialog open={parent} title="Parent" onClose={() => setParent(false)}>
      <Button>Parent action</Button>
      <Dialog open={child} title="Child" onClose={() => setChild(false)}><Button>Child action</Button></Dialog>
    </Dialog></>
  }
  render(<StrictMode><Together /></StrictMode>)
  const trigger = screen.getByText('Open both')
  trigger.focus()
  fireEvent.click(trigger)
  const child = screen.getByRole('dialog', { name: 'Child' })
  expect(child).toContainElement(document.activeElement as HTMLElement)
  expect(child.parentElement).not.toHaveAttribute('inert')
  const parent = screen.getAllByRole('dialog', { hidden: true }).find(panel => panel !== child)!
  expect(parent.parentElement).toHaveAttribute('inert')
  fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
  expect(screen.queryByRole('dialog', { name: 'Child' })).not.toBeInTheDocument()
  expect(screen.getByRole('dialog', { name: 'Parent' })).toContainElement(document.activeElement as HTMLElement)
  fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
  expect(trigger).toHaveFocus()
})
