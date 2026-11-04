import { randomUUID } from 'node:crypto'
import { test, expect, type BrowserContext, type Page } from '@playwright/test'
import { admitOriginal, readRunConfig } from './social/admission.mjs'

type Role = 'requester' | 'recipient' | 'restricted'
const stateModule = '/e2e/real/social/browser-state.ts'
test.use({ trace: 'off', video: 'off', screenshot: 'off' })

/** Reuses separately allocated real services. Never launches a server or injects a token. */
test('IN03 actual public controller segments (full WS/cache gate remains NOT_RUN)', async ({ playwright }, testInfo) => {
  const config = readRunConfig(process.env.MCR_IN03_REAL_CONFIG)
  const admission = admitOriginal(config)
  await testInfo.attach('admission', { body: JSON.stringify(admission), contentType: 'application/json' })
  if (admission.status !== 'INPUTS_PRESENT') throw new Error('NOT_RUN: redacted original IN03 prerequisites are missing')
  if (!process.env.MCR_IN03_PUBLIC_ALLOCATION) throw new Error('NOT_RUN: actual public workflow requires an accepted named allocation')
  const browser = await playwright.chromium.launch()
  const contexts: BrowserContext[] = [], pages = new Map<Role, Page>()
  const coverage: Record<string, string> = { IN03: 'NOT_RUN', policyWS: 'NOT_RUN', block: 'NOT_RUN', staleResponse: 'NOT_RUN', clientCache: 'NOT_RUN',
    restrictedAgeHTTP: 'NOT_RUN', unknownEligibility: 'NOT_RUN', providerUnavailable: 'NOT_RUN' }
  const pageFor = (role: Role) => { const page = pages.get(role); if (!page) throw new Error('IN03 actor unavailable'); return page }
  try {
    for (const role of ['requester', 'recipient', 'restricted'] as const) {
      const context = await browser.newContext(); contexts.push(context)
      const page = await context.newPage(); pages.set(role, page)
      await page.goto(new URL('/e2e/real/social/entry.html', config.origin).href)
      const packet = await page.evaluate(async ({ module, input, source }) => {
        const state = await import(/* @vite-ignore */ module)
        return state.open(input, source)
      }, { module: stateModule, input: { ...config.principals[role], eventId: config.eventId }, source: config.source })
      // Public projection only. Authenticated C03 and its digest must never be attached.
      await testInfo.attach(`public-c03-${role}`, { body: JSON.stringify(packet), contentType: 'application/json' })
      await page.evaluate(async ({ module, path, revision }) => {
        const state = await import(/* @vite-ignore */ module)
        const producer = await import(/* @vite-ignore */ path)
        if (producer.sourceRevision !== revision || typeof producer.availability !== 'function') throw new Error('NOT_RUN: production availability export/revision mismatch')
        state.bind(producer.availability)
      }, { module: stateModule, path: config.availabilityModule, revision: config.availabilityRevision })
    }
    coverage.canonicalIdentity = 'EXECUTED'
    const discovered = await pageFor('requester').evaluate(async ({ module, introId, authorId }) => {
      const state = await import(/* @vite-ignore */ module)
      return state.current().discover(introId, authorId)
    }, { module: stateModule, introId: config.resources.introId, authorId: config.principals.recipient.userId })
    expect(discovered.discovered).toBe(true); coverage.intro = 'EXECUTED'
    const comment = await pageFor('requester').evaluate(async ({ module, postId, text }) => {
      const state = await import(/* @vite-ignore */ module)
      return state.current().comment(postId, text)
    }, { module: stateModule, postId: config.resources.postId, text: `IN03 public journey ${randomUUID()}` })
    expect(comment.persisted).toBe(true); coverage.comment = 'EXECUTED'
    const request = await pageFor('requester').evaluate(async ({ module, introId, recipientId }) => {
      const state = await import(/* @vite-ignore */ module)
      return state.current().request(introId, recipientId, 'INSTAGRAM')
    }, { module: stateModule, introId: config.resources.introId, recipientId: config.principals.recipient.userId })
    expect(request.state).toBe('PENDING'); coverage.requesterConsent = 'EXECUTED'
    // Resource identities stay in private run artifacts for the assigned backend cleanup owner.
    await testInfo.attach('created-resources-private', { body: JSON.stringify({ commentId: comment.commentId, requestId: request.requestId }), contentType: 'application/json' })
    const notification = await pageFor('recipient').evaluate(async ({ module, requestId }) => {
      const state = await import(/* @vite-ignore */ module)
      return state.current().notification(requestId)
    }, { module: stateModule, requestId: request.requestId })
    expect(notification.readPersisted).toBe(true); coverage.notification = 'EXECUTED'
    const decision = await pageFor('recipient').evaluate(async ({ module, requestId }) => {
      const state = await import(/* @vite-ignore */ module)
      return state.current().decide(requestId, 'INSTAGRAM')
    }, { module: stateModule, requestId: request.requestId })
    expect(decision.reciprocalConsent).toBe(true); coverage.recipientDecision = 'EXECUTED'
    for (const role of ['requester', 'recipient'] as const) {
      const revealed = await pageFor(role).evaluate(async ({ module, requestId }) => {
        const state = await import(/* @vite-ignore */ module)
        return state.current().reveal(requestId, 'INSTAGRAM')
      }, { module: stateModule, requestId: request.requestId })
      expect(revealed.noStore).toBe(true)
    }
    coverage.explicitReveal = 'EXECUTED'
    for (const [role, requestId, statuses, cell] of [
      ['restricted', request.requestId, config.denials.unrelated, 'unrelatedHTTP'],
      ['requester', config.resources.expiredRequestId, config.denials.expired, 'expiredHTTP'],
    ] as const) {
      const denial = await pageFor(role).evaluate(async ({ module, requestId, statuses, expired }) => {
        const state = await import(/* @vite-ignore */ module)
        return expired ? state.current().denyExpiredReveal(requestId, 'INSTAGRAM', statuses)
          : state.current().denyReveal(requestId, 'INSTAGRAM', statuses)
      }, { module: stateModule, requestId, statuses, expired: cell === 'expiredHTTP' })
      expect(denial.denied).toBe(true); coverage[cell] = 'EXECUTED'
    }
    const revoked = await pageFor('recipient').evaluate(async ({ module, requestId }) => {
      const state = await import(/* @vite-ignore */ module)
      return state.current().revoke(requestId)
    }, { module: stateModule, requestId: request.requestId })
    expect(revoked.localWithdrawn).toBe(true); coverage.revokeLocal = 'EXECUTED'
    const denial = await pageFor('requester').evaluate(async ({ module, requestId, statuses }) => {
      const state = await import(/* @vite-ignore */ module)
      return state.current().denyReveal(requestId, 'INSTAGRAM', statuses)
    }, { module: stateModule, requestId: request.requestId, statuses: config.denials.revoked })
    expect(denial.denied).toBe(true); coverage.revokedHTTP = 'EXECUTED'
  } finally {
    try {
      for (const [role, page] of pages) {
        try {
          const http = await page.evaluate(async module => { const state = await import(/* @vite-ignore */ module); return state.http() }, stateModule)
          await testInfo.attach(`http-private-${role}`, { body: JSON.stringify(http), contentType: 'application/json' })
        } finally {
          await page.evaluate(async module => { const state = await import(/* @vite-ignore */ module); state.close() }, stateModule).catch(() => {})
        }
      }
    } finally {
      const closed = await Promise.allSettled(contexts.map(context => context.close()))
      try { await browser.close() }
      finally {
        coverage.contextCleanup = closed.every(value => value.status === 'fulfilled') ? 'EXECUTED' : 'FAILED'
        await testInfo.attach('coverage', { body: JSON.stringify(coverage), contentType: 'application/json' })
      }
    }
  }
})
