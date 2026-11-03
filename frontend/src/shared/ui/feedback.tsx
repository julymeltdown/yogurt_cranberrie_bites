import { useId, useLayoutEffect, useRef, type ReactNode } from 'react'
import { Button } from './controls'

export interface FeedbackProps {
  kind: 'status' | 'error' | 'empty'
  title: string
  children?: ReactNode
  action?: ReactNode
}

export function Feedback({ kind, title, children, action }: FeedbackProps) {
  const id = useId()
  return (
    <section className={`mcr-feedback mcr-feedback--${kind}`} aria-labelledby={id}
      role={kind === 'error' ? 'alert' : kind === 'status' ? 'status' : undefined}>
      <h2 id={id}>{title}</h2>
      {children && <div className="mcr-feedback__body">{children}</div>}
      {action && <div className="mcr-feedback__action">{action}</div>}
    </section>
  )
}

export function Skeleton({ label = 'Loading', lines = 3 }: { label?: string; lines?: number }) {
  const count = Number.isFinite(lines) ? Math.min(8, Math.max(1, Math.floor(lines))) : 3
  return (
    <div className="mcr-skeleton" role="status">
      <span className="mcr-visually-hidden">{label}</span>
      <div aria-hidden="true">
        {Array.from({ length: count }, (_, index) => <div key={index} className="mcr-skeleton__line" />)}
      </div>
    </div>
  )
}

export interface ToastMessage {
  id: string
  message: string
  tone?: 'info' | 'error'
}
export interface ToastRegionProps {
  messages: readonly ToastMessage[]
  onDismiss: (id: string) => void
  label?: string
}

export function ToastRegion({ messages, onDismiss, label = 'Notifications' }: ToastRegionProps) {
  const region = useRef<HTMLDivElement>(null)
  const buttons = useRef(new Map<string, HTMLButtonElement>())
  const pending = useRef<{
    removed: string
    next: string | undefined
    previous: string | undefined
    button: HTMLButtonElement
  } | null>(null)
  useLayoutEffect(() => {
    const target = pending.current
    if (!target || messages.some(message => message.id === target.removed)) return
    pending.current = null
    // A deferred dismiss must not pull focus away from work started elsewhere.
    if (document.activeElement !== document.body && document.activeElement !== target.button) return
    const button = (target.next ? buttons.current.get(target.next) : undefined)
      ?? (target.previous ? buttons.current.get(target.previous) : undefined)
    ;(button ?? region.current)?.focus({ preventScroll: true })
  }, [messages])

  return (
    <div className="mcr-toasts" role="region" aria-label={label} ref={region} tabIndex={-1}>
      {(['info', 'error'] as const).map(tone => (
        <ol key={tone} className="mcr-toast-list" aria-live={tone === 'error' ? 'assertive' : 'polite'}
          aria-relevant="additions text" aria-atomic="false" aria-label={tone === 'error' ? 'Errors' : 'Updates'}>
          {messages.filter(message => (message.tone ?? 'info') === tone).map(message => (
            <li className={`mcr-toast mcr-toast--${tone}`} key={message.id}>
              <p>{message.message}</p>
              <Button variant="quiet" aria-label={`Dismiss notification: ${message.message}`}
                ref={element => {
                  if (element) buttons.current.set(message.id, element)
                  else buttons.current.delete(message.id)
                }}
                onClick={event => {
                  if (document.activeElement === event.currentTarget) {
                    const ordered = [...messages.filter(item => item.tone !== 'error'),
                      ...messages.filter(item => item.tone === 'error')]
                    const index = ordered.findIndex(item => item.id === message.id)
                    pending.current = { removed: message.id, next: ordered[index + 1]?.id,
                      previous: ordered[index - 1]?.id, button: event.currentTarget }
                  }
                  onDismiss(message.id)
                }}>Dismiss</Button>
            </li>
          ))}
        </ol>
      ))}
    </div>
  )
}
