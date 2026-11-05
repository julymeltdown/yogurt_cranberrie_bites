import type { ReactNode } from 'react'

const paths: Record<'close' | 'back' | 'expand' | 'reduce' | 'stars', ReactNode> = {
  close: <path d="m6 6 12 12M18 6 6 18" />,
  back: <path d="m7 5 7 7-7 7" />,
  expand: <path d="M5 10V5h5M14 5h5v5M19 14v5h-5M10 19H5v-5" />,
  reduce: <path d="M5 12h14" />,
  stars: <path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-6 3 1.1-6.2L3 9.6l6.2-.9Z" />,
}

/** PI01 decorative ink. The incumbent native control owns behavior and name. */
export function WindowInk({ name, className = '' }: { name: keyof typeof paths; className?: string }) {
  return <svg className={`mcr-reference-window-ink${name === 'stars' ? ' mcr-reference-window-ink--stars' : ''} ${className}`}
    viewBox="0 0 24 24" width={24} height={24} fill="none" stroke="currentColor" strokeWidth={1.6}
    strokeLinecap="square" strokeLinejoin="miter" aria-hidden="true" focusable="false">{paths[name]}</svg>
}
