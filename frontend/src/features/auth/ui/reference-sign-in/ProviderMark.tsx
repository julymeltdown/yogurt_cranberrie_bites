/** Decorative recognizable marks; disabled parent buttons own all semantics. */
export function ProviderMark({ provider }: { provider: 'X' | 'Discord' | 'Google' }) {
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    {provider === 'X' ? <path fill="currentColor" d="M3 3h5l5 7 6-7h2l-7 9 7 9h-5l-5-7-6 7H3l7-9Zm3 2 11 14h2L8 5Z" />
      : provider === 'Google' ? <path fill="currentColor" d="M21 10H12v4h5a6 6 0 1 1-2-7l3-3A10 10 0 1 0 22 12l-.2-2Z" />
        : <path fill="currentColor" d="M5 5 9 4l1 2h4l1-2 4 1c2 3 3 7 3 11l-5 3-2-2 2-1c-3 1-7 1-10 0l2 1-2 2-5-3c0-4 1-8 3-11Zm3 5a2 2 0 1 0 0 4 2 2 0 0 0 0-4Zm8 0a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z" />}
  </svg>
}
