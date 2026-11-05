// Isolate the disconnect test from the prior tools-unavailable case.
export async function denyToolsAndRefresh(page,editor,expect){
 const refresh=editor.getByRole('button',{name:'Refresh confirmed wall',exact:true})
 await expect(refresh).toBeVisible();await expect(refresh).toBeEnabled()
 // Apply the fixture response change and dispatch the real button handler in
 // one task; an autonomous qualification tick can otherwise remove the button
 // while Playwright is waiting for layout stability.
 await refresh.evaluate(button=>{
  if(button.disabled||button.getBoundingClientRect().width===0)throw new Error('Refresh must remain actionable before tools denial')
  window.__MURAL_ROOT_ENTRY__.denyTools(true)
  button.click()
 })
}
export async function restoreTools(page,editor,expect){
 await page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.denyTools(false))
 await expect.poll(()=>page.evaluate(async()=>(await window.__MURAL_ROOT_ENTRY__.worldProof()).mural?.objects??0),{timeout:15000}).toBe(1)
 await expect(editor).toBeVisible()
 for(const label of ['Pencil preview','Erase preview','Fill preview'])await expect(editor.getByRole('button',{name:label,exact:true})).toBeEnabled()
 await expect(editor.getByLabel('Wall synchronization status',{exact:true})).toHaveAttribute('data-phase','ready')
}
