import { expect, type Locator, type Response } from '@playwright/test'
import { DefaultAppUI } from './ui'

type Platform = 'INSTAGRAM' | 'X'
export type SocialScenario = { introId: string; platform: Platform; requesterHandle: string; recipientHandle: string }
type RequestRow = { request_id: string; event_id: string; requester_id: string; recipient_id: string; intro_id: string; state: string }
type Notice = { notification_id: string; event_id: string; target: { target_type: string; target_id: string } | null; created_at: string; read_at: string | null }
const uuid = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
const label = (platform: Platform) => platform === 'INSTAGRAM' ? 'Instagram' : 'X'

/** Drives the existing application. All credentials, handles, target IDs and bodies stay in RAM. */
export class SocialUI {
  private readonly page
  private readonly prefix
  constructor(readonly ui: DefaultAppUI, private readonly eventId: string, private readonly requesterId: string,
    private readonly recipientId: string, private readonly scenario: SocialScenario) {
    this.page = ui.page; this.prefix = `/api/concert/events/${eventId}`
  }
  private matches(value: RequestRow, state: string, id?: string) {
    return uuid(value.request_id) && (!id || value.request_id === id) && value.event_id === this.eventId
      && value.requester_id === this.requesterId && value.recipient_id === this.recipientId
      && value.intro_id === this.scenario.introId && value.state === state
  }
  private async body<T>(response: Response): Promise<T> {
    expect(response.status() === 200).toBe(true)
    return response.json() as Promise<T>
  }
  private read(path: string) {
    return this.page.waitForResponse(response => response.request().method() === 'GET' && new URL(response.url()).pathname === this.prefix + path)
  }
  private async noConsent(button: Locator, suffix: string) {
    let writes = 0
    const count = (request: import('@playwright/test').Request) => {
      if (request.method() === 'POST' && new URL(request.url()).pathname === this.prefix + suffix) writes++
    }
    this.page.on('request', count)
    try {
      await expect(button).toBeDisabled()
      await this.page.waitForTimeout(250)
      expect(writes).toBe(0)
    } finally { this.page.off('request', count) }
  }
  private async command(button: Locator, suffix: string, state: string, id?: string) {
    let settled = false, failed = false, writes = 0
    const count = (request: import('@playwright/test').Request) => {
      if (request.method() === 'POST' && new URL(request.url()).pathname === this.prefix + suffix) writes++
    }
    const verified = this.page.waitForResponse(async response => {
      if (response.request().method() !== 'GET' || response.status() !== 200
        || !new URL(response.url()).pathname.startsWith(this.prefix + '/social-requests/')) return false
      try { return this.matches(await response.json() as RequestRow, state, id) } catch { return false }
    }).then(response => this.body<RequestRow>(response)).then(value => { settled = true; return value }, () => { failed = true; return null })
    this.page.on('request', count)
    try {
      const sent = this.page.waitForResponse(response => response.request().method() === 'POST' && new URL(response.url()).pathname === this.prefix + suffix)
      await button.click()
      expect([200, 202].includes((await sent).status())).toBe(true)
      for (let attempt = 0; attempt < 12 && !settled && !failed; attempt++) {
        const check = this.page.getByRole('button', { name: 'Check change status', exact: true })
        if (await check.isVisible()) await check.click() // Original receipt lookup only; never resubmit.
        await this.page.waitForTimeout(250)
      }
      const result = await verified
      expect(result !== null && !failed).toBe(true)
      await expect(this.page.getByText('The server confirmed this change.', { exact: true })).toBeVisible()
      expect(writes).toBe(1)
      return result!
    } finally { this.page.off('request', count) }
  }
  async create() {
    await this.ui.link('/meet'); await this.ui.link('/meet/people')
    const profile = this.page.locator(`a[href=${JSON.stringify('/people/' + this.recipientId)}]:visible`).first()
    for (let index = 0; index < 10 && !await profile.isVisible(); index++) {
      const more = this.page.getByRole('button', { name: 'More people', exact: true })
      if (!await more.isVisible()) break
      const next = this.read('/intros'); await more.click(); await this.body(await next)
    }
    await expect(profile).toBeVisible(); await profile.click(); await this.ui.sameWorld()
    const intro = this.read('/intros/' + this.scenario.introId)
    await this.page.getByRole('button', { name: 'Request sharing', exact: true }).click()
    const value = await this.body<{ intro_id: string; event_id: string; author: { user_id: string }; state: string }>(await intro)
    expect(value.intro_id === this.scenario.introId && value.event_id === this.eventId
      && value.author.user_id === this.recipientId && value.state === 'PUBLISHED').toBe(true)
    const offer = this.page.getByRole('region', { name: 'Request sharing', exact: true })
    const consent = offer.getByRole('checkbox', { name: `I consent to share my ${label(this.scenario.platform)} account name with this person if they approve a reciprocal exchange.`, exact: true })
    await expect(consent).not.toBeChecked()
    const send = offer.getByRole('button', { name: 'Send sharing request', exact: true })
    await this.noConsent(send, '/social-requests'); await consent.check()
    const created = await this.command(send, '/social-requests', 'PENDING')
    await this.ui.sameWorld(); return created.request_id
  }
  async open(id: string, direction: 'INCOMING' | 'OUTGOING', state: string) {
    await this.ui.link('/me'); await this.ui.link('/me/requests')
    const back = this.page.getByRole('button', { name: 'Back to requests', exact: true })
    if (await back.isVisible()) await back.click()
    const list = this.page.getByRole('region', { name: 'Sharing requests', exact: true })
    await list.getByLabel('Direction', { exact: true }).selectOption(direction)
    const response = this.read('/me/social-requests')
    await list.getByRole('button', { name: 'Refresh requests', exact: true }).click()
    let items = (await this.body<{ items: RequestRow[] }>(await response)).items
    for (let index = 0; index < 10; index++) {
      const position = items.findIndex(item => this.matches(item, state, id))
      if (position >= 0) {
        const detail = this.read('/social-requests/' + id)
        await list.getByRole('button', { name: 'Open request', exact: true }).nth(position).click()
        expect(this.matches(await this.body<RequestRow>(await detail), state, id)).toBe(true)
        await expect(this.page.getByRole('region', { name: 'Sharing request detail', exact: true }).getByRole('status')).toHaveText(state)
        await this.ui.sameWorld(); return
      }
      const more = list.getByRole('button', { name: 'Load older requests', exact: true })
      if (!await more.isVisible()) break
      const next = this.read('/me/social-requests'); await more.click()
      items = [...items, ...(await this.body<{ items: RequestRow[] }>(await next)).items]
    }
    throw Error('Actual request absent from bounded authorized UI list')
  }
  async approve(id: string) {
    const detail = this.page.getByRole('region', { name: 'Sharing request detail', exact: true })
    const platform = detail.getByRole('checkbox', { name: `Exchange ${label(this.scenario.platform)} account names`, exact: true })
    const consent = detail.getByRole('checkbox', { name: 'I explicitly consent to share my selected account names with the requester.', exact: true })
    const approve = detail.getByRole('button', { name: 'Approve selected platforms', exact: true })
    await expect(platform).not.toBeChecked(); await expect(consent).not.toBeChecked()
    await this.noConsent(approve, `/social-requests/${id}/decision`)
    await platform.check(); await this.noConsent(approve, `/social-requests/${id}/decision`)
    await consent.check(); await this.command(approve, `/social-requests/${id}/decision`, 'APPROVED', id)
  }
  async reveal(id: string, ownerId: string, handle: string) {
    await expect(this.page.getByRole('region', { name: 'Revealed account name', exact: true })).toHaveCount(0)
    const response = this.read(`/social-requests/${id}/reveal`)
    await this.page.getByRole('button', { name: 'Reveal ' + label(this.scenario.platform), exact: true }).click()
    const result = await response, headers = await result.allHeaders()
    expect(headers['cache-control']?.split(',').map(value => value.trim().toLowerCase()).includes('no-store')).toBe(true)
    const secret = await this.body<{ request_id: string; platform: string; owner_id: string; handle: string }>(result)
    expect(secret.request_id === id && secret.platform === this.scenario.platform && secret.owner_id === ownerId && secret.handle === handle).toBe(true)
    const region = this.page.getByRole('region', { name: 'Revealed account name', exact: true })
    await expect(region).toBeVisible()
    expect(await region.locator('.sharing-handle').textContent() === '@' + handle).toBe(true)
  }
  async revoke(id: string) {
    await this.page.getByRole('button', { name: 'Revoke sharing', exact: true }).click()
    await this.command(this.page.getByRole('button', { name: 'Confirm revocation', exact: true }), `/social-requests/${id}/revoke`, 'REVOKED', id)
    await this.withdrawn()
  }
  async withdrawn() {
    await expect(this.page.getByRole('region', { name: 'Revealed account name', exact: true })).toHaveCount(0)
    await this.ui.sameWorld()
  }
  async blockRecipient() {
    await this.ui.link('/meet'); await this.ui.link('/meet/people')
    const profile = this.page.locator(`a[href=${JSON.stringify('/people/' + this.recipientId)}]:visible`).first()
    for (let index = 0; index < 10 && !await profile.isVisible(); index++) {
      const more = this.page.getByRole('button', { name: 'More people', exact: true })
      if (!await more.isVisible()) break
      const next = this.read('/intros'); await more.click(); await this.body(await next)
    }
    await expect(profile).toBeVisible(); await profile.click()
    await this.page.getByRole('button', { name: 'Block user', exact: true }).click()
    const sent = this.page.waitForResponse(value => value.request().method() === 'PUT' && new URL(value.url()).pathname === `${this.prefix}/me/blocks/${this.recipientId}`)
    const persisted = this.read('/me/blocks')
    await this.page.getByRole('button', { name: 'Confirm block', exact: true }).click()
    expect([200, 202].includes((await sent).status())).toBe(true)
    for (let attempt = 0; attempt < 12; attempt++) {
      if (new URL(this.page.url()).pathname === '/me/blocks') break
      const check = this.page.getByRole('button', { name: 'Check original request status', exact: true })
      if (await check.isVisible()) await check.click()
      await this.page.waitForTimeout(250)
    }
    await expect(this.page).toHaveURL(url => url.pathname === '/me/blocks')
    let body = await this.body<{ items: { target_user_id: string; blocked_by_me: boolean }[] }>(await persisted)
    for (let index = 0; index < 10; index++) {
      if (body.items.some(item => item.target_user_id === this.recipientId && item.blocked_by_me)) {
        // Compare privately; never include the subject ID in a locator failure or public evidence.
        const rows = await this.page.locator('.mcr-safety-id').allTextContents()
        expect(rows.includes(this.recipientId)).toBe(true); await this.ui.sameWorld(); return
      }
      const more = this.page.getByRole('button', { name: 'Next page', exact: true })
      if (!await more.isVisible()) break
      const next = this.read('/me/blocks'); await more.click(); body = await this.body(await next)
    }
    throw Error('Canonical block not present in bounded persisted UI list')
  }
  private sortNotices(items: Notice[]) {
    const stamp = (value: string) => value.slice(0, 19) + value.slice(19, -1).replace(/^\./, '').padEnd(9, '0')
    return [...new Map(items.map(item => [item.notification_id, item])).values()]
      .sort((a, b) => stamp(b.created_at).localeCompare(stamp(a.created_at)) || b.notification_id.localeCompare(a.notification_id))
  }
  async notification(id: string, mark: boolean): Promise<void> {
    await this.ui.link('/notifications')
    const inbox = this.page.getByRole('region', { name: 'Notification inbox', exact: true })
    await inbox.getByLabel('Unread only', { exact: true }).uncheck()
    const response = this.read('/me/notifications')
    await inbox.getByRole('button', { name: 'Refresh inbox', exact: true }).click()
    let items = this.sortNotices((await this.body<{ items: Notice[] }>(await response)).items)
    for (let index = 0; index < 10; index++) {
      const position = items.findIndex(item => item.event_id === this.eventId && item.target?.target_type === 'SOCIAL_REQUEST' && item.target.target_id === id)
      if (position >= 0) {
        const notice = items[position]!, row = inbox.getByRole('list', { name: 'Event updates', exact: true }).locator('li').nth(position)
        if (mark) {
          expect(notice.read_at === null).toBe(true)
          const sent = this.page.waitForResponse(value => value.request().method() === 'PATCH' && new URL(value.url()).pathname === `${this.prefix}/me/notifications/${notice.notification_id}/read`)
          await row.getByRole('button', { name: 'Mark as read', exact: true }).click()
          expect([200, 202].includes((await sent).status())).toBe(true)
          for (let attempt = 0; attempt < 12 && await row.getByRole('button', { name: 'Mark as read', exact: true }).isVisible(); attempt++) {
            const check = inbox.getByRole('button', { name: 'Check command status', exact: true })
            if (await check.isVisible()) await check.click()
            await this.page.waitForTimeout(250)
          }
          await expect(row.getByText('Read', { exact: true })).toBeVisible()
          return this.notification(id, false) // Fresh list GET must persist read_at; no mutation replay.
        }
        expect(notice.read_at !== null && Number.isFinite(Date.parse(notice.read_at))).toBe(true)
        const authorized = this.read('/social-requests/' + id)
        await row.getByRole('button', { name: 'Open update', exact: true }).click()
        const target = await this.body<RequestRow>(await authorized)
        expect(this.matches(target, 'PENDING', id)).toBe(true)
        await expect(this.page).toHaveURL(url => url.pathname === '/me/requests'); await this.ui.sameWorld(); return
      }
      const more = inbox.getByRole('button', { name: 'Load older updates', exact: true })
      if (!await more.isVisible()) break
      const next = this.read('/me/notifications'); await more.click()
      items = this.sortNotices([...items, ...(await this.body<{ items: Notice[] }>(await next)).items])
    }
    throw Error('Actual target notification absent from bounded UI inbox')
  }
}
