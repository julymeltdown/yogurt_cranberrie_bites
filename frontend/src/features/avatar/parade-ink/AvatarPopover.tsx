import { useId, type ReactNode, type Ref } from 'react'
import './parade-ink-avatar.css'

/** The caller supplies current authorized data/actions and owns selection and withdrawal. */
export function AvatarPopover({ displayName, portrait, profileAction, onClose, closeButtonRef }: {
  readonly displayName: string
  readonly portrait: ReactNode
  readonly profileAction: ReactNode
  readonly onClose: () => void
  readonly closeButtonRef?: Ref<HTMLButtonElement>
}) {
  const id = useId()
  return <section className="pi-avatar-popover" role="dialog" aria-modal="false" aria-labelledby={`${id}-name`}
    onKeyDown={event => { if (event.key === 'Escape' && !event.defaultPrevented) { event.preventDefault(); event.stopPropagation(); onClose() } }}>
    <div className="pi-avatar-popover__portrait">{portrait ?? <span role="status">Portrait unavailable</span>}</div>
    <div className="pi-avatar-popover__identity">
      <h2 id={`${id}-name`}>{displayName}</h2>
      <div className="pi-avatar-popover__actions">{profileAction}</div>
    </div>
    <button ref={closeButtonRef} className="pi-avatar-popover__close" type="button" aria-label="Close avatar details" onClick={onClose}>
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="m5 5 14 14M19 5 5 19" /></svg>
    </button>
  </section>
}
