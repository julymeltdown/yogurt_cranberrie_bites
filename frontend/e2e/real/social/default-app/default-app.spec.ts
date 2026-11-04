import { test, expect, type BrowserContext } from '@playwright/test'
import { randomUUID } from 'node:crypto'
import { writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'
import { bindServingTree, frozenSource, servingTreeCurrent } from './source.mjs'
import { readDefaultInputs } from './serve.mjs'
import { observeDefaultApp } from './observe'
import { DefaultAppUI } from './ui'
import { SocialUI, type SocialScenario } from './social-ui'

/** Actual default app only. No old browser-state import, route mock, global API or second identity. */
test('default application real UI segments; original full gate stays NOT_RUN', async ({ playwright }) => {
  const bound = readDefaultInputs(), repository = resolve(fileURLToPath(new URL('.', import.meta.url)), '../../../../..')
  expect(execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim()).toBe(bound.input.harnessRevision)
  expect(execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim()).toBe('')
  const source = bindServingTree(frozenSource(repository, bound.input.publishedRevision), bound.input.servedRoot)
  const main = source.files.find((file: { path: string }) => file.path === 'frontend/src/main.tsx')!
  const world = new URL('/world-ws', bound.serving.origin); world.protocol = world.protocol === 'https:' ? 'wss:' : 'ws:'
  const worldURL = world.href
  test.setTimeout(bound.input.timeoutMs)
  const browser = await playwright.chromium.launch({ channel: 'chrome' })
  const contexts: BrowserContext[] = [], actors: { ui: DefaultAppUI; observation: ReturnType<typeof observeDefaultApp> }[] = []
  const coverage: Record<string, string> = { originalIN03: 'NOT_RUN', policyPeerPropagation: 'NOT_RUN', reconnect: 'NOT_RUN',
    restrictedAge: 'NOT_RUN', introPositive: 'NOT_RUN', sharingPositive: 'NOT_RUN', privateCache: 'CACHE_NOT_RUN', signout: 'NOT_RUN' }
  const cleanupResources: ({ commentId: string; persisted: boolean } | { requestId: string; terminal: string | null } | { blockTargetId: string; persisted: boolean })[] = []
  let expired = false
  const deadline = setTimeout(() => { expired = true; void browser.close().catch(() => {}) }, Math.max(1, bound.cutoff - Date.now() - 5000))
  try {
    for (const role of ['requester', 'recipient', 'restricted'] as const) {
      const context = await browser.newContext({ viewport: { width: 390, height: 843 } }); contexts.push(context)
      const page = await context.newPage(); page.setDefaultTimeout(15_000)
      const principal = bound.principals[role]
      const observation = observeDefaultApp(page, { origin: bound.serving.origin, eventId: bound.eventId,
        userId: principal.userId, worldURL, source: { revision: source.revision, mainSha256: main.sha256 } })
      const ui = new DefaultAppUI(page, observation.feedIds, principal.userId); actors.push({ ui, observation })
      await ui.signIn(bound.serving.origin, principal)
      await ui.rememberWorld()
      await expect.poll(() => observation.read().bootstrapBound).toBeGreaterThan(0)
      await expect.poll(() => observation.read().matchedBaselines).toBeGreaterThan(0)
      if (bound.input.cases.includes('feed-read')) {
        await expect.poll(() => observation.read().rows.filter((row: { kind?: string; path?: string; status?: number }) => row.kind === 'http' && row.path === '/api/concert/events/{id}/feed' && row.status === 200).length).toBeGreaterThan(0)
        await expect(page.getByRole('region', { name: 'Event feed' })).toBeVisible()
        coverage[`${role}:feedRead`] = 'EXECUTED'
      }
      await ui.link('/me'); await ui.link('/feed')
      coverage[`${role}:sameWorldNavigation`] = 'EXECUTED'
      for (const kind of ['intro', 'people', 'sharing', 'notification'] as const) {
        if (bound.input.cases.includes(`${kind}-unavailable`)) {
          await ui.unavailable(kind); coverage[`${role}:${kind}CompositionUnavailable`] = 'EXECUTED'
        }
      }
      if (role === 'requester' && bound.input.cases.includes('feed-comment')) {
        cleanupResources.push(await ui.comment(bound.resources.postId, `IN03 default app ${randomUUID()}`))
        coverage.actualUICommentReadback = 'EXECUTED'
      }
      await observation.drain()
      const result = observation.read()
      expect(result.mainLoads).toBe(1); expect(result.loginRequests).toBe(1); expect(result.oldEntryRequests).toBe(0)
      expect(result.peakWorld).toBe(1); expect(result.worldOpened).toBe(1); expect(result.liveWorld).toBe(1)
      expect(result.overflow || result.inspectionsFailed).toBe(false); expect(result.violations).toEqual([])
    }
    if (bound.social) {
      const requester = actors[0]!, recipient = actors[1]!, scenario = bound.social as SocialScenario
      const requesterUI = new SocialUI(requester.ui, bound.eventId, bound.principals.requester.userId, bound.principals.recipient.userId, scenario)
      const recipientUI = new SocialUI(recipient.ui, bound.eventId, bound.principals.requester.userId, bound.principals.recipient.userId, scenario)
      const id = await requesterUI.create(), cleanup = { requestId: id, terminal: null as string | null }
      cleanupResources.push(cleanup); coverage.introDiscoveryAndDetail = 'EXECUTED'; coverage.requesterConsent = 'EXECUTED'
      await recipientUI.notification(id, true); coverage.notificationPersistedReadAndTarget = 'EXECUTED'
      await recipientUI.open(id, 'INCOMING', 'PENDING'); await recipientUI.approve(id)
      await requesterUI.open(id, 'OUTGOING', 'APPROVED'); await recipientUI.open(id, 'INCOMING', 'APPROVED')
      await requesterUI.reveal(id, bound.principals.recipient.userId, scenario.recipientHandle)
      await recipientUI.reveal(id, bound.principals.requester.userId, scenario.requesterHandle)
      coverage.reciprocalDecisionAndExplicitReveal = 'EXECUTED'
      const baselines = requester.observation.read().matchedBaselines
      try {
        await contexts[0]!.setOffline(true)
        await expect.poll(() => requester.observation.read().liveWorld).toBe(0)
        await requesterUI.withdrawn()
      } finally { await contexts[0]!.setOffline(false) }
      await expect.poll(() => requester.observation.read().matchedBaselines).toBeGreaterThan(baselines)
      await expect.poll(() => requester.observation.read().liveWorld).toBe(1)
      expect(requester.observation.read().protectedReadsBeforeBaseline).toBe(0)
      await requesterUI.open(id, 'OUTGOING', 'APPROVED')
      await requesterUI.reveal(id, bound.principals.recipient.userId, scenario.recipientHandle)
      coverage.reconnectAndBaselineReadOrdering = 'EXECUTED'
      const invalidations = requester.observation.read().consentInvalidations
      await recipientUI.revoke(id)
      await expect.poll(() => requester.observation.read().consentInvalidations).toBeGreaterThan(invalidations)
      await requesterUI.withdrawn(); coverage.policyPeerPropagation = 'EXECUTED'
      await recipientUI.open(id, 'INCOMING', 'REVOKED'); await requesterUI.open(id, 'OUTGOING', 'REVOKED')
      cleanup.terminal = 'REVOKED'; coverage.persistedRevocationAndDOMWithdrawal = 'EXECUTED'
      const blockId = await requesterUI.create()
      cleanupResources.push({ requestId: blockId, terminal: null })
      await recipientUI.open(blockId, 'INCOMING', 'PENDING'); await recipientUI.approve(blockId)
      await recipientUI.open(blockId, 'INCOMING', 'APPROVED')
      await recipientUI.reveal(blockId, bound.principals.requester.userId, scenario.requesterHandle)
      const blockInvalidations = recipient.observation.read().blockInvalidations
      await requesterUI.blockRecipient()
      cleanupResources.push({ blockTargetId: bound.principals.recipient.userId, persisted: true })
      await expect.poll(() => recipient.observation.read().blockInvalidations).toBeGreaterThan(blockInvalidations)
      await recipientUI.withdrawn(); coverage.persistedBlockAndPeerDOMWithdrawal = 'EXECUTED'
    }
    for (const [index, actor] of actors.entries()) {
      await actor.ui.sameWorld(); await actor.observation.drain()
      const result = actor.observation.read()
      expect(result.mainLoads).toBe(1); expect(result.loginRequests).toBe(1)
      expect(result.peakWorld).toBe(1); expect(result.worldOpened).toBe(bound.social && index === 0 ? 2 : 1); expect(result.liveWorld).toBe(1)
      expect(result.protectedReadsBeforeBaseline).toBe(0)
      expect(result.overflow || result.inspectionsFailed).toBe(false); expect(result.violations).toEqual([])
    }
    if (bound.social) {
      for (const actor of actors) {
        await actor.ui.signOut()
        await expect.poll(() => actor.observation.read().liveWorld).toBe(0)
      }
      coverage.signoutDOMAndSocketClosure = 'EXECUTED'
    }
    expect(expired).toBe(false); expect(servingTreeCurrent(source)).toBe(true)
  } finally {
    clearTimeout(deadline)
    for (const actor of actors) await actor.ui.disposeHandles().catch(() => {})
    const closed = await Promise.allSettled(contexts.map(context => context.close()))
    await browser.close().catch(() => {})
    for (const actor of actors) { await actor.observation.drain(); actor.observation.dispose() }
    const result = { actualIN03: 'NOT_RUN', coverage, expired, contextCleanup: closed.every(value => value.status === 'fulfilled'), cleanupResources,
      executionRevision: source.revision, frozenBridge: source.frozenBridge, sourceManifestSha256: source.manifestSha256,
      observations: actors.map(actor => actor.observation.read()),
      limits: ['CACHE_NOT_RUN: no qualified same-identity live cache observation interface.',
        'Source-authored social-ui steps require actual owner resources and execution; EXECUTED is a subset, not whole IN03.',
        'No restricted-age, unrelated/expired/revoked/blocked HTTP-denial, delayed-response or private-cache qualification.',
        'One Auth construction follows frozen source; observed login/World counts are recorded, not private object counts.'] }
    await writeFile(resolve(process.env.MCR_DEFAULT_APP_ARTIFACTS!, 'default-app-private.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx', mode: 0o600 })
  }
})
