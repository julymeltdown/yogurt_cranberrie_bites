import { vi } from 'vitest'

// The retained feature fixtures predate mandatory Auth admission. Supply its current
// canonical HTTP response while keeping real SessionController/request/identity paths.
// Feature request handlers and their request counts remain unchanged.
vi.mock('../../../../auth/model/session', async importOriginal => {
  const actual = await importOriginal<typeof import('../../../../auth/model/session')>()
  const { admission } = await import('../../../../auth/__tests__/enrollment-consent/fixtures')
  return { ...actual, createSessionController(options: Parameters<typeof actual.createSessionController>[0]) {
    const fetch = options.fetch ?? globalThis.fetch
    return actual.createSessionController({ ...options, fetch: (input, init) => {
      const request = input instanceof Request ? input : new Request(input, init)
      return new URL(request.url).pathname === '/api/auth/enrollment/admission'
        ? Promise.resolve(Response.json(admission())) : fetch(input, init)
    } })
  } }
})
