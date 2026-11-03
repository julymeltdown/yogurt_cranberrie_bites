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
 return <dialog className="mcr-safety-dialog" ref={ref} aria-labelledby={id} onKeyDown={event=>{
   if(event.key!=='Tab')return
   const stops=Array.from(ref.current!.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href],[tabindex="0"]')).filter(node=>node.getClientRects().length>0)
   const first=stops[0],last=stops.at(-1)
   if(!first||!last){event.preventDefault();ref.current?.focus();return}
   if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus()}
   else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus()}
  }} onCancel={event=>{event.preventDefault();onDismiss()}}>
  <h2 id={id}>{title}</h2>{children}
 </dialog>
}
