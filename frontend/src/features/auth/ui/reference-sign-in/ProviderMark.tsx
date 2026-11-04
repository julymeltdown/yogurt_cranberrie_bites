/** Decorative recognizable marks; disabled parent buttons own all semantics. */
export function ProviderMark({ provider }: { provider: 'X' | 'Discord' | 'Google' }) {
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    {provider === 'X' ? <path fill="currentColor" d="M3 3h5l5 7 6-7h2l-7 9 7 9h-5l-5-7-6 7H3l7-9Zm3 2 11 14h2L8 5Z" />
      : provider === 'Google' ? <path fill="currentColor" d="M21 10H12v4h5a6 6 0 1 1-2-7l3-3A10 10 0 1 0 22 12l-.2-2Z" />
        : <path fill="currentColor" fillRule="evenodd" d="M8 4.5 5 5.3C2.8 8.5 1.8 12 2 16.5l4.5 2.8 1.4-2-1.8-1a15.7 15.7 0 0 0 11.8 0l-1.8 1 1.4 2 4.5-2.8c.2-4.5-.8-8-3-11.2l-3-.8-.9 1.7a17 17 0 0 0-6.2 0ZM8 10c-2.5 0-2.5 4 0 4s2.5-4 0-4Zm8 0c-2.5 0-2.5 4 0 4s2.5-4 0-4Z" />}
  </svg>
}
