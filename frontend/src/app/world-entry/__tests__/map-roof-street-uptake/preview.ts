// TEST ONLY: current admission response around the unchanged actual-main HTTP/C05 fixture.
import { admission } from '../../../../features/auth/__tests__/enrollment-consent/fixtures'
import { authResponse } from '../../../../features/auth/__tests__/fixtures/data'

await import('../art06-default-entry/preview')
if (!import.meta.env.DEV) throw Error('Development-only roof/street fixture')
const fallback = window.fetch.bind(window)
const reads: { method: string; protected: boolean; qualification: string; response: ReturnType<typeof admission> }[] = []
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  if (new URL(request.url).pathname !== '/api/auth/enrollment/admission') return fallback(input, init)
  if (request.method !== 'GET' || request.headers.get('Authorization') !== 'Bearer ' + authResponse().accessToken)
    throw Error('Roof/street fixture requires the canonical protected admission read')
  const response = admission()
  reads.push({ method: request.method, protected: true, qualification: 'SYNTHETIC_ADMITTED_HTTP_RESPONSE_NOT_LIVE', response })
  return Response.json(response)
}
Object.assign(window, { __MCR_ROOF_ADMISSION__: () => structuredClone(reads) })
