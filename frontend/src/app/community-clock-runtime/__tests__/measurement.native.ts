/** Actual existing-cohort capture only. Requires its own fresh browser offer.
 * No route interception, synthetic C03, saved browser state or product mutation.
 * Private participant credentials and full Bootstrap bodies are never written. */
import { test, expect } from '@playwright/test'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { decodeBootstrap } from '../../../features/auth/model/bootstrap'
import { canonicalUuid } from '../../../features/auth/model/contract'
import { decodeClockBinding, decodeClockBrowser, deriveClockLimits, type ClockMeasurement, type ClockSample } from '../input'

test('capture genuine C03 intervals in the recorded browser environment', async ({ browser }) => {
  const configPath = process.env.MCR_CLOCK_MEASUREMENT_CONFIG, output = process.env.MCR_CLOCK_MEASUREMENT_OUTPUT
  if (!configPath || !output || !process.env.MCR_CLOCK_BROWSER_OFFER) throw Error('A fresh named offer and private existing-cohort config/output are required')
  const config = JSON.parse(await readFile(configPath, 'utf8')) as {
    binding: unknown; participantFixture: string; sampleOffsetsMs: number[]; observationIntervalMs: number
  }
  expect(Object.keys(config).sort()).toEqual(['binding', 'observationIntervalMs', 'participantFixture', 'sampleOffsetsMs'])
  const binding = decodeClockBinding(config.binding)
  expect(config.participantFixture.startsWith('/')).toBe(true)
  const actor = JSON.parse(await readFile(config.participantFixture, 'utf8')) as { userId: string; eventId: string; accessToken: string }
  expect(canonicalUuid(actor.userId) && actor.eventId === binding.eventId && typeof actor.accessToken === 'string' && actor.accessToken.length > 0).toBe(true)
  expect(config.sampleOffsetsMs.length >= 3 && config.sampleOffsetsMs.length <= 64).toBe(true)
  expect(config.sampleOffsetsMs.every((n, i, a) => Number.isFinite(n) && n >= 0 && n <= 60000 && (!i || n > a[i - 1]!))).toBe(true)
  expect(Number.isFinite(config.observationIntervalMs) && config.observationIntervalMs >= 2).toBe(true)
  const context = await browser.newContext({ serviceWorkers: 'block' })
  try {
    const page = await context.newPage()
    await page.goto(binding.origin, { waitUntil: 'domcontentloaded' })
    expect(new URL(page.url()).origin).toBe(binding.origin)
    const fingerprint = decodeClockBrowser(await page.evaluate(() => ({ userAgent: navigator.userAgent,
      platform: navigator.platform, crossOriginIsolated: window.crossOriginIsolated === true })))
    // The closure stays in browser memory; no global property or auth bytes in evidence.
    const handle = await page.evaluateHandle(({ interval }) => {
      const observationsMs: number[] = [], initialOrigin = performance.timeOrigin
      let uninterrupted = true
      const observe = () => {
        const at = performance.now(), previous = observationsMs.at(-1)
        if (document.visibilityState !== 'visible' || performance.timeOrigin !== initialOrigin || !Number.isFinite(at) || (previous !== undefined && at < previous)) uninterrupted = false
        observationsMs.push(at)
        if (observationsMs.length > 4096) uninterrupted = false
        return at
      }
      const withdraw = () => { uninterrupted = false }
      const visibility = () => { if (document.visibilityState !== 'visible') withdraw() }
      const restored = (event: PageTransitionEvent) => { if (event.persisted) withdraw() }
      document.addEventListener('visibilitychange', visibility)
      document.addEventListener('freeze', withdraw); document.addEventListener('resume', withdraw)
      window.addEventListener('pagehide', withdraw); window.addEventListener('pageshow', restored)
      const started = observe(), timer = setInterval(observe, interval)
      return { started, observe, async request({ eventId, accessToken }: { eventId: string; accessToken: string }) {
        const requestStartedMs = observe()
        const response = await fetch(`/api/concert/events/${eventId}/bootstrap`, { method: 'GET', cache: 'no-store',
          credentials: 'omit', headers: { Authorization: `Bearer ${accessToken}` } })
        if (!response.ok) throw Error('Actual C03 request failed')
        return { requestStartedMs, body: await response.json() as unknown }
      }, finish() {
        observe(); clearInterval(timer)
        document.removeEventListener('visibilitychange', visibility)
        document.removeEventListener('freeze', withdraw); document.removeEventListener('resume', withdraw)
        window.removeEventListener('pagehide', withdraw); window.removeEventListener('pageshow', restored)
        return { observationsMs, uninterrupted }
      } }
    }, { interval: config.observationIntervalMs })
    const samples: ClockSample[] = []
    try {
      for (const offset of config.sampleOffsetsMs) {
        const remaining = await handle.evaluate((state, offset) => state.started + offset - performance.now(), offset)
        if (remaining > 0) await page.waitForTimeout(remaining)
        const response = await handle.evaluate((state, actor) => state.request(actor), actor)
        // Canonical contract/hash/scope decoding completes BEFORE the browser
        // receipt observation; the interval includes this driver round trip.
        const source = await decodeBootstrap(response.body, { userId: actor.userId, eventId: actor.eventId, generation: 1 })
        const responseReceivedMs = await handle.evaluate(state => state.observe())
        expect(typeof source.document?.server_time).toBe('string')
        samples.push({ requestStartedMs: response.requestStartedMs, responseReceivedMs, serverTime: source.document!.server_time as string })
      }
      const continuous = await handle.evaluate(state => state.finish())
      expect(continuous.uninterrupted).toBe(true)
      const evidence = { kind: 'MCR_C03_CLOCK_CAPTURE_V1', binding, browser: fingerprint, samples,
        observationsMs: continuous.observationsMs, uninterrupted: true as const,
        offer: process.env.MCR_CLOCK_BROWSER_OFFER, limitations: 'Conditional finite observed environment; not a universal future drift guarantee.' }
      const bytes = JSON.stringify(evidence, null, 2) + '\n'
      const measurement: ClockMeasurement = { kind: 'MCR_C03_CLOCK_MEASUREMENT_V1', classification: 'MEASURED_RUNTIME',
        evidenceSha256: createHash('sha256').update(bytes).digest('hex'), binding, browser: fingerprint,
        samples, observationsMs: continuous.observationsMs, uninterrupted: true }
      const limits = deriveClockLimits(measurement)
      await mkdir(output, { recursive: true })
      await writeFile(`${output}/capture.json`, bytes)
      await writeFile(`${output}/measurement.json`, JSON.stringify(measurement, null, 2) + '\n')
      await writeFile(`${output}/limits.json`, JSON.stringify(limits, null, 2) + '\n')
    } finally {
      await handle.evaluate(state => state.finish()).catch(() => {})
      await handle.dispose()
    }
  } finally { await context.close() }
})
