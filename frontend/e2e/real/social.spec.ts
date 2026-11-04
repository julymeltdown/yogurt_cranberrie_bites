import { test, expect } from '@playwright/test'
import { admitHTTP, readRunConfig } from './social/admission.mjs'

/** Requires a separately allocated same-origin Vite/backend workflow. Never launches a server. */
test('IN03 real HTTP controller preparation (full IN03 remains NOT_RUN)', async ({ browser }, testInfo) => {
  const config = readRunConfig(process.env.MCR_C07_REAL_CONFIG)
  const admission = admitHTTP(config)
  await testInfo.attach('admission', { body: JSON.stringify(admission), contentType: 'application/json' })
  if (admission.status !== 'INPUTS_PRESENT') throw new Error('NOT_RUN: see redacted admission prerequisites; fixture fallback is forbidden')
  const contexts = []
  try {
    for (const role of ['requester', 'recipient', 'restricted'] as const) {
      const context = await browser.newContext(); contexts.push(context)
      const page = await context.newPage(); await page.goto(config.origin)
      // Dynamic imports use the separately admitted same-origin deployment; never inject tokens/session state.
      const summary = await page.evaluate(async ({ config, role }) => {
        const entryPath = '/e2e/real/social/real-entry.ts'
        const { openRealSocial, waitForController } = await import(/* @vite-ignore */ entryPath)
        const producer = await import(/* @vite-ignore */ config.availabilityModule)
        if (producer.sourceRevision !== config.availabilityRevision || typeof producer.availability !== 'function') throw new Error('NOT_RUN: availability producer revision/export mismatch')
        const harness = await openRealSocial({ ...config.principals[role], eventId: config.eventId }, producer.availability)
        try {
          const { owner, sharing } = harness
          for (const service of ['people', 'feed', 'inbox', 'requests']) if (!owner.available(service)) throw new Error('NOT_RUN: social service unavailable')
          owner.people.start(); await waitForController(() => owner.people.read().page.phase, owner.people.subscribe)
          owner.feed.start(); await waitForController(() => owner.feed.read().feed.phase, owner.feed.subscribe)
          owner.inbox.start(); await waitForController(() => owner.inbox.read().phase, owner.inbox.subscribe)
          if (role === 'restricted') {
            await sharing.loadRequest(config.resources.approvedRequestId)
            if (await sharing.revealHandle('INSTAGRAM') || sharing.read().secret.data) throw new Error('Restricted principal revealed a private handle')
          }
          return { people: owner.people.read().page.phase, feed: owner.feed.read().feed.phase, inbox: owner.inbox.read().phase, secret: sharing.read().secret.data !== null }
        } finally { harness.dispose() }
      }, { config, role })
      expect(summary.secret).toBe(false)
      if (role !== 'restricted') { expect(summary.people).toBe('ready'); expect(summary.feed).toBe('ready'); expect(summary.inbox).toBe('ready') }
    }
  } finally { for (const context of contexts) await context.close() }
  await testInfo.attach('coverage', { body: JSON.stringify({ HTTP_PREPARATION: 'EXECUTED', IN03: 'NOT_RUN', missing: admission.unavailable }), contentType: 'application/json' })
})
