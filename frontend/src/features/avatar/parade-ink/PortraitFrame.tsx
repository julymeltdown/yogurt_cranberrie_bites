import type { ReactNode } from 'react'
import './parade-ink-avatar.css'

/** A presentation frame. The supplied child keeps its renderer, pixels and lifetime. */
export function PortraitFrame({ children, label, size = 64, fitContainer = false, className = '' }: {
  readonly children: ReactNode
  readonly label: string
  readonly size?: 64 | 128
  readonly fitContainer?: boolean
  readonly className?: string
}) {
  return <span className={`pi-portrait-frame${fitContainer ? ' pi-portrait-frame--fit' : ''} ${className}`}
    data-portrait-size={size} role="group" aria-label={label}>{children}</span>
}
