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
  cacheDir: '/tmp/mcr-qa-fe06-agy8u3i5/cache3',
  server: { watch: null },
  optimizeDeps: { noDiscovery: true }
});

try {
  const { createWorldRuntime } = await dev.ssrLoadModule('/src/world/runtime.ts');
  const { loadMap } = await dev.ssrLoadModule('/src/world/map/resources.ts');
  const { syntheticRelease } = await dev.ssrLoadModule('/src/world/theme/__tests__/fixtures/releases.ts');

  async function load(id) {
    const f = await syntheticRelease(id);
    let disposed = false;
    const res = await loadMap({
      mapText: JSON.stringify(f.map),
      themeText: JSON.stringify(f.theme),
      pin: f.pin,
      resources: { read: async r => r.kind === 'png' ? f.png : f.tiledBytes },
      decode: async () => ({
        image: { naturalWidth: 96, naturalHeight: 32 },
        dispose: () => { disposed = true; }
      })
    }, new AbortController().signal);
    return { res, getDisposed: () => disposed };
  }

  const initial = await load('REVENGE');
  const next = await load('PARADE');

  let activeEngineTheme = 'REVENGE';
  let runtimeRef;
  let reentrantCalled = false;

  const host = document.createElement('div');
  const runtime = createWorldRuntime({
    host,
    load: async () => initial.res,
    importEngine: async () => () => ({
      ready: Promise.resolve(),
      closed: new Promise(() => {}),
      resize: () => {},
      setVisible: () => {},
      destroy: () => {},
      replaceSkin: map => {
        activeEngineTheme = map.theme.theme_id;
        reentrantCalled = true;
        runtimeRef.replaceSkin(async () => next.res).catch(() => {});
      }
    }),
    timeoutMs: 500
  });
  runtimeRef = runtime;

  await runtime.start();
  assert.equal(runtime.readSkin(), 'REVENGE');

  let threwError = null;
  try {
    await runtime.replaceSkin(async () => next.res);
  } catch (err) {
    threwError = err;
  }

  const finding = {
    reentrantCalled,
    threwError: threwError ? threwError.message : null,
    reason: threwError?.reason,
    runtimeReadSkin: runtime.readSkin(),
    activeEngineTheme,
    nextImageDisposed: next.getDisposed(),
    initialImageDisposed: initial.getDisposed()
  };

  console.log('REENTRANT RESULT:', JSON.stringify(finding, null, 2));

  await writeFile('/tmp/mcr-qa-fe06-agy8u3i5/reentrancy-finding.json', JSON.stringify(finding, null, 2) + '\n');
} finally {
  await dev.close();
  dom.window.close();
}

