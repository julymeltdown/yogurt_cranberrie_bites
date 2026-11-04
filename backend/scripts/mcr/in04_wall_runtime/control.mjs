// Private QA05 owner control. Only Node builtins: verified data-URL import safe.
import { lstat, open } from 'node:fs/promises';
import { constants } from 'node:fs';
import { isAbsolute, join } from 'node:path';
import { createConnection } from 'node:net';

const fail = () => { throw new Error('QA05_OWNED_CONTROL_UNAVAILABLE'); };
const need = value => { if (!value) fail(); };
const identity = s => [s.dev, s.ino, s.size, s.mtimeMs, s.ctimeMs].join(':');
async function owned(path, kind) {
  need(typeof path === 'string' && isAbsolute(path));
  const s = await lstat(path);
  need(!s.isSymbolicLink() && s.uid === process.getuid() && !(s.mode & 0o077));
  need(kind === 'directory' ? s.isDirectory() : kind === 'socket' ? s.isSocket() : s.isFile());
  return s;
}
const equal = (a, b) => {
  if (a === b) return true;
  if (!a || !b || typeof a !== 'object' || typeof b !== 'object') return false;
  const ak = Object.keys(a).sort(), bk = Object.keys(b).sort();
  return ak.length === bk.length && ak.every((k, i) => k === bk[i] && equal(a[k], b[k]));
};
function binding(c) {
  need(c && Array.isArray(c.members) && c.members.length === 2);
  return { candidate: c.releaseRevision, environmentId: c.environmentId, eventId: c.eventId,
    windowId: c.windowId, origin: c.origin, domainHash: c.contractHashes.domain,
    realtimeHash: c.contractHashes.realtime, members: c.members.map(a => a.userId) };
}
async function descriptor(c) {
  const path = process.env.MCR_QA05_BACKEND_DESCRIPTOR;
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
  need(d.version === 1 && /^[a-f0-9]{48}$/.test(d.owner) && equal(d.binding, binding(c)));
  const root = await owned(d.root, 'directory'), socket = await owned(d.socket, 'socket');
  need(Array.isArray(d.rootIdentity) && equal(d.rootIdentity, [root.dev, root.ino]));
  need(d.socket === join(d.root, 'qa05-control.sock') && Array.isArray(d.socketIdentity)
    && equal(d.socketIdentity, [socket.dev, socket.ino]));
  need(Number.isFinite(d.expiresAtEpoch) && d.expiresAtEpoch * 1000 > Date.now()
    && d.expiresAtEpoch * 1000 <= Date.now() + 900000);
  return d;
}
async function call(c, action, payload = {}) {
  try {
    const d = await descriptor(c), bound = binding(c);
    return await new Promise((resolve, reject) => {
      const socket = createConnection(d.socket);
      let raw = Buffer.alloc(0), settled = false;
      const finish = (error, value) => {
        if (settled) return;
        settled = true; socket.destroy();
        error ? reject(new Error('QA05_OWNED_CONTROL_UNAVAILABLE')) : resolve(value);
      };
      // The owner never blocks this world-only socket for lease expiry. PG mode
      // may stop/start the incumbent cluster, within its explicitly larger bound.
      const timeout = action === 'requestPostgresConcertRestart' || action === 'checkpoint' ? 90000 : 12000;
      socket.setTimeout(timeout, () => finish(true));
      const expiry = setTimeout(() => finish(true), Math.max(1, d.expiresAtEpoch * 1000 - Date.now()));
      socket.on('close', () => clearTimeout(expiry));
      socket.on('error', () => finish(true));
      socket.on('connect', () => socket.end(JSON.stringify({ action, owner: d.owner, binding: bound,
        ...payload }) + '\n'));
      socket.on('data', bytes => {
        if (raw.length + bytes.length > 32768) { finish(true); return; }
        raw = Buffer.concat([raw, bytes]);
      });
      socket.on('end', () => {
        try {
          const result = JSON.parse(raw.toString('utf8'));
          need(equal(result.binding, bound) && d.expiresAtEpoch * 1000 > Date.now());
          if (action === 'inspect') {
            need(result.state === 'OBSERVED' && result.generation && Array.isArray(result.rooms)
              && Array.isArray(result.memberships));
          } else {
            need(result.requestId === payload.requestId
              && ['WORLD_RESTART', 'POSTGRES_CONCERT_RESTART'].includes(result.action)
              && ['QUEUED', 'STOPPING', 'LEASE_WAIT', 'REPLACEMENT_STARTING', 'COMPLETE', 'FAILED'].includes(result.state));
            if (action === 'requestWorldRestart') need(result.action === 'WORLD_RESTART');
            if (action === 'requestPostgresConcertRestart') need(result.action === 'POSTGRES_CONCERT_RESTART');
          }
          finish(false, result);
        } catch { finish(true); }
      });
    });
  } catch { fail(); }
}
function request(c, action, value) {
  need(value && Object.keys(value).sort().join(',') === 'beforeGeneration,requestId'
    && typeof value.requestId === 'string' && /^[A-Za-z0-9:_-]{1,128}$/.test(value.requestId)
    && value.beforeGeneration && typeof value.beforeGeneration === 'object');
  return call(c, action, { requestId: value.requestId, beforeGeneration: value.beforeGeneration });
}
export const ownerControl = Object.freeze({
  inspect: c => call(c, 'inspect'),
  requestWorldRestart: (c, value) => request(c, 'requestWorldRestart', value),
  requestPostgresConcertRestart: (c, value) => request(c, 'requestPostgresConcertRestart', value),
  checkpoint: (c, requestId) => call(c, 'checkpoint', { requestId }),
});
