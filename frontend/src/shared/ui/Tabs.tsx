import { useId, useRef, type KeyboardEvent, type ReactNode } from 'react'

export interface TabItem {
  value: string
  label: string
  content: ReactNode
  disabled?: boolean
}
export interface TabsProps {
  label: string
  items: readonly TabItem[]
  value: string
  onValueChange: (value: string) => void
  orientation?: 'horizontal' | 'vertical'
}

export function Tabs({ label, items, value, onValueChange, orientation = 'horizontal' }: TabsProps) {
  const id = useId()
  const buttons = useRef(new Map<string, HTMLButtonElement>())
  const selected = items.find(item => item.value === value && !item.disabled)
    ?? items.find(item => !item.disabled)
  function move(event: KeyboardEvent<HTMLButtonElement>, current: string) {
    const enabled = items.filter(item => !item.disabled)
    const index = enabled.findIndex(item => item.value === current)
    const forward = orientation === 'horizontal' ? 'ArrowRight' : 'ArrowDown'
    const backward = orientation === 'horizontal' ? 'ArrowLeft' : 'ArrowUp'
    let target: TabItem | undefined
    if (event.key === forward) target = enabled[(index + 1) % enabled.length]
    else if (event.key === backward) target = enabled[(index - 1 + enabled.length) % enabled.length]
    else if (event.key === 'Home') target = enabled[0]
    else if (event.key === 'End') target = enabled.at(-1)
    if (target) {
      event.preventDefault()
      buttons.current.get(target.value)?.focus()
    }
  }

  return (
    <div className="mcr-tabs">
      <div role="tablist" aria-label={label} aria-orientation={orientation} className="mcr-tablist">
        {items.map((item, index) => (
          <button key={item.value} type="button" role="tab" id={`${id}-tab-${index}`}
            aria-controls={`${id}-panel-${index}`} aria-selected={item === selected}
            tabIndex={item === selected ? 0 : -1} disabled={item.disabled} className="mcr-tab"
            ref={element => {
              if (element) buttons.current.set(item.value, element)
              else buttons.current.delete(item.value)
            }}
            onFocus={() => { if (item.value !== selected?.value) onValueChange(item.value) }}
            onClick={() => { if (item.value !== selected?.value) onValueChange(item.value) }}
            onKeyDown={event => move(event, item.value)}>{item.label}</button>
        ))}
      </div>
      {items.map((item, index) => (
        <div key={item.value} role="tabpanel" id={`${id}-panel-${index}`}
          aria-labelledby={`${id}-tab-${index}`} hidden={item !== selected} tabIndex={0}
          className="mcr-tabpanel">{item.content}</div>
      ))}
    </div>
  )
}
