import { afterEach, expect, it, vi } from 'vitest'
import { observePublicHTTP } from '../../../../e2e/real/social/observations'

afterEach(() => { vi.unstubAllGlobals() })
const origin = 'http://127.0.0.1:43127'
const path = '/api/concert/events/9ab386e2-a82a-41e4-829d-000000000010/people/9ab386e2-a82a-41e4-829d-000000000002'

it('delays the actual supplied upstream response without substituting its body/status or retaining private data in observations', async () => {
  const upstream = Response.json({ privateCanary: 'unit-private-profile' }, { headers: { 'cache-control': 'no-store' } })
  const transport = vi.fn(async () => upstream)
  vi.stubGlobal('location', { origin }); vi.stubGlobal('fetch', transport)
  const observer = observePublicHTTP(), hold = observer.holdNextProfileRead(path)
  let delivered = false
  const pending = observer.fetch(new Request(origin + path)).then(response => { delivered = true; return response })
  try {
    await vi.waitFor(() => { expect(hold.read()).toEqual({ received: true, status: 200 }) })
    expect(transport).toHaveBeenCalledTimes(1); expect(delivered).toBe(false)
    expect(observer.read()).toEqual([{ sequence: 1, method: 'GET', path, status: 200, noStore: true }])
    expect(JSON.stringify(observer.read())).not.toContain('unit-private-profile')
    hold.release(); expect(await pending).toBe(upstream)
    expect(delivered).toBe(true)
  } finally { observer.dispose(); await pending }
})

it('releases a held response on disposal and rejects a competing hold or mutation endpoint', async () => {
  vi.stubGlobal('location', { origin }); vi.stubGlobal('fetch', async () => new Response(null, { status: 404 }))
  const observer = observePublicHTTP(), hold = observer.holdNextProfileRead(path)
  const pending = observer.fetch(new Request(origin + path))
  try {
    expect(() => observer.holdNextProfileRead(path)).toThrow()
    await vi.waitFor(() => { expect(hold.read().received).toBe(true) })
    observer.dispose(); expect((await pending).status).toBe(404)
    expect(() => observer.holdNextProfileRead(path + '/decision')).toThrow()
  } finally { hold.release(); observer.dispose(); await pending }
})
