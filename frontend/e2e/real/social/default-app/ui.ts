import { expect, type ElementHandle, type Page } from '@playwright/test'

/** Browser operations use existing DOM/UI only. No injected Auth/controller/router/World API. */
export class DefaultAppUI {
  private host: ElementHandle<HTMLElement | SVGElement> | null = null
  private canvas: ElementHandle<HTMLElement | SVGElement> | null = null
  constructor(readonly page: Page, private readonly feedIds: () => readonly string[], private readonly userId: string) {}
  async signIn(origin: string, principal: { email: string; password: string }) {
    try {
      await this.page.goto(new URL('/login?returnTo=%2Ffeed', origin).href)
      await this.page.getByLabel('Email', { exact: true }).fill(principal.email)
      await this.page.getByLabel('Password', { exact: true }).fill(principal.password)
      await this.page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
    } catch { throw Error('NOT_RUN: default application sign-in unavailable') }
    await expect(this.page.locator('[data-mcr-shell]')).toHaveCount(1)
    await expect(this.page).toHaveURL(/\/feed$/)
  }
  async rememberWorld() {
    await expect(this.page.locator('[data-world-host] canvas')).toHaveCount(1)
    this.host = await this.page.locator('[data-world-host]').elementHandle()
    this.canvas = await this.page.locator('[data-world-host] canvas').elementHandle()
    if (!this.host || !this.canvas) throw Error('NOT_RUN: actual World canvas unavailable')
  }
  async sameWorld() {
    expect(this.host && await this.host.evaluate(node => node.isConnected && node === document.querySelector('[data-world-host]'))).toBe(true)
    expect(this.canvas && await this.canvas.evaluate(node => node.isConnected && node === document.querySelector('[data-world-host] canvas'))).toBe(true)
    await expect(this.page.locator('[data-world-host] canvas')).toHaveCount(1)
  }
  async link(path: string) {
    // Exact href is from the frozen product routes; never call history.pushState or router internals.
    const link = this.page.locator(`a[href=${JSON.stringify(path)}]:visible`).first()
    await link.click(); await expect(this.page).toHaveURL(url => url.pathname === path)
    await this.sameWorld()
  }
  async unavailable(kind: 'intro' | 'people' | 'sharing' | 'notification') {
    if (kind === 'intro') { await this.link('/meet'); await this.link('/meet/intro/edit') }
    if (kind === 'people') { await this.link('/meet'); await this.link('/meet/people') }
    if (kind === 'sharing') { await this.link('/me'); await this.link('/me/social') }
    if (kind === 'notification') await this.link('/notifications')
    await expect(this.page.getByText('This section is unavailable for the current account or event.', { exact: true })).toBeVisible()
    // Unavailability is composition evidence only, never an age/provider or HTTP denial result.
  }
  async comment(postId: string, text: string) {
    await this.link('/feed')
    await this.openPost(postId)
    await this.sameWorld()
    await this.page.getByLabel('Comment text', { exact: true }).fill(text)
    const response = this.page.waitForResponse(value => value.request().method() === 'POST' && new URL(value.url()).pathname.endsWith(`/posts/${postId}/comments`))
    await this.page.getByRole('button', { name: 'Send comment', exact: true }).click()
    expect([200, 202]).toContain((await response).status())
    for (let attempt = 0; attempt < 12; attempt++) {
      const check = this.page.getByRole('button', { name: 'Check command status', exact: true })
      if (await check.isVisible()) await check.click()
      if (await this.page.getByText(text, { exact: true }).isVisible()) break
      await this.page.waitForTimeout(250)
    }
    await expect(this.page.getByText(text, { exact: true })).toBeVisible()
    // Re-enter using UI to require a new persisted server read, without replacing the World shell.
    await this.link('/feed'); return this.commentReadback(postId, text)
  }
  private async commentReadback(postId: string, text: string) {
    const persisted = this.page.waitForResponse(value => value.request().method() === 'GET' && new URL(value.url()).pathname.endsWith(`/posts/${postId}`))
    await this.openPost(postId)
    const response = await persisted
    expect(response.status()).toBe(200)
    let commentId = this.commentIdentity((await response.json() as { comments?: unknown }).comments, postId, text)
    await expect(this.page).toHaveURL(url => url.pathname === `/feed/${postId}`)
    for (let page = 0; page < 10 && !await this.page.getByText(text, { exact: true }).isVisible(); page++) {
      const more = this.page.getByRole('button', { name: 'More comments', exact: true })
      if (!await more.isVisible()) break
      const response = this.page.waitForResponse(value => value.request().method() === 'GET' && new URL(value.url()).pathname.endsWith(`/posts/${postId}/comments`))
      await more.click()
      const body = await (await response).json() as { items?: unknown }
      commentId ??= this.commentIdentity(body.items, postId, text)
    }
    await expect(this.page.getByText(text, { exact: true })).toBeVisible(); await this.sameWorld()
    if (!commentId) throw Error('Actual comment readback identity unavailable')
    return { commentId, persisted: true }
  }
  private commentIdentity(value: unknown, postId: string, text: string): string | null {
    if (!Array.isArray(value)) return null
    const row = value.find((item: { text?: string; post_id?: string; author?: { user_id?: string } }) => item.text === text && item.post_id === postId && item.author?.user_id === this.userId) as { comment_id?: string } | undefined
    return typeof row?.comment_id === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(row.comment_id) ? row.comment_id : null
  }
  private async openPost(postId: string) {
    const buttons = this.page.getByRole('button', { name: /^Open thread ·/ })
    for (let page = 0; page < 10; page++) {
      const index = this.feedIds().indexOf(postId)
      if (index >= 0) {
        await expect(buttons.nth(index)).toBeVisible(); await buttons.nth(index).click()
        await expect(this.page).toHaveURL(url => url.pathname === `/feed/${postId}`); return
      }
      const more = this.page.getByRole('button', { name: 'Load older posts', exact: true })
      if (!await more.isVisible()) break
      const next = this.page.waitForResponse(value => value.status() === 200 && new URL(value.url()).pathname.endsWith('/feed'))
      await more.click(); await next
      await expect.poll(() => buttons.count()).toBeGreaterThan(0)
    }
    throw Error('NOT_RUN: admitted Post not exposed in bounded actual Feed pages')
  }
  async disposeHandles() { await this.host?.dispose(); await this.canvas?.dispose(); this.host = null; this.canvas = null }
  async signOut() {
    await this.link('/me')
    await this.page.getByText('Profile options', { exact: true }).click()
    await this.link('/me/account')
    await this.page.getByRole('button', { name: 'Sign out', exact: true }).click()
    const response = this.page.waitForResponse(value => value.request().method() === 'POST' && ['/api/auth/logout', '/auth/logout'].includes(new URL(value.url()).pathname))
    await this.page.getByRole('button', { name: 'Confirm sign out', exact: true }).click()
    expect((await response).status()).toBe(204)
    await expect(this.page.locator('[data-mcr-shell]')).toHaveCount(0)
    await expect(this.page.locator('[data-world-host] canvas')).toHaveCount(0)
    await expect(this.page.getByRole('region', { name: 'Revealed account name', exact: true })).toHaveCount(0)
  }
}
