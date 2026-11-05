import { useRef } from 'react'
import { ShellLink } from '../navigation'
import { ConcertIcon } from '../../shared/ui/concert-window'

/** State belongs to the persistent shell so Back can restore an originating link. */
export function WorldTools({ open, onOpenChange }: {
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
}) {
  const toggle = useRef<HTMLButtonElement>(null)
  return <main id="shell-content" className="shell-world-tools" tabIndex={-1}
    onKeyDown={event => {
      if (event.key !== 'Escape' || !open) return
      event.preventDefault()
      event.stopPropagation()
      onOpenChange(false)
      toggle.current?.focus()
    }}>
    <h1 className="mcr-visually-hidden">World</h1>
    <button ref={toggle} type="button" className="world-tools-toggle" aria-label="World tools"
      aria-expanded={open} aria-controls="world-tools-destinations" onClick={() => onOpenChange(!open)}>
      <ConcertIcon name="world" /><span>Tools</span>
    </button>
    <nav id="world-tools-destinations" aria-label="World tools" hidden={!open}>
      <ShellLink id="world-map" to="/world/map"><ConcertIcon name="map" /><span>Map</span></ShellLink>
      <ShellLink id="world-check-in" to="/world/check-in"><ConcertIcon name="meet" /><span>Check-in</span></ShellLink>
      <ShellLink id="world-rooms" to="/world/rooms"><ConcertIcon name="world" /><span>Rooms</span></ShellLink>
      <ShellLink id="world-nearby" to="/world/nearby"><ConcertIcon name="meet" /><span>Nearby</span></ShellLink>
      <ShellLink id="world-help" to="/world/help"><ConcertIcon name="help" /><span>Help</span></ShellLink>
    </nav>
  </main>
}
