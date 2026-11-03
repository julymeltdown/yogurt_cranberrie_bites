import { createContext, useContext, useId, useLayoutEffect, useRef, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { Button } from './controls'
import { addModal, canFocus, tabStops, topModal, type ModalLayer } from './modal-focus'

const ParentDialog = createContext<string | null>(null)

export type DialogCloseReason = 'escape' | 'backdrop' | 'close-button'
export interface DialogProps {
  open: boolean
  onClose: (reason: DialogCloseReason) => void
  title: ReactNode
  description?: string
  children: ReactNode
  presentation?: 'dialog' | 'sheet'
  dismissOnBackdrop?: boolean
  initialFocusRef?: RefObject<HTMLElement | null>
  returnFocusRef?: RefObject<HTMLElement | null>
  closeLabel?: string
}

export function Dialog({
  open, onClose, title, description, children, presentation = 'dialog',
  dismissOnBackdrop = true, initialFocusRef, returnFocusRef, closeLabel = 'Close dialog',
}: DialogProps) {
  const id = useId()
  const parentId = useContext(ParentDialog)
  const panelRef = useRef<HTMLDivElement>(null)
  const overlayRef = useRef<HTMLDivElement>(null)
  const onCloseRef = useRef(onClose)
  const backdropPress = useRef(false)
  useLayoutEffect(() => { onCloseRef.current = onClose })

  useLayoutEffect(() => {
    const panel = panelRef.current
    const overlay = overlayRef.current
    if (!open || !panel || !overlay) return
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    let lastFocus: HTMLElement | null = null
    const focus = () => {
      const initial = initialFocusRef?.current
      const target = lastFocus && canFocus(lastFocus, panel) ? lastFocus
        : initial && canFocus(initial, panel) ? initial : tabStops(panel)[0] ?? panel
      target.focus({ preventScroll: true })
    }
    const layer: ModalLayer = { id, parentId, panel, overlay, focus,
      returnTarget: returnFocusRef?.current ?? previousFocus }
    const remove = addModal(layer)
    const containFocus = (event: FocusEvent) => {
      if (topModal() !== layer) return
      if (event.target instanceof HTMLElement && panel.contains(event.target)) lastFocus = event.target
      else focus()
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (topModal() !== layer || event.defaultPrevented || event.isComposing) return
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        onCloseRef.current('escape')
      } else if (event.key === 'Tab') {
        const stops = tabStops(panel)
        const index = stops.findIndex(element => element === document.activeElement)
        const next = event.shiftKey ? (index <= 0 ? stops.length - 1 : index - 1)
          : (index + 1) % stops.length
        event.preventDefault()
        ;(stops[next] ?? panel).focus()
      }
    }
    document.addEventListener('keydown', onKeyDown)
    document.addEventListener('focusin', containFocus)
    if (topModal() === layer) focus()
    return () => {
      const wasTop = topModal() === layer
      document.removeEventListener('keydown', onKeyDown)
      document.removeEventListener('focusin', containFocus)
      remove()
      if (!wasTop) return
      const target = returnFocusRef?.current ?? layer.returnTarget
      const top = topModal()
      if (target?.isConnected && !target.closest('[inert]') && (!top || top.panel.contains(target))) {
        target.focus({ preventScroll: true })
      } else top?.focus()
    }
  }, [open, initialFocusRef, returnFocusRef, id, parentId])

  if (!open) return null
  return createPortal(
    <div className={`mcr-overlay mcr-overlay--${presentation}`} ref={overlayRef}
      onPointerDown={event => { backdropPress.current = event.target === event.currentTarget }}
      onPointerCancel={() => { backdropPress.current = false }}
      onClick={event => {
        if (dismissOnBackdrop && backdropPress.current && event.target === event.currentTarget
          && topModal()?.overlay === overlayRef.current) onClose('backdrop')
        backdropPress.current = false
      }}>
      <ParentDialog value={id}><div className={`mcr-dialog mcr-dialog--${presentation}`} ref={panelRef} role="dialog"
        aria-modal="true" aria-labelledby={`${id}-title`}
        aria-describedby={description ? `${id}-description` : undefined} tabIndex={-1}>
        <header className="mcr-dialog__header">
          <h2 id={`${id}-title`}>{title}</h2>
          <Button variant="quiet" aria-label={closeLabel} onClick={() => onClose('close-button')}>Close</Button>
        </header>
        <div className="mcr-dialog__body">
          {description && <p className="mcr-dialog__description" id={`${id}-description`}>{description}</p>}
          {children}
        </div>
      </div></ParentDialog>
    </div>, document.body,
  )
}
