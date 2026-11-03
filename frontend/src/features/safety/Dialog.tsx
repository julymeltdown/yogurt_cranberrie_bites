import { useId, useLayoutEffect, useRef, type ReactNode } from 'react'
/** Native modal supplies inert background, focus containment and Escape semantics. */
export function SafetyDialog({ title, children, onDismiss }: { title:string;children:ReactNode;onDismiss:()=>void }) {
 const ref=useRef<HTMLDialogElement>(null),id=useId()
 useLayoutEffect(()=>{
  const dialog=ref.current!,previous=document.activeElement
  if(typeof dialog.showModal==='function')dialog.showModal();else dialog.setAttribute('open','')
  dialog.querySelector<HTMLElement>('[data-initial-focus]')?.focus()
  return()=>{if(dialog.open&&typeof dialog.close==='function')dialog.close();if(previous instanceof HTMLElement&&previous.isConnected)previous.focus()}
 },[])
 return <dialog className="mcr-safety-dialog" ref={ref} aria-labelledby={id} onCancel={event=>{event.preventDefault();onDismiss()}}>
  <h2 id={id}>{title}</h2>{children}
 </dialog>
}
