// Use the actual default-shell recovery control before canonical signout.
export async function reconnectAfterDegraded(page,editor,expect){
 const status=page.getByRole('region',{name:'World connection status',exact:true})
 await expect(status).toBeVisible()
 await status.getByRole('button',{name:'Reconnect',exact:true}).click()
 await expect.poll(()=>page.evaluate(()=>window.__MURAL_ROOT_ENTRY__.state().active),{timeout:15000}).toBe(1)
 await expect(status).toHaveCount(0)
 await expect(editor).toBeVisible()
 await expect(editor.getByLabel('Wall synchronization status',{exact:true})).toHaveAttribute('data-phase','ready')
}
