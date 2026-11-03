import { useId, useRef, type KeyboardEvent, type ReactNode } from 'react'

const glyphs: Record<string, ReactNode> = {
  presets: <path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2L12 17.3l-5.6 2.9 1.1-6.2L3 9.6l6.2-.9Z" />,
  body: <><circle cx="12" cy="5" r="3" /><path d="M7 11h10l2 8h-5v3h-4v-3H5Z" /></>,
  hair: <path d="M5 18c-3-5-2-10 1-13 3-3 9-3 12 1 3 3 3 9 1 12l-3-2 1-5-4-4-2 5-3-1 1 5-2 3Z" />,
  eyes: <><path d="M2 11c3-5 7-5 10 0-3 5-7 5-10 0Zm10 0c3-5 7-5 10 0-3 5-7 5-10 0Z" /><path d="M7 9v4m10-4v4" /></>,
  outfit: <path d="m8 3 4 2 4-2 6 5-4 4-2-2v11H8V10l-2 2-4-4Z" />,
  accessory: <><path d="m3 9 7 3-3 6-5-4Zm18 0-7 3 3 6 5-4Z" /><circle cx="12" cy="13" r="2" /></>,
  tone: <><path d="M12 3a9 9 0 1 0 0 18h2a2 2 0 0 0 1-4c-2-1-1-3 1-3h2c5 0 3-11-6-11Z" /><path d="M6 10h.01M9 6h.01M15 6h.01M18 10h.01" strokeWidth="3" /></>,
}
export function CategoryIcon({ category }: { readonly category: string }) {
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" strokeLinecap="round" aria-hidden="true" focusable="false">{glyphs[category] ?? glyphs.presets}</svg>
}
interface Category { readonly value: string; readonly label: string; readonly content: ReactNode }
/** Feature-local icon presentation; same automatic-activation vertical keyboard pattern as shared Tabs. */
export function CategoryRail({ items, value, onChange }: { readonly items: readonly Category[]; readonly value: string; readonly onChange: (value: string) => void }) {
  const id = useId(), buttons = useRef(new Map<string, HTMLButtonElement>())
  function move(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next = event.key === 'ArrowDown' ? (index + 1) % items.length : event.key === 'ArrowUp' ? (index - 1 + items.length) % items.length
      : event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : -1
    if (next >= 0) { event.preventDefault(); buttons.current.get(items[next]!.value)?.focus() }
  }
  return <>
    <div className="mcr-avatar-category-rail" role="tablist" aria-label="Avatar categories" aria-orientation="vertical">
      {items.map((item, index) => <button type="button" key={item.value} role="tab" id={`${id}-tab-${index}`} aria-controls={`${id}-panel-${index}`}
        aria-label={item.label} title={item.label} aria-selected={value === item.value} tabIndex={value === item.value ? 0 : -1}
        ref={element => { if (element) buttons.current.set(item.value, element); else buttons.current.delete(item.value) }}
        onFocus={() => { if (value !== item.value) onChange(item.value) }} onClick={() => { if (value !== item.value) onChange(item.value) }} onKeyDown={event => move(event, index)}>
        <CategoryIcon category={item.value} /><span className="mcr-avatar-sr-only">{item.label}</span>
      </button>)}
    </div>
    {items.map((item, index) => <div role="tabpanel" key={item.value} id={`${id}-panel-${index}`} aria-labelledby={`${id}-tab-${index}`} hidden={value !== item.value}
      tabIndex={0} className="mcr-avatar-category-panel">{item.content}</div>)}
  </>
}
