import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

export const producerCheckout = resolve(import.meta.dirname, '../../../../../..')
export interface ProducerInputs {
  readonly commit: string; readonly files: Readonly<Record<string, { readonly bytes: number; readonly sha256: string }>>
}
export const producerInputs: ProducerInputs = JSON.parse(readFileSync(new URL('./producer-inputs.json', import.meta.url), 'utf8'))
export const reviewedInputs: ProducerInputs = JSON.parse(readFileSync(new URL('./reviewed-inputs.json', import.meta.url), 'utf8'))
/** Published paths work in a clean clone; pinned Git blobs support the preceding isolated proposal. */
export function readPinnedProducer(path: string, inputs = producerInputs): Uint8Array<ArrayBuffer> {
  const pin = inputs.files[path]
  if (!pin) throw Error('Unpinned producer input')
  const local = resolve(producerCheckout, path)
  const bytes = existsSync(local) ? readFileSync(local)
    : execFileSync('git', ['show', `${inputs.commit}:${path}`], { cwd: producerCheckout })
  if (bytes.byteLength !== pin.bytes || createHash('sha256').update(bytes).digest('hex') !== pin.sha256) throw Error(`Producer pin mismatch: ${path}`)
  return new Uint8Array(bytes)
}
export const readReviewedProducer = (path: string) => readPinnedProducer(path, reviewedInputs)
