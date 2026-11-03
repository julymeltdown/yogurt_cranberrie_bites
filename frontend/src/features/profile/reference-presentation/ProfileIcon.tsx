export type ProfileIconName = 'edit' | 'link' | 'report' | 'block' | 'settings'
export function ProfileIcon({ name }: { name: ProfileIconName }) {
  return <svg className="reference-profile-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true" focusable="false">
    {name === 'edit' ? <path d="m4 17 1 4 4-1L21 8l-5-5L4 17Zm9-11 5 5M4 21h16" />
      : name === 'link' ? <path d="m10 7 3-3a5 5 0 0 1 7 7l-4 4M14 17l-3 3a5 5 0 0 1-7-7l4-4m0 7 8-8" />
        : name === 'report' ? <path d="M5 22V3h14l-3 5 3 5H5" />
          : name === 'block' ? <><circle cx="12" cy="12" r="9" /><path d="m6 6 12 12" /></>
            : <><path d="m9 3-1 3-3 1 1 3-2 2 2 2-1 3 3 1 1 3h6l1-3 3-1-1-3 2-2-2-2 1-3-3-1-1-3Z" /><circle cx="12" cy="12" r="3" /></>}
  </svg>
}
