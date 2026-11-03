// Offline helper regression only. Never imported by the real gate or claimed as live evidence.
import { afterEach, describe, expect, it } from 'vitest'
import { EventEmitter } from 'node:events'
import type { Page } from '@playwright/test'
import { mkdtemp, readFile, writeFile, chmod, rm, symlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { loadConfig, parseConfig, sha256 } from './config'
import { WorldObserver } from './observer'
import { decode } from '../../../src/world/network/transport/contract'
import fixture from '../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import type { Snapshot, Delta } from '../../../src/world/network/WorldTransport'
const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const candidate = () => ({ version: 1, isolated: true, origin: 'http://127.0.0.1:4000', eventId: id(1), releaseRevision: 'a'.repeat(40),
  accounts: [{ userId: id(2), email: 'one@example.invalid', password: 'offline-test-password' }, { userId: id(3), email: 'two@example.invalid', password: 'offline-test-password' }],
  serviceReceipt: { path: '/tmp/offline-service-receipt.json', sha256: 'b'.repeat(64) }, artReview: { path: '/tmp/offline-art-review.json', sha256: 'c'.repeat(64) } })
const temporary: string[] = []
afterEach(async () => { await Promise.all(temporary.splice(0).map(p => rm(p, { recursive: true, force: true }))) })
describe('real IN01 precondition parsing', () => {
  it('requires explicitly isolated provisioned accounts and pinned receipts', () => { expect(parseConfig(candidate()).accounts).toHaveLength(2) })
  it.each([undefined, {}, { ...candidate(), isolated: false }, { ...candidate(), releaseRevision: 'main' },
    { ...candidate(), accounts: [candidate().accounts[0], candidate().accounts[0]] },
    { ...candidate(), accounts: [{ ...candidate().accounts[0], password: '' }, candidate().accounts[1]] },
    { ...candidate(), origin: 'http://external.example' }, { ...candidate(), origin: 'https://private:secret@example.test' },
    { ...candidate(), origin: 'https://example.test/path' }, { ...candidate(), mocks: true },
    { ...candidate(), serviceReceipt: { path: 'relative.json', sha256: 'b'.repeat(64) } },
  ])('rejects invalid/unisolated declarations without echoing values', value => {
    expect(() => parseConfig(value)).toThrow(/IN01 NOT_RUN: [A-Z_]+/)
    try { parseConfig(value) } catch (error) { expect(String(error)).not.toContain('secret'); expect(String(error)).not.toContain('offline-test-password') }
  })
  it('requires a private owned file and verifies actual receipt hashes', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'in01-helper-')); temporary.push(directory)
    const receipt = join(directory, 'receipt.json'); await writeFile(receipt, '{"offlineHelperOnly":true}')
    const value = candidate(); value.serviceReceipt = value.artReview = { path: receipt, sha256: sha256(await readFile(receipt)) }
    const config = join(directory, 'config.json'); await writeFile(config, JSON.stringify(value), { mode: 0o600 })
    expect((await loadConfig(config)).origin).toBe(value.origin)
    await chmod(config, 0o644); await expect(loadConfig(config)).rejects.toThrow('CONFIG_MUST_BE_PRIVATE_OWNED_FILE')
    await chmod(config, 0o600); await writeFile(receipt, 'changed'); await expect(loadConfig(config)).rejects.toThrow('RECEIPT_HASH_MISMATCH')
  })
  it('fails absent configuration before allocating browser/network', async () => { await expect(loadConfig('')).rejects.toThrow('MCR_IN01_CONFIG_REQUIRED') })
  it('rejects a symlink configuration and malformed JSON', async () => {
    const directory = await mkdtemp(join(tmpdir(), 'in01-helper-')); temporary.push(directory)
    const target = join(directory, 'target'), link = join(directory, 'link'); await writeFile(target, 'no json', { mode: 0o600 }); await symlink(target, link)
    await expect(loadConfig(link)).rejects.toThrow('CONFIG_MUST_BE_PRIVATE_OWNED_FILE'); await expect(loadConfig(target)).rejects.toThrow('INVALID_CONFIG_JSON')
  })
})
const snapshot = fixture.messages[1]!.message as Snapshot
const delta = fixture.messages.find(m => m.message.type === 'delta')!.message as Delta
describe('passive real-frame observer', () => {
  it('requires valid C05 data and updates only ordered same-stream membership', () => {
    const o = new WorldObserver(decode); o.receive(JSON.stringify(snapshot)); expect(o.players.size).toBe(snapshot.players.length)
    const d = { ...delta, streamId: snapshot.streamId, roomEpoch: snapshot.roomEpoch, streamSeq: 1 }
    o.receive(JSON.stringify(d)); expect(o.streamSeq).toBe(1); expect(o.invalidFrames).toBe(0)
    o.receive(JSON.stringify({ ...d, streamSeq: 3 })); expect(o.players.size).toBe(0); expect(o.baseline).toBeNull(); expect(o.invalidFrames).toBe(1)
    o.receive(JSON.stringify(snapshot)); expect(o.snapshots).toBe(2)
  })
  it('clears foreign epochs and revocation instead of retaining a ghost roster', () => {
    const o = new WorldObserver(decode); o.receive(JSON.stringify(snapshot))
    o.receive(JSON.stringify({ ...delta, streamSeq: 1, streamId: snapshot.streamId, roomEpoch: id(901) })); expect(o.players.size).toBe(0)
    o.receive(JSON.stringify(snapshot)); o.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'revoke', streamId: snapshot.streamId, reason: 'SESSION_REVOKED' })); expect(o.players.size).toBe(0)
  })
  it('rejects invalid frames and erases retained private observations on cleanup', () => {
    const o = new WorldObserver(decode); o.receive('{bad'); expect(o.invalidFrames).toBe(1)
    o.receive(JSON.stringify(snapshot)); o.clear(); expect(o.players.size).toBe(0); expect(o.baseline).toBeNull()
  })
})

describe('runtime validator and passive socket accounting', () => {
  it('counts overlapping real-socket events and ignores frames from a retired socket', () => {
    // EventEmitter is an offline callback driver, not a browser/network mock in the real gate.
    const page = new EventEmitter(), one = new EventEmitter(), two = new EventEmitter(), o = new WorldObserver(decode)
    const socket = (events: EventEmitter) => Object.assign(events, { url: () => 'ws://127.0.0.1:4000/world-ws' })
    o.attach(page as unknown as Page, 'http://127.0.0.1:4000')
    page.emit('websocket', socket(one)); one.emit('framereceived', { payload: JSON.stringify(snapshot) })
    expect(o.active).toBe(1); expect(o.players.size).toBe(snapshot.players.length)
    page.emit('websocket', socket(two)); expect(o.peak).toBe(2)
    one.emit('close'); expect(o.active).toBe(1)
    one.emit('framereceived', { payload: JSON.stringify(snapshot) }); expect(o.snapshots).toBe(1)
    two.emit('close'); expect(o.active).toBe(0); expect(o.players.size).toBe(0)
  })
})
