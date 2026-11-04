// Self-contained owner adapter; safe to import from verified data-URL bytes.
// The socket is served by the actual managed stack, never a JSON proof fixture.
import { lstat, open } from 'node:fs/promises';
import { constants } from 'node:fs';
import { isAbsolute, join } from 'node:path';
import { createConnection } from 'node:net';

const refuse = () => { throw new Error('IN04_MANAGED_RUNTIME_UNAVAILABLE'); };
function need(value) { if (!value) refuse(); }
async function owned(path, kind) {
  need(isAbsolute(path));
  const st = await lstat(path);
  need(!st.isSymbolicLink() && st.uid === process.getuid() && !(st.mode & 0o077));
  need(kind === 'socket' ? st.isSocket() : kind === 'directory' ? st.isDirectory() : st.isFile());
  return st;
}
const identity = st => [st.dev, st.ino, st.size, st.mtimeMs, st.ctimeMs].join(':');
async function descriptor() {
  const path = process.env.MCR_IN04_BACKEND_DESCRIPTOR;
  need(typeof path === 'string' && isAbsolute(path));
  const before = await owned(path, 'file');
  need(before.size <= 8192);
  const fd = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  let bytes;
  try {
    need(identity(before) === identity(await fd.stat()));
    bytes = await fd.readFile();
    need(bytes.length <= 8192 && identity(before) === identity(await fd.stat()));
  } finally { await fd.close(); }
  need(identity(before) === identity(await lstat(path)));
  const d = JSON.parse(bytes.toString('utf8'));
  need(d.version === 1 && typeof d.owner === 'string' && /^[a-f0-9]{48}$/.test(d.owner));
  const root = await owned(d.root, 'directory');
  need(Array.isArray(d.rootIdentity) && d.rootIdentity[0] === root.dev && d.rootIdentity[1] === root.ino);
  need(d.socket === join(d.root, 'in04-observe.sock'));
  await owned(d.socket, 'socket');
  need(Number.isFinite(d.expiresAtEpoch) && d.expiresAtEpoch * 1000 > Date.now());
  return d;
}
function binding(c) {
  return { candidate: c.releaseRevision, eventId: c.eventId, environmentId: c.environmentId,
    origin: c.origin, domainHash: c.contractHashes.domain, realtimeHash: c.contractHashes.realtime,
    members: c.members.map(a => a.userId), operator: c.operator.userId, restoreSnapshotId: c.restoreSnapshotId };
}
async function observe(c, baseline) {
  const d = await descriptor();
  return await new Promise((resolve, reject) => {
    const socket = createConnection(d.socket);
    let raw = Buffer.alloc(0), settled = false;
    const finish = (error, value) => {
      if (settled) return; settled = true; socket.destroy();
      error ? reject(new Error('IN04_MANAGED_RUNTIME_UNAVAILABLE')) : resolve(value);
    };
    socket.setTimeout(12000, () => finish(true));
    socket.on('error', () => finish(true));
    socket.on('connect', () => socket.end(JSON.stringify(baseline === undefined
      ? { action: 'inspect', owner: d.owner, binding: binding(c) }
      : { action: 'restart-baseline', owner: d.owner, binding: binding(c), baseline }) + '\n'));
    socket.on('data', chunk => {
      raw = Buffer.concat([raw, chunk]);
      if (raw.length > 32768) finish(true);
    });
    socket.on('end', () => {
      try {
        const result = JSON.parse(raw.toString('utf8'));
        need(result.state === 'OBSERVED');
        if (baseline === undefined) {
          need(result.proof.candidate === c.releaseRevision
            && result.proof.environmentId === c.environmentId && result.proof.eventId === c.eventId);
        } else {
          need(Object.keys(result).sort().join(',') === 'generation,state'
            && Object.keys(result.generation).sort().join(',') === 'concertPid,postmasterStartedAt,systemIdentifier'
            && Object.keys(baseline).sort().join(',') === 'concertPid,postmasterStartedAt,systemIdentifier'
            && result.generation.systemIdentifier === baseline.systemIdentifier
            && result.generation.postmasterStartedAt === baseline.postmasterStartedAt
            && result.generation.concertPid === baseline.concertPid);
        }
        finish(false, result);
      } catch { finish(true); }
    });
  });
}
export const runtimeQualification = Object.freeze({
  async inspect(config) { return (await observe(config)).proof; },
  async observeRestart(config, before) {
    need(typeof before.roomEpoch === 'string' && before.roomEpoch.length > 0
      && typeof before.streamId === 'string' && before.streamId.length > 0);
    const started = Date.now(), initial = await observe(config);
    // Notification only: the separately prearmed owner loop decides and queues
    // the actual restart. No retry after an uncertain notification response.
    await observe(config, initial.generation);
    const deadline = Math.min(started + config.restartObservationTimeoutMs, started + 60000);
    while (Date.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 200));
      try {
        const current = await observe(config);
        // Same owned data cluster and pinned binaries, a new real postmaster,
        // and a replacement Concert child. QA independently verifies both WS
        // epochs/stream IDs; no claimed epoch is manufactured here.
        if (current.generation.systemIdentifier === initial.generation.systemIdentifier
          && current.generation.postmasterStartedAt !== initial.generation.postmasterStartedAt
          && current.generation.concertPid !== initial.generation.concertPid) {
          need(Date.parse(current.generation.postmasterStartedAt) > started);
          return { candidate: config.releaseRevision, environmentId: config.environmentId,
            restartedAt: current.generation.postmasterStartedAt };
        }
      } catch { /* During the externally performed restart readiness can fail. */ }
    }
    throw new Error('IN04_EXTERNAL_RESTART_NOT_OBSERVED');
  },
});
