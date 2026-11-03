export function FeedGlyph({ name }: { name: 'star' | 'comment' | 'share' | 'plus' | 'refresh' | 'options' }) {
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
    {name === 'star' && <path fill="currentColor" d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9Z" />}
    {name === 'comment' && <path fill="currentColor" d="M4 4h16v12h-7l-5 5v-5H4Z" />}
    {name === 'share' && <><circle cx="5" cy="12" r="2" /><circle cx="18" cy="5" r="2" /><circle cx="18" cy="19" r="2" /><path d="m7 11 9-5M7 13l9 5" /></>}
    {name === 'plus' && <path d="M12 4v16M4 12h16" />}
    {name === 'refresh' && <path d="M20 11a8 8 0 1 0-2 6M20 4v7h-7" />}
    {name === 'options' && <><circle cx="5" cy="12" r="1" /><circle cx="12" cy="12" r="1" /><circle cx="19" cy="12" r="1" /></>}
  </svg>
}
