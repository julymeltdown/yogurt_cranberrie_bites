import { createRequire } from 'node:module';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';

const root = '/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme/frontend';
const require = createRequire(root + '/package.json');
const { createServer } = await import(require.resolve('vite'));
const { JSDOM } = require('jsdom');

const dom = new JSDOM('<!doctype html><body></body>', { url: 'http://localhost/' });
for (const key of ['window', 'document', 'HTMLElement', 'HTMLDivElement', 'HTMLImageElement', 'Event', 'KeyboardEvent', 'MouseEvent', 'AbortController', 'URL', 'Blob']) {
  globalThis[key] = dom.window[key];
}
globalThis.requestAnimationFrame = () => 1;
globalThis.cancelAnimationFrame = () => {};
globalThis.ResizeObserver = class { observe() {} disconnect() {} };

const dev = await createServer({
  root,
  configFile: false,
  cacheDir: '/tmp/mcr-qa-fe06-agy8u3i5/cache',
  server: { watch: null },
  optimizeDeps: { noDiscovery: true }
});

const report = {};

try {
  const { validateTiled, THEME_IMAGE_MARKER } = await dev.ssrLoadModule('/src/world/map/tiled.ts');
  const { assertSameWorld } = await dev.ssrLoadModule('/src/world/theme/compatibility.ts');
  const { createLocalThemePreferences } = await dev.ssrLoadModule('/src/features/theme/model/preferences.ts');
  const { createWorldRuntime } = await dev.ssrLoadModule('/src/world/runtime.ts');
  const { synthetic } = await dev.ssrLoadModule('/src/world/__tests__/fixtures/synthetic.ts');

  // --- 1. Tiled Validator Probes ---
  const syn = await synthetic();
  const baseTiled = syn.tiled;
  const geometry = syn.map.geometry;
  const texture = syn.theme.texture;

  function makeTiled(imagePath, extra = {}) {
    const copy = structuredClone(baseTiled);
    copy.tilesets[0].image = imagePath;
    Object.assign(copy.tilesets[0], extra);
    return JSON.stringify(copy);
  }

  // A. Marker accepted
  validateTiled(makeTiled(THEME_IMAGE_MARKER), geometry, texture);
  // B. Exact texture path accepted
  validateTiled(makeTiled(texture.path), geometry, texture);

  // C. Disallowed image paths rejected
  const rejectedPaths = ['world/parade.png', '../evil.png', 'http://evil.com/t.png', '', 'mcr-theme.jpg', 'world/mcr-theme.png'];
  for (const p of rejectedPaths) {
    assert.throws(() => validateTiled(makeTiled(p), geometry, texture), /MapError|Map/i, `Should reject ${p}`);
  }

  // D. Disallowed geometry/tileset configs rejected
  assert.throws(() => validateTiled(makeTiled(THEME_IMAGE_MARKER, { columns: 4 }), geometry, texture));
  assert.throws(() => validateTiled(makeTiled(THEME_IMAGE_MARKER, { tilecount: 4 }), geometry, texture));
  assert.throws(() => validateTiled(makeTiled(THEME_IMAGE_MARKER, { firstgid: 2 }), geometry, texture));
  report.tiledValidator = 'PASS';

  // --- 2. C06 Immutable Map Identity & assertSameWorld Probes ---
  const current = {
    map: syn.map,
    theme: syn.theme,
    widthPx: 160,
    heightPx: 160,
    spawn: syn.map.geometry.spawns[0]
  };

  // Same map, different theme_id -> PASS
  assertSameWorld(current, { ...current, theme: { ...syn.theme, theme_id: 'PARADE' } });

  // Altered map content_hash -> FAILS
  assert.throws(() => assertSameWorld(current, { ...current, map: { ...syn.map, content_hash: 'sha256:different' } }));
  // Altered tilemap path -> FAILS
  assert.throws(() => assertSameWorld(current, { ...current, map: { ...syn.map, tilemap: { ...syn.map.tilemap, path: 'world/other.json' } } }));
  // Altered avatar manifest id -> FAILS
  assert.throws(() => assertSameWorld(current, { ...current, theme: { ...syn.theme, avatar_manifest_id: 'avatar_2' } }));
  // Altered avatar content hash -> FAILS
  assert.throws(() => assertSameWorld(current, { ...current, theme: { ...syn.theme, avatar_content_hash: 'sha256:avatar2' } }));
  report.immutableMapIdentity = 'PASS';

  // --- 3. Local Scoped Preferences Probes ---
  const storage = new Map();
  const prefs = createLocalThemePreferences({
    getItem: k => storage.get(k) ?? null,
    setItem: (k, v) => storage.set(k, v)
  });
  prefs.write({ userId: 'u1', eventId: 'e1' }, 'PARADE');
  assert.equal(prefs.read({ userId: 'u1', eventId: 'e1' }), 'PARADE');
  assert.equal(prefs.read({ userId: 'u1:e1', eventId: 'x' }), null);
  assert.equal(prefs.read({ userId: 'u1', eventId: 'e2' }), null);
  assert.throws(() => prefs.read({ userId: '', eventId: 'e1' }));
  assert.throws(() => prefs.read({ userId: 'u1', eventId: '' }));
  report.scopedPreferences = 'PASS';

  // --- 4. Atomic ReplaceSkin & Supersession Probes ---
  let initialDisposed = false;
  let slowDisposed = false;
  let fastDisposed = false;

  const initialLoaded = {
    ...current,
    image: { naturalWidth: 96, naturalHeight: 32 },
    dispose: () => { initialDisposed = true; }
  };

  const slowCandidate = {
    ...current,
    theme: { ...syn.theme, theme_id: 'PARADE' },
    image: { naturalWidth: 96, naturalHeight: 32 },
    dispose: () => { slowDisposed = true; }
  };

  const fastCandidate = {
    ...current,
    theme: { ...syn.theme, theme_id: 'DANGER' },
    image: { naturalWidth: 96, naturalHeight: 32 },
    dispose: () => { fastDisposed = true; }
  };

  let activeEngineTheme = 'REVENGE';
  const host = document.createElement('div');
  const runtime = createWorldRuntime({
    host,
    load: async () => initialLoaded,
    importEngine: async () => () => ({
      ready: Promise.resolve(),
      closed: new Promise(() => {}),
      resize: () => {},
      setVisible: () => {},
      destroy: () => {},
      replaceSkin: map => {
        activeEngineTheme = map.theme.theme_id;
      }
    }),
    timeoutMs: 500
  });

  await runtime.start();
  assert.equal(runtime.readSkin(), 'REVENGE');

  let resolveSlow;
  const slowPromise = new Promise(r => { resolveSlow = r; });
  const pSlow = runtime.replaceSkin(() => slowPromise);
  const pFast = runtime.replaceSkin(async () => fastCandidate);

  await pFast;
  assert.equal(runtime.readSkin(), 'DANGER');
  assert.equal(activeEngineTheme, 'DANGER');
  assert.equal(initialDisposed, true);

  resolveSlow(slowCandidate);
  await assert.rejects(pSlow, /cancelled/);
  assert.equal(slowDisposed, true, 'Superseded slow candidate must be disposed');
  assert.equal(runtime.readSkin(), 'DANGER', 'Skin must stay DANGER');

  // Rollback on engine failure
  let rollbackCandidateDisposed = false;
  const rollbackCand = {
    ...slowCandidate,
    dispose: () => { rollbackCandidateDisposed = true; }
  };

  const host2 = document.createElement('div');
  let init2Disposed = false;
  const runtime2 = createWorldRuntime({
    host: host2,
    load: async () => ({ ...initialLoaded, dispose: () => { init2Disposed = true; } }),
    importEngine: async () => () => ({
      ready: Promise.resolve(),
      closed: new Promise(() => {}),
      resize: () => {},
      setVisible: () => {},
      destroy: () => {},
      replaceSkin: () => { throw new Error('GL upload failed'); }
    }),
    timeoutMs: 500
  });
  await runtime2.start();

  await assert.rejects(runtime2.replaceSkin(async () => rollbackCand), /GL upload failed/);
  assert.equal(runtime2.readSkin(), 'REVENGE');
  assert.equal(rollbackCandidateDisposed, true, 'Failed candidate must be disposed');
  assert.equal(init2Disposed, false, 'Previous active skin must not be disposed on rollback');
  report.atomicRollbackAndSupersession = 'PASS';

  await writeFile('/tmp/mcr-qa-fe06-agy8u3i5/independent-probe-result.json', JSON.stringify(report, null, 2) + '\n');
  console.log('Independent probe results:', JSON.stringify(report, null, 2));

  runtime.dispose();
  await runtime.settled();
  runtime2.dispose();
  await runtime2.settled();
} finally {
  await dev.close();
  dom.window.close();
}

