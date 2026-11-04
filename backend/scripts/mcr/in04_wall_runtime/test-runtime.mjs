// Protocol/custody tests use an explicitly synthetic diagnostic socket.
// They do not qualify application roles, services, SQL, restart, or IN04.
import assert from 'node:assert/strict';
import { mkdtemp, chmod, writeFile, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:net';
import { stat } from 'node:fs/promises';
import { runtimeQualification as port } from './runtime.mjs';

const root = await mkdtemp(join(tmpdir(), 'mcr-in04-protocol-'));
await chmod(root, 0o700);
const descriptor = join(root, 'descriptor.json'), socket = join(root, 'in04-observe.sock');
const uid = '12345678-1234-4234-8234-123456789abc';
const config = { releaseRevision: 'a'.repeat(40), environmentId: uid, eventId: uid,
  origin: 'http://127.0.0.1:40123', contractHashes: { domain: 'b'.repeat(64), realtime: 'c'.repeat(64) },
  members: [{ userId: uid }, { userId: uid }], operator: { userId: uid },
  restoreSnapshotId: uid, restartObservationTimeoutMs: 1000 };
const owner = 'd'.repeat(48), info = await stat(root);
const record = { version: 1, owner, root, socket, rootIdentity: [info.dev, info.ino], expiresAtEpoch: Date.now() / 1000 + 120 };
let requests = 0, mode = 'valid', notices = 0;
const server = createServer({ allowHalfOpen: true }, connection => {
  let input = '';
  connection.on('data', chunk => { input += chunk; });
  connection.on('end', () => {
    const request = JSON.parse(input); requests++;
    assert(['inspect', 'restart-baseline'].includes(request.action)); assert.equal(request.owner, owner);
    assert.equal(request.binding.candidate, config.releaseRevision);
    assert.equal(request.binding.operator, uid);
    assert(!input.includes('password') && !input.includes('accessToken'));
    if (request.action === 'restart-baseline') {
      notices++;
      assert.deepEqual(Object.keys(request).sort(), ['action', 'baseline', 'binding', 'owner']);
      assert.equal(request.baseline.concertPid, 42);
      connection.end(mode === 'baseline-refused' ? '{"state":"UNAVAILABLE"}\n' : JSON.stringify({
        state: 'OBSERVED', generation: mode === 'baseline-mismatch'
          ? { ...request.baseline, concertPid: 999 } : request.baseline }) + '\n');
      return;
    }
    const proof = { candidate: mode === 'mismatch' ? 'e'.repeat(40) : config.releaseRevision,
      environmentId: uid, eventId: uid };
    connection.end(mode === 'unavailable' ? '{"state":"UNAVAILABLE"}\n' : JSON.stringify({ state: 'OBSERVED', proof,
      generation: { systemIdentifier: 'synthetic-protocol-test', concertPid: 42, postmasterStartedAt: '2026-01-01T00:00:00Z' } }) + '\n');
  });
});
await new Promise(resolve => server.listen(socket, resolve)); await chmod(socket, 0o600);
const save = async value => { await writeFile(descriptor, JSON.stringify(value), { mode: 0o600 }); await chmod(descriptor, 0o600); };
process.env.MCR_IN04_BACKEND_DESCRIPTOR = descriptor;
try {
  await save(record); assert.equal((await port.inspect(config)).candidate, config.releaseRevision);
  assert.equal(requests, 1);
  for (const bad of [{ ...record, expiresAtEpoch: 1 }, { ...record, rootIdentity: [0, 0] }, { ...record, socket: join(root, 'foreign.sock') }]) {
    await save(bad); await assert.rejects(port.inspect(config));
  }
  assert.equal(requests, 1, 'invalid custody must refuse before socket access');
  await save(record); await chmod(descriptor, 0o644); await assert.rejects(port.inspect(config));
  await chmod(descriptor, 0o600);
  const alias = join(root, 'alias.json'); await symlink(descriptor, alias);
  process.env.MCR_IN04_BACKEND_DESCRIPTOR = alias; await assert.rejects(port.inspect(config));
  process.env.MCR_IN04_BACKEND_DESCRIPTOR = descriptor;
  for (const value of ['mismatch', 'unavailable']) { mode = value; await assert.rejects(port.inspect(config)); }
  mode = 'valid';
  await assert.rejects(port.observeRestart(config, { roomEpoch: 'actual-test-input', streamId: 'actual-test-input' }), /EXTERNAL_RESTART_NOT_OBSERVED/);
  assert.equal(notices, 1, 'one notification, no restart command and no retry');
  for (const failure of ['baseline-refused', 'baseline-mismatch']) {
    mode = failure;
    const beforeRequests = requests, beforeNotices = notices;
    await assert.rejects(port.observeRestart(config, { roomEpoch: 'fixture', streamId: 'fixture' }), /MANAGED_RUNTIME_UNAVAILABLE/);
    assert.equal(notices, beforeNotices + 1, 'uncertain notification is never resent');
    assert.equal(requests, beforeRequests + 2, 'no polling after failed notification');
  }
  console.log('PASS: valid protocol; expired/replaced/foreign/public/symlink refusal; live identity/unavailability refusal; unchanged restart refused. Synthetic protocol only.');
} finally {
  delete process.env.MCR_IN04_BACKEND_DESCRIPTOR;
  await new Promise(resolve => server.close(resolve)); await rm(root, { recursive: true });
}
