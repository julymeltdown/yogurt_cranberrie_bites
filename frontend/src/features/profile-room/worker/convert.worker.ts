/// <reference lib="webworker" />
/**
 * WP-07 T-606: the worker entry. The conversion itself lives in `convert.ts` so
 * the same handler is exercised by the test suite without a real worker.
 */
import { handleConvert, type ConvertReply, type ConvertRequest } from './convert'

const scope = self as unknown as DedicatedWorkerGlobalScope

scope.addEventListener('message', (event: MessageEvent<ConvertRequest>) => {
  const request = event.data
  if (!request || request.type !== 'convert') return
  void handleConvert(request).then((reply: ConvertReply) => {
    // The encoded bytes are transferred, not copied: the source photo never
    // leaves the worker in any other form.
    scope.postMessage(reply, [reply.type === 'converted' ? reply.png : new ArrayBuffer(0)])
  })
})
