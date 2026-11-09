/**
 * WP-10 round 2: the shared icon set for profile/social screens.
 *
 * The reference target uses a small, consistent set of line icons at one weight.
 * The new screens previously used either no icons at all or a text glyph
 * (`\u2665` for a heart), which is why they read as unfinished next to the
 * reference.
 *
 * Conventions, matching the two icon sets already in the product
 * (`shared/ui/concert-window` and `features/profile/reference-presentation`):
 *
 *  - a 24x24 viewBox, `stroke="currentColor"`, `strokeWidth="2"`;
 *  - `fill="none"` with square caps and mitre joins, so the set reads as one
 *    family rather than a mix of round and square terminals;
 *  - `aria-hidden="true"` and `focusable="false"`: an icon is decorative. The
 *    control that contains it supplies the accessible name.
 *
 * An icon is never the only carrier of meaning: every place one is used also has
 * a text label or an accessible name, so a colour-blind or screen-reader user
 * loses nothing.
 */
import type { ReactNode } from 'react'

export type SocialIconName =
  | 'heart'
  | 'heart-filled'
  | 'comment'
  | 'share'
  | 'people'
  | 'person'
  | 'photo'
  | 'calendar'
  | 'location'
  | 'lock'
  | 'chevron-right'
  | 'edit'
  | 'plus'
  | 'warning'

const paths: Record<SocialIconName, ReactNode> = {
  // An outline heart; the filled variant is the pressed state.
  heart: <path d="M12 20.5 4.2 12.9a4.9 4.9 0 0 1 0-7 4.9 4.9 0 0 1 7 0l.8.8.8-.8a4.9 4.9 0 0 1 7 0 4.9 4.9 0 0 1 0 7Z" />,
  'heart-filled': <path d="M12 20.5 4.2 12.9a4.9 4.9 0 0 1 0-7 4.9 4.9 0 0 1 7 0l.8.8.8-.8a4.9 4.9 0 0 1 7 0 4.9 4.9 0 0 1 0 7Z" fill="currentColor" />,
  comment: <path d="M4 4h16v12H9l-5 4V4Z" />,
  share: <><path d="M4 12v8h16v-8" /><path d="M12 16V3M8 7l4-4 4 4" /></>,
  people: <><circle cx="8" cy="8" r="3" /><circle cx="17" cy="9" r="2" /><path d="M2 20v-3a6 6 0 0 1 12 0v3M16 14a5 5 0 0 1 6 5v1" /></>,
  person: <><circle cx="12" cy="8" r="4" /><path d="M4 22v-3a8 8 0 0 1 16 0v3Z" /></>,
  photo: <><path d="M3 4h18v16H3z" /><path d="m3 16 5-5 4 4 3-3 6 6" /><circle cx="15" cy="8" r="1.5" /></>,
  calendar: <><path d="M3 5h18v16H3z" /><path d="M3 10h18M8 3v4M16 3v4" /></>,
  location: <><path d="M12 22s7-7.2 7-12a7 7 0 1 0-14 0c0 4.8 7 12 7 12Z" /><circle cx="12" cy="10" r="2.5" /></>,
  lock: <><path d="M5 11h14v10H5z" /><path d="M8 11V7a4 4 0 0 1 8 0v4" /></>,
  'chevron-right': <path d="m9 5 7 7-7 7" />,
  edit: <path d="m4 17 1 4 4-1L21 8l-5-5L4 17Zm9-11 5 5" />,
  plus: <path d="M12 5v14M5 12h14" />,
  warning: <><path d="M12 3 2 21h20L12 3Z" /><path d="M12 9v6M12 18v.5" /></>,
}

export function SocialIcon({ name, className = '' }: { name: SocialIconName; className?: string }) {
  return (
    <svg
      className={className === '' ? 'pi-icon' : `pi-icon ${className}`}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="square"
      strokeLinejoin="miter"
      aria-hidden="true"
      focusable="false"
    >
      {paths[name]}
    </svg>
  )
}
