// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { useRef, useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { Button, Field, Tabs, Feedback, Skeleton, ToastRegion, type ToastMessage } from '../index'

afterEach(cleanup)

describe('form controls', () => {
  it('defaults to a non-submitting button and blocks duplicate activation while busy', () => {
    const submit = vi.fn(event => event.preventDefault())
    const click = vi.fn()
    const { rerender } = render(<form onSubmit={submit}><Button onClick={click}>Save</Button></form>)
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(click).toHaveBeenCalledOnce()
    expect(submit).not.toHaveBeenCalled()
    rerender(<Button busy onClick={click}>Save</Button>)
    const button = screen.getByRole('button', { name: 'Save' })
    expect(button).toBeDisabled()
    expect(button).toHaveAttribute('aria-busy', 'true')
    fireEvent.click(button)
    expect(click).toHaveBeenCalledOnce()
  })

  it('forwards the native ref and supports explicit submit', () => {
    const submit = vi.fn(event => event.preventDefault())
    function Form() {
      const ref = useRef<HTMLButtonElement>(null)
      return <form onSubmit={submit}>
        <Button type="submit" ref={ref}>Send</Button>
        <Button onClick={() => ref.current?.focus()}>Focus send</Button>
      </form>
    }
    render(<Form />)
    fireEvent.click(screen.getByText('Focus send'))
    expect(screen.getByText('Send')).toHaveFocus()
    fireEvent.click(screen.getByText('Send'))
    expect(submit).toHaveBeenCalledOnce()
  })

  it('associates label, required state, hint, external description and error with the input', () => {
    const { rerender } = render(<>
      <p id="extra">Kept private.</p>
      <Field label="Display name" required hint="Up to 40 characters." error="Enter a name."
        aria-describedby="extra" />
    </>)
    const input = screen.getByRole('textbox', { name: 'Display name' })
    expect(input).toBeRequired()
    expect(input).toHaveAttribute('aria-invalid', 'true')
    expect(input).toHaveAccessibleDescription('Kept private. Up to 40 characters. Enter a name.')
    expect(screen.getByRole('alert')).toHaveTextContent('Enter a name.')
    rerender(<Field label="Display name" />)
    expect(screen.getByRole('textbox')).not.toHaveAttribute('aria-invalid')
    expect(screen.getByRole('textbox')).not.toHaveAttribute('aria-describedby')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('keeps generated field IDs unique and preserves native change and disabled behavior', () => {
    const change = vi.fn()
    render(<><Field label="First" onChange={change} /><Field label="Second" disabled /></>)
    const first = screen.getByRole('textbox', { name: 'First' })
    expect(first.id).not.toBe(screen.getByRole('textbox', { name: 'Second' }).id)
    fireEvent.change(first, { target: { value: 'Night' } })
    expect(change).toHaveBeenCalledOnce()
    expect(first).toHaveValue('Night')
    expect(screen.getByRole('textbox', { name: 'Second' })).toBeDisabled()
  })
})

function TabExample({ orientation = 'horizontal' }: { orientation?: 'horizontal' | 'vertical' }) {
  const [value, setValue] = useState('one')
  return <Tabs label="Plans" value={value} onValueChange={setValue} orientation={orientation} items={[
    { value: 'one', label: 'First', content: <p>First content</p> },
    { value: 'two', label: 'Unavailable', disabled: true, content: 'Unavailable content' },
    { value: 'three', label: 'Last', content: <input aria-label="Last content" /> },
  ]} />
}

describe('tabs', () => {
  it('uses roving focus, wraps, skips disabled tabs and associates panels', () => {
    render(<TabExample />)
    const first = screen.getByRole('tab', { name: 'First' })
    const last = screen.getByRole('tab', { name: 'Last' })
    first.focus()
    fireEvent.keyDown(first, { key: 'ArrowRight' })
    expect(last).toHaveFocus()
    expect(last).toHaveAttribute('aria-selected', 'true')
    expect(first).toHaveAttribute('tabindex', '-1')
    const panel = screen.getByRole('tabpanel', { name: 'Last' })
    expect(last).toHaveAttribute('aria-controls', panel.id)
    expect(panel).toHaveAttribute('aria-labelledby', last.id)
    expect(screen.getByText('First content')).not.toBeVisible()
    fireEvent.keyDown(last, { key: 'ArrowRight' })
    expect(first).toHaveFocus()
    fireEvent.keyDown(first, { key: 'ArrowLeft' })
    expect(last).toHaveFocus()
    fireEvent.keyDown(last, { key: 'Home' })
    expect(first).toHaveFocus()
    fireEvent.keyDown(first, { key: 'End' })
    expect(last).toHaveFocus()
  })

  it('preserves vertical scroll keys in horizontal tabs and activates mouse clicks', () => {
    render(<TabExample />)
    const first = screen.getByRole('tab', { name: 'First' })
    expect(fireEvent.keyDown(first, { key: 'ArrowDown' })).toBe(true)
    fireEvent.click(screen.getByRole('tab', { name: 'Last' }))
    expect(screen.getByRole('tabpanel', { name: 'Last' })).toBeVisible()
  })

  it('uses Up/Down in vertical tabs and namespaces independent tab widgets', () => {
    render(<><TabExample orientation="vertical" /><TabExample /></>)
    const [vertical, horizontal] = screen.getAllByRole('tablist')
    const first = within(vertical!).getByRole('tab', { name: 'First' })
    first.focus()
    fireEvent.keyDown(first, { key: 'ArrowDown' })
    expect(within(vertical!).getByRole('tab', { name: 'Last' })).toHaveFocus()
    expect(first.id).not.toBe(within(horizontal!).getByRole('tab', { name: 'First' }).id)
  })
})

describe('feedback and notifications', () => {
  it('distinguishes live status/error from an ordinary empty state and decorative skeleton', () => {
    const { rerender } = render(<Feedback kind="status" title="Waiting for confirmation">Keep this page open.</Feedback>)
    expect(screen.getByRole('status')).toHaveAccessibleName('Waiting for confirmation')
    rerender(<Feedback kind="error" title="Could not save" action={<Button>Try again</Button>}>Your draft is here.</Feedback>)
    expect(screen.getByRole('alert')).toHaveAccessibleName('Could not save')
    expect(screen.getByRole('button', { name: 'Try again' })).toBeEnabled()
    rerender(<Feedback kind="empty" title="No plans yet" />)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    rerender(<Skeleton label="Loading plans" lines={2} />)
    expect(screen.getByRole('status')).toHaveTextContent('Loading plans')
    expect(screen.getByRole('status').querySelector('[aria-hidden="true"]')).not.toBeNull()
  })

  it('keeps live regions mounted before messages arrive and preserves focused dismissal', () => {
    function Example() {
      const [messages, setMessages] = useState<ToastMessage[]>([])
      return <>
        <Button onClick={() => setMessages([{ id: 'a', message: 'Draft retained.' },
          { id: 'b', message: 'Upload failed.', tone: 'error' }])}>Notify</Button>
        <ToastRegion messages={messages} onDismiss={id => setMessages(items => items.filter(item => item.id !== id))} />
      </>
    }
    render(<Example />)
    const region = screen.getByRole('region', { name: 'Notifications' })
    const polite = within(region).getByRole('list', { name: 'Updates' })
    const urgent = within(region).getByRole('list', { name: 'Errors' })
    expect(polite).toHaveAttribute('aria-live', 'polite')
    expect(urgent).toHaveAttribute('aria-live', 'assertive')
    fireEvent.click(screen.getByText('Notify'))
    expect(within(region).getByRole('list', { name: 'Updates' })).toBe(polite)
    const dismiss = screen.getByRole('button', { name: 'Dismiss notification: Draft retained.' })
    dismiss.focus()
    fireEvent.click(dismiss)
    const last = screen.getByRole('button', { name: 'Dismiss notification: Upload failed.' })
    expect(last).toHaveFocus()
    fireEvent.click(last)
    expect(region).toHaveFocus()
    expect(within(region).queryByRole('button')).not.toBeInTheDocument()
  })

  it('does not steal focus on notification insertion or off-focus dismissal', () => {
    const dismiss = vi.fn()
    const { rerender } = render(<><input aria-label="Draft" /><ToastRegion messages={[]} onDismiss={dismiss} /></>)
    const input = screen.getByRole('textbox')
    input.focus()
    rerender(<><input aria-label="Draft" /><ToastRegion messages={[{ id: '1', message: 'Saved.' }]} onDismiss={dismiss} /></>)
    expect(input).toHaveFocus()
    fireEvent.click(screen.getByRole('button'))
    rerender(<><input aria-label="Draft" /><ToastRegion messages={[]} onDismiss={dismiss} /></>)
    expect(input).toHaveFocus()
  })

  it('does not steal focus when controlled dismissal is delayed', () => {
    const messages = [{ id: 'a', message: 'Draft retained.' }]
    const dismiss = vi.fn()
    const { rerender } = render(<><input aria-label="Draft" /><ToastRegion messages={messages} onDismiss={dismiss} /></>)
    const button = screen.getByRole('button')
    button.focus()
    fireEvent.click(button)
    const input = screen.getByRole('textbox')
    input.focus()
    rerender(<><input aria-label="Draft" /><ToastRegion messages={[]} onDismiss={dismiss} /></>)
    expect(input).toHaveFocus()
  })
})
