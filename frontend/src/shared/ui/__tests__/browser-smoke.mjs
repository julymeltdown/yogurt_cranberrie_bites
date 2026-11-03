// Run from frontend/ with an explicitly selected local Chrome. No browser downloads.
import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

assert(process.env.MCR_CHROME_EXECUTABLE, 'Set MCR_CHROME_EXECUTABLE to a local Chrome executable')
const server = await createServer({ server: { host: '127.0.0.1', port: 0, strictPort: false } })
let browser
try {
  await server.listen()
  const address = server.httpServer.address()
  assert(address && typeof address !== 'string')
  browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE, headless: true })
  const errors = []
  for (const [width, height] of [[360, 800], [390, 844], [430, 932], [1280, 800]]) {
    const context = await browser.newContext({ viewport: { width, height }, reducedMotion: 'reduce' })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`http://127.0.0.1:${address.port}/src/shared/ui/__tests__/fixtures/preview.html`)
    await expect(page.getByRole('heading', { name: 'MCR primitives' })).toBeVisible()
    if (width === 390 && process.env.MCR_UI_SCREENSHOT) {
      await page.screenshot({ path: process.env.MCR_UI_SCREENSHOT, fullPage: true })
    }
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `overflow at ${width}px`)
    for (const target of await page.locator('.mcr-button, .mcr-input, .mcr-tab').all()) {
      if (!(await target.isVisible())) continue
      const box = await target.boundingBox()
      assert(box && box.width >= 44 && box.height >= 44, `touch target below 44px at ${width}px`)
    }
    assert.equal(await page.getByRole('button', { name: 'Open dialog' }).evaluate(
      element => getComputedStyle(element).transitionDuration), '0s')

    const plans = page.getByRole('tab', { name: 'Plans', exact: true })
    await plans.focus()
    await page.keyboard.press('ArrowRight')
    await expect(page.getByRole('tab', { name: 'Updates' })).toBeFocused()
    await page.keyboard.press('Home')
    await expect(plans).toBeFocused()
    await page.keyboard.press('Tab')
    await expect(page.getByRole('tabpanel', { name: 'Plans', exact: true })).toBeFocused()

    const opener = page.getByRole('button', { name: 'Open dialog', exact: true })
    await opener.click()
    const panel = page.getByRole('dialog', { name: 'Plan dialog', exact: true })
    await expect(page.getByRole('textbox', { name: 'Plan name' })).toBeFocused()
    await expect(page.locator('#fixture')).toHaveAttribute('inert', '')
    const bounds = await panel.boundingBox()
    assert(bounds && bounds.x >= 0 && bounds.y >= 0 && bounds.x + bounds.width <= width
      && bounds.y + bounds.height <= height, `dialog clipped at ${width}px`)
    await page.keyboard.press('Shift+Tab')
    await expect(panel.getByRole('button', { name: 'Close dialog' })).toBeFocused()
    await page.keyboard.press('Shift+Tab')
    const nestedOpener = page.getByRole('button', { name: 'Open confirmation' })
    await expect(nestedOpener).toBeFocused()
    await page.keyboard.press('Tab')
    await expect(panel.getByRole('button', { name: 'Close dialog' })).toBeFocused()
    await page.keyboard.press('Tab')
    await expect(page.getByRole('textbox', { name: 'Plan name' })).toBeFocused()
    await page.keyboard.press('Tab')
    await page.keyboard.press('Enter')
    await expect(page.getByRole('dialog', { name: 'Confirmation', exact: true })).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(nestedOpener).toBeFocused()
    await expect(panel).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(opener).toBeFocused()
    await expect(page.locator('#fixture')).not.toHaveAttribute('inert')

    await page.getByRole('button', { name: 'Open stacked dialogs' }).click()
    const confirmation = page.getByRole('dialog', { name: 'Confirmation', exact: true })
    await expect(confirmation).toBeVisible()
    assert(await confirmation.evaluate(element => element.contains(document.activeElement)), 'initial nested focus')
    // A real click also verifies stacking/occlusion when child layout effects ran first.
    await confirmation.getByRole('button', { name: 'Close dialog' }).click()
    await expect(confirmation).not.toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.getByRole('button', { name: 'Open stacked dialogs' })).toBeFocused()

    await page.getByRole('button', { name: 'Open sheet', exact: true }).click()
    const sheet = page.getByRole('dialog', { name: 'Plan sheet' })
    await expect(sheet).toBeVisible()
    const sheetBox = await sheet.boundingBox()
    assert(sheetBox && Math.abs(sheetBox.y + sheetBox.height - height) <= 1, 'sheet not bottom-aligned')
    const fieldBox = await page.getByRole('textbox', { name: 'Plan name' }).boundingBox()
    assert(fieldBox)
    await page.mouse.move(fieldBox.x + 8, fieldBox.y + 8)
    await page.mouse.down()
    await page.mouse.move(4, 4)
    await page.mouse.up()
    await expect(sheet).toBeVisible() // dragging out of content must not dismiss
    await page.mouse.click(4, 4)
    await expect(sheet).not.toBeVisible()
    await expect(page.getByRole('button', { name: 'Open sheet', exact: true })).toBeFocused()

    await page.getByRole('button', { name: 'Show notifications' }).click()
    const dismiss = page.getByRole('button', { name: 'Dismiss notification: Your draft is still here.' })
    await dismiss.focus()
    await page.keyboard.press('Enter')
    await expect(page.getByRole('button', { name: 'Dismiss notification: Could not send. Try again.' })).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(page.getByRole('region', { name: 'Notifications' })).toBeFocused()
    console.log(`PASS FE05 Chrome ${width}x${height}: native keyboard/focus, nested modal, sheet, targets, overflow, reduced motion, toast`)
    await context.close()
  }
  assert.deepEqual(errors, [], 'browser console errors')
  console.log(`PASS browser ${browser.version()}; physical devices and screen readers NOT_RUN`)
} finally {
  await browser?.close()
  await server.close()
}
