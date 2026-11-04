// Next original software states; imported only by a later frozen cohort.
import assert from 'node:assert/strict'
import {expect} from '@playwright/test'

export async function settingsCase({page,c,navigate,state,selected,hit,bounds,shot,retained}){
 await navigate(page,'/me/settings')
 const settings=page.getByRole('region',{name:'Settings',exact:true}),era=settings.getByRole('combobox',{name:/^Preferred era/})
 await expect(era).toBeEnabled()
 const low=settings.getByRole('checkbox',{name:'Low power',exact:true}),motion=settings.getByRole('checkbox',{name:'Reduced motion',exact:true}),sound=settings.getByRole('checkbox',{name:'Enable sound',exact:true})
 c.savedDraft={era:await era.inputValue(),lowPower:await low.isChecked(),reducedMotion:await motion.isChecked(),sound:await sound.isChecked()}
 const save=settings.getByRole('button',{name:'Save preferences',exact:true}),cancel=settings.getByRole('button',{name:'Cancel preference changes',exact:true})
 await expect(save).toBeDisabled();await expect(cancel).toBeDisabled();assert.equal(c.savedDraft.sound,false)
 c.toggleTargets=[]
 for(const checkbox of [low,motion]){
  // The actual44px label is the checkbox pointer target; the native22px glyph is not claimed44px.
  const label=checkbox.locator('..');c.toggleTargets.push(await hit(label));await label.click()
 }
 await expect(low).toBeChecked({checked:!c.savedDraft.lowPower});await expect(motion).toBeChecked({checked:!c.savedDraft.reducedMotion})
 const nextEra=c.savedDraft.era==='DANGER'?'PARADE':'DANGER';await era.selectOption(nextEra)
 await expect(save).toBeEnabled();await expect(cancel).toBeEnabled();c.primaryTarget=await hit(save);c.cancelTarget=await hit(cancel)
 assert.equal(selected(await state(page)),c.era,'Unsaved form era is not applied to World')
 await cancel.focus();await page.keyboard.press('Enter')
 await expect(low).toBeChecked({checked:c.savedDraft.lowPower});await expect(motion).toBeChecked({checked:c.savedDraft.reducedMotion});await expect(sound).not.toBeChecked();await expect(era).toHaveValue(c.savedDraft.era)
 await expect(save).toBeDisabled();await expect(cancel).toBeDisabled();c.cancelRestoredDraft=true
 c.geometry=await bounds(page);if(c.capture)c.screenshot=await shot(page,c.id+'.png');c.runtime=await retained(page,c)
 await settings.getByRole('link',{name:'Back to world',exact:true}).click();await expect.poll(async()=>(await state(page)).path).toBe('/world');c.returnState=await retained(page,c)
}

export async function giftsActivityCase({page,c,navigate,state,hit,bounds,shot,retained}){
 await navigate(page,'/gifts/mine')
 const activity=page.getByRole('region',{name:'My gift activity',exact:true}),group=activity.getByRole('group',{name:'Gift activity collections',exact:true})
 await expect(activity.getByRole('heading',{name:'My gifts',exact:true})).toBeVisible()
 const giving=group.getByRole('button',{name:'GIVING',exact:true}),receiving=group.getByRole('button',{name:'RECEIVING',exact:true})
 c.tabTargets=[await hit(giving),await hit(receiving)]
 await expect(giving).toHaveAttribute('aria-pressed','true');await expect(activity.getByText('No giving activity on this page.',{exact:true})).toBeVisible()
 await receiving.click();await expect(receiving).toHaveAttribute('aria-pressed','true');await expect(giving).toHaveAttribute('aria-pressed','false');await expect(activity.getByText('No receiving activity on this page.',{exact:true})).toBeVisible()
 // Existing contract fixture really has no own activity: do not synthesize recipients/stock/history/private pickup information.
 assert.equal(await activity.locator('a[href*="/gifts/pickups/"]').count(),0)
 c.receivingEmpty=true;c.geometry=await bounds(page);if(c.capture)c.screenshot=await shot(page,c.id+'.png')
 await giving.click();await expect(activity.getByText('No giving activity on this page.',{exact:true})).toBeVisible();c.tabsRestored=true;c.runtime=await retained(page,c)
 const browse=activity.getByRole('link',{name:'Browse giveaways →',exact:true});c.browseTarget=await hit(browse);await browse.click();await expect.poll(async()=>(await state(page)).path).toBe('/gifts');c.returnState=await retained(page,c)
}
