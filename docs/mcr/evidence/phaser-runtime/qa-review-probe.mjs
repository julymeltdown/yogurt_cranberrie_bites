import { createRequire } from 'node:module'
import { writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'
const qa = '/Users/lhs/Desktop/mcr-worktrees/qa-fe-runtime'
const root = process.argv[2] ?? qa
const output = process.argv[3] ?? '/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-fe14-qa-hljKKQ/combined-initial.json'
const require = createRequire(qa + '/frontend/package.json')
const { JSDOM } = require('jsdom')
const { createServer } = await import(pathToFileURL(qa + '/frontend/node_modules/vite/dist/node/index.js'))
const dom = new JSDOM('<!doctype html><div id="host"></div>')
globalThis.window = dom.window; globalThis.document = dom.window.document
globalThis.ResizeObserver = class { observe() {} disconnect() {} }
globalThis.requestAnimationFrame = () => 1; globalThis.cancelAnimationFrame = () => {}
const server = await createServer({ root: root + '/frontend', configFile: false, server: { middlewareMode: true, hmr: false, watch: null } })
try {
  const { createWorldRuntime } = await server.ssrLoadModule('/src/world/runtime.ts')
  const { synthetic, rehash } = await server.ssrLoadModule('/src/world/__tests__/fixtures/synthetic.ts')
  const { validateManifests } = await server.ssrLoadModule('/src/world/map/manifest.ts')
  const f = await synthetic(), meta = await validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), f.pin)
  const host = document.getElementById('host'), result = { candidateRoot: root, inheritedDrain: [], reentrantVisibility: [], uriParity: [] }
  for (const action of ['stop', 'dispose', 'abort']) {
    let allocations = 0, close
    const closed = new Promise(resolve => { close = resolve })
    const factory = () => { allocations++; return { ready: Promise.resolve(), closed: allocations === 1 ? closed : Promise.resolve(), resize() {}, setVisible() {}, destroy() {} } }
    const options = { host, load: async () => ({ ...meta, image: {}, dispose() {} }), importEngine: async () => factory }
    const a = createWorldRuntime(options), b = createWorldRuntime(options)
    await a.start(); a.stop(); const signal = new AbortController(), replacement = a.start(signal.signal)
    if (action === 'abort') signal.abort(); else a[action]()
    const before = await b.start(), beforeCount = allocations
    close(); await replacement; await a.settled()
    const after = await b.start()
    result.inheritedDrain.push({ action, before, allocationsBeforeOldClosed: beforeCount, after, allocationsAfterOldClosed: allocations })
    a.dispose(); b.dispose(); await b.settled()
  }
  for (const action of ['stop', 'dispose', 'restart']) {
    let runtime, calls = 0, destroyed = 0, disposed = 0, next
    const seen = []
    const factory = () => ({ ready: Promise.resolve(), closed: Promise.resolve(), resize() {}, destroy() { destroyed++ }, setVisible() {
      if (calls++ === 0) {
        if (action === 'restart') { runtime.stop(); next = runtime.start() } else runtime[action]()
      }
    } })
    runtime = createWorldRuntime({ host, load: async () => ({ ...meta, image: {}, dispose() { disposed++ } }), importEngine: async () => factory })
    runtime.subscribe(() => seen.push(runtime.read().phase))
    const initialResult = await runtime.start(); if (next) await next
    await runtime.settled()
    result.reentrantVisibility.push({ action, initialResult, finalState: runtime.read(), seen: [...seen], destroyed, disposed, childCount: host.childElementCount })
    runtime.dispose(); await runtime.settled()
  }
  for (const uri of ['http://[::ffff:192.168.001.1]/', 'http://[::ffff:192.168.01.1]/', 'http://[::ffff:192.168.1.1]/', 'x://[V1.ab:cd]/', 'x://[v1.ab:cd]/', 'http://[::]/', 'urn:abc:def']) {
    const f = await synthetic(); f.theme.provenance.source_uri = uri; const pin = await rehash(f.map, f.theme)
    let client = true
    try { await validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), pin) } catch { client = false }
    const program = "import json,runpy,sys;a=runpy.run_path('scripts/mcr/validate-assets.py');d=json.load(sys.stdin);c=a['validators']();a['validate_document'](d['map'],c,'map');a['validate_document'](d['theme'],c,'theme');a['validate_map'](d['map'])"
    const python = spawnSync('/tmp/mcr-fe14-format-venv/bin/python', ['-B', '-c', program], { cwd: root, input: JSON.stringify({ map: f.map, theme: f.theme }), encoding: 'utf8' })
    if (python.error || python.status === null) throw python.error ?? Error(python.stderr)
    result.uriParity.push({ uri, client, python: python.status === 0, pythonExit: python.status })
  }
  writeFileSync(output, JSON.stringify(result, null, 2) + '\n')
  console.log(JSON.stringify(result, null, 2))
} finally { await server.close(); dom.window.close() }
