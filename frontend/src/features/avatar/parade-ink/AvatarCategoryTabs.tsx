import { useId, useRef, type KeyboardEvent, type ReactNode } from 'react'
import './parade-ink-avatar.css'

interface Category { readonly value: string; readonly label: string; readonly content: ReactNode }

/** Horizontal automatic-activation tabs; all catalog categories remain reachable. */
export function AvatarCategoryTabs({ items, value, onChange }: {
  readonly items: readonly Category[]
  readonly value: string
  readonly onChange: (value: string) => void
}) {
  const id = useId(), buttons = useRef(new Map<string, HTMLButtonElement>())
  function move(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next = event.key === 'ArrowRight' ? (index + 1) % items.length
      : event.key === 'ArrowLeft' ? (index - 1 + items.length) % items.length
        : event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : -1
    if (next < 0 || !items.length) return
    event.preventDefault(); event.stopPropagation()
    buttons.current.get(items[next]!.value)?.focus()
  }
  return <>
    <div className="pi-avatar-tabs" role="tablist" aria-label="Avatar categories" aria-orientation="horizontal">
      {items.map((item, index) => <button key={item.value} type="button" role="tab" id={`${id}-tab-${index}`}
        className={['hair', 'eyes', 'outfit'].includes(item.value) ? 'pi-avatar-tabs__primary' : 'pi-avatar-tabs__secondary'}
        aria-controls={`${id}-panel-${index}`} aria-selected={item.value === value} tabIndex={item.value === value ? 0 : -1}
        ref={element => { if (element) buttons.current.set(item.value, element); else buttons.current.delete(item.value) }}
        onFocus={() => { if (item.value !== value) onChange(item.value) }}
        onClick={() => { if (item.value !== value) onChange(item.value) }} onKeyDown={event => move(event, index)}>{item.label}</button>)}
    </div>
    {items.map((item, index) => <div key={item.value} className="mcr-avatar-category-panel pi-avatar-tabpanel" role="tabpanel"
      id={`${id}-panel-${index}`} aria-labelledby={`${id}-tab-${index}`} hidden={item.value !== value} tabIndex={0}>{item.content}</div>)}
  </>
}
