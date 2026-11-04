import { createClient, schema } from './index.js'
import type { Operations, Schemas } from './types.js'
const client = createClient({ baseUrl: 'https://fixture.invalid' })
// @ts-expect-error unknown operation must not compile
client.call('__UNKNOWN__', {})
// @ts-expect-error unknown named schema must not compile
schema('__UNKNOWN__')
// @ts-expect-error required login body must be present
client.operations.login({})
// @ts-expect-error required provider path parameter must be present
client.operations.receiveAppleOAuthFormPost({ body: {} })
const parsed: Schemas['Bootstrap'] = schema('Bootstrap').parse({})
void parsed
function typed<K extends keyof Operations>(id: K, request: Operations[K]['request']) {
  const response: Promise<Operations[K]['response']> = client.call(id, request)
  return response
}
void typed
