// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { harness, gift, reservation, giftId, memoryJournal } from '../../gift-pickup/__tests__/harness'
import { createGiftPickup } from '../../gift-pickup/model/controller'

it('actual pickup controller retains uncertainty offline and never silently replays owner claim after online/reload', async () => {
  let online = true
  const journal = memoryJournal(), h = harness({ owner: true, journal, handler: r => {
    if (!online) throw new TypeError('Offline synthetic transport')
    return Response.json(new URL(r.url).pathname.endsWith(giftId) ? gift() : reservation())
  } })
  try {
    await h.controller.load(); h.controller.setClaimCode('123456'); online = false
    await h.controller.claim(); expect(h.controller.read().phase).toBe('uncertain')
    expect(h.controller.read().receipt).toBeNull(); expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
    expect(JSON.stringify(journal.peek())).not.toMatch(/123456|token|instructions/)
    online = true; window.dispatchEvent(new Event('online')); await Promise.resolve()
    await h.controller.load(); await h.controller.claim()
    expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
    h.controller.dispose(); const next = createGiftPickup(h.config)
    try { await next.load(); await next.claim(); expect(next.read().phase).toBe('uncertain'); expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1) }
    finally { next.dispose() }
  } finally { h.dispose() }
})
