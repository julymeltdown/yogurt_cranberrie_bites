import { createHash } from 'node:crypto'
import { lstat, readFile } from 'node:fs/promises'
import { resolve, isAbsolute } from 'node:path'
export class Prerequisite extends Error {
  readonly code: string
  constructor(code: string) { super(`IN01 NOT_RUN: ${code}`); this.code = code }
}
export interface Account { email: string; password: string; userId: string }
export interface GateConfig {
  version: 1; isolated: true; origin: string; eventId: string; releaseRevision: string
  accounts: [Account, Account]
  serviceReceipt: { path: string; sha256: string }
  artReview: { path: string; sha256: string }
}
const uuid = (v: unknown): v is string => typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v)
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)
const exact = (v: Record<string, unknown>, keys: string[]) => Object.keys(v).length === keys.length && keys.every(k => Object.hasOwn(v, k))
function requireValue(v: unknown, code: string): asserts v { if (!v) throw new Prerequisite(code) }
export const sha256 = (bytes: Uint8Array | string) => createHash('sha256').update(bytes).digest('hex')
export function parseConfig(value: unknown): GateConfig {
  requireValue(record(value) && exact(value, ['version', 'isolated', 'origin', 'eventId', 'releaseRevision', 'accounts', 'serviceReceipt', 'artReview']), 'INVALID_CONFIG')
  const v = value as Record<string, unknown>
  requireValue(v.version === 1 && v.isolated === true && uuid(v.eventId) && typeof v.releaseRevision === 'string' && /^[0-9a-f]{40}$/.test(v.releaseRevision), 'INVALID_RELEASE_DECLARATION')
  let url: URL
  try { url = new URL(String(v.origin)) } catch { throw new Prerequisite('INVALID_ORIGIN') }
  requireValue(url.origin === v.origin && !url.username && !url.password && (url.protocol === 'https:' || (url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname))), 'INVALID_ORIGIN')
  requireValue(Array.isArray(v.accounts) && v.accounts.length === 2, 'TWO_PROVISIONED_ACCOUNTS_REQUIRED')
  const accounts = v.accounts as unknown[]
  for (const a of accounts) requireValue(record(a) && exact(a, ['email', 'password', 'userId']) && uuid(a.userId) && typeof a.email === 'string' && /^[^\s@]+@[^\s@]+$/.test(a.email) && typeof a.password === 'string' && a.password.length >= 8 && a.password.length <= 512, 'INVALID_ACCOUNT')
  const [a, b] = accounts as [Account, Account]
  requireValue(a.userId !== b.userId && a.email.toLowerCase() !== b.email.toLowerCase(), 'ACCOUNTS_MUST_BE_INDEPENDENT')
  for (const receipt of [v.serviceReceipt, v.artReview]) requireValue(record(receipt) && exact(receipt, ['path', 'sha256']) && typeof receipt.path === 'string' && isAbsolute(receipt.path) && typeof receipt.sha256 === 'string' && /^[0-9a-f]{64}$/.test(receipt.sha256), 'PINNED_RECEIPTS_REQUIRED')
  return v as unknown as GateConfig
}
export async function loadConfig(path = process.env.MCR_IN01_CONFIG): Promise<GateConfig> {
  requireValue(path && isAbsolute(path), 'MCR_IN01_CONFIG_REQUIRED')
  let bytes: Buffer
  try {
    const info = await lstat(path)
    requireValue(info.isFile() && !info.isSymbolicLink() && info.uid === process.getuid?.() && !(info.mode & 0o077) && info.size <= 16384, 'CONFIG_MUST_BE_PRIVATE_OWNED_FILE')
    bytes = await readFile(path)
  } catch (error) { if (error instanceof Prerequisite) throw error; throw new Prerequisite('CONFIG_UNREADABLE') }
  let parsed: unknown
  try { parsed = JSON.parse(bytes.toString('utf8')) } catch { throw new Prerequisite('INVALID_CONFIG_JSON') }
  const config = parseConfig(parsed)
  for (const receipt of [config.serviceReceipt, config.artReview]) {
    try { const info = await lstat(receipt.path); requireValue(info.isFile() && !info.isSymbolicLink() && info.size < 4 * 1024 * 1024 && sha256(await readFile(resolve(receipt.path))) === receipt.sha256, 'RECEIPT_HASH_MISMATCH') }
    catch (error) { if (error instanceof Prerequisite) throw error; throw new Prerequisite('RECEIPT_UNREADABLE') }
  }
  return config
}
