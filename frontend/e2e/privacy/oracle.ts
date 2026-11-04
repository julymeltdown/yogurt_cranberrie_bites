/** Test oracle only: never a replacement for server authorization or response decoding. */
export const roles = ['owner', 'member', 'stranger', 'blocked', 'suspended', 'expired', 'admin'] as const
export const operations = ['list', 'detail', 'mutation', 'WS', 'cache'] as const
export type Role = typeof roles[number]
export type Operation = typeof operations[number]
export function check(condition: unknown, label: string): asserts condition {
  if (!condition) throw new Error(`QA04 assertion: ${label}`)
}
export const markers = ['qa04_private_handle', 'qa04_private_claim', 'qa04_private_evidence', 'qa04_private_beacon', 'qa04_private_recipient'] as const
export function assertAbsent(value: unknown, forbidden: readonly string[] = markers) {
  const serialized = JSON.stringify(value) ?? ''
  check(forbidden.every(marker => !serialized.includes(marker)), 'unauthorized private marker retained')
}
export interface Cell {
  role: Role; operation: Operation
  status: 'PASS' | 'UNSUPPORTED'
  evidence: string
  provenance: 'SYNTHETIC_FIXTURE'
}
export function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(r => { resolve = r })
  return { promise, resolve }
}
export async function until(predicate: () => boolean) {
  for (let i = 0; i < 100; i++) {
    if (predicate()) return
    await new Promise(r => setTimeout(r, 5))
  }
  throw new Error('QA04 condition timed out')
}
