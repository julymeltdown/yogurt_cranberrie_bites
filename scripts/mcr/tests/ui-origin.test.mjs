import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, writeFile, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { checkUIOrigin } from '../check-ui-origin.mjs';

const SCRIPT = fileURLToPath(new URL('../check-ui-origin.mjs', import.meta.url));
const REVISION = '5eecdcaffd1a9d505b9ab2a974f8d5623217e90a';
const digest = value => createHash('sha256').update(value).digest('hex');
const LEGACY = 'export const OldControl = () => <button>synthetic legacy UI</button>;\n';
const HEADLESS = 'export const normalize = x => x;\n';

async function fixture(t) {
  const root = await mkdtemp(path.join(tmpdir(), 'mcr-ui-origin-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const write = async (name, value) => {
    await mkdir(path.dirname(path.join(root, name)), { recursive: true });
    await writeFile(path.join(root, name), value);
  };
  // Synthetic hash metadata, not a copy of the real inventory or upstream assets.
  const inventory = {
    schema_version: 1, source: { repository: 'julymeltdown/bigyeting', revision: REVISION },
    legacy_exclusions: Array.from({ length: 422 }, (_, i) => ({
      path: `frontend/src/components/Old${i}.tsx`, git_blob: '1'.repeat(40),
      sha256: digest(i ? `synthetic excluded ${i}` : LEGACY), decision: 'DO_NOT_COPY',
    })),
    reuse_candidates: Array.from({ length: 7 }, (_, i) => ({
      source_files: [{ path: `frontend/src/lib/helper${i}.ts`, git_blob: '2'.repeat(40),
        sha256: digest(i ? `synthetic helper ${i}` : HEADLESS) }],
    })),
    source_package: { direct_dependencies: [{ name: '@phosphor-icons/react',
      disposition: 'EXCLUDE_FROM_FOUNDATION' }] },
  };
  const policy = { schema_version: 1, inventory: { path: 'docs/mcr/frontend/reuse-inventory.json',
    source_revision: REVISION, sha256: '' }, reviewed_headless_extracts: [] };
  const save = async () => {
    const json = JSON.stringify(inventory);
    policy.inventory.sha256 = digest(json);
    await write(policy.inventory.path, json);
    await write('docs/mcr/reuse-allowlist.json', JSON.stringify(policy));
  };
  await write('frontend/src/features/world.ts', 'export const originalWorld = true;\n');
  await save();
  return { root, write, inventory, policy, save, check: () => checkUIOrigin({ root }) };
}

test('clean original code and prose containing library/import words pass', async t => {
  const f = await fixture(t);
  f.inventory.legacy_exclusions[1].path = 'frontend/src/app/router.tsx';
  await f.save();
  await f.write('frontend/README.md', 'import x from "@mantine/core"; three gsap legacy UI');
  await f.write('frontend/src/features/info.ts', `
// import x from '@mantine/core';
/* export * from 'three'; */
const prose = 'import x from "gsap"';
import { createElement } from 'react';
import { Router } from '../app/router';
`);
  assert.equal((await f.check()).result, 'PASS');
});

test('renamed byte-identical legacy UI fails', async t => {
  const f = await fixture(t);
  await f.write('frontend/src/features/new-control.tsx', LEGACY);
  const report = await f.check();
  assert.equal(report.result, 'FAIL');
  assert.match(report.violations.join('\n'), /forbidden legacy bytes/);
});

test('copied asset bytes under public or tests fail regardless of extension', async t => {
  const f = await fixture(t);
  const bytes = Buffer.from([137, 80, 78, 71, 0, 255, 19]);
  f.inventory.legacy_exclusions[0].sha256 = digest(bytes);
  f.inventory.legacy_exclusions[0].path = 'frontend/public/brand/old.png';
  await f.save();
  for (const name of ['frontend/public/world/new.bin', 'frontend/tests/fixtures/avatar.data']) {
    await f.write(name, bytes);
  }
  assert.equal((await f.check()).violations.filter(v => v.includes('forbidden legacy bytes')).length, 2);
});

test('static prohibited dependency and legacy namespace import forms fail', async t => {
  const f = await fixture(t);
  for (const code of [
    `import { Button } from '@mantine/core';`, `export * from 'three/addons';`,
    `const x = import('gsap');`, `const x = require('@ffmpeg/ffmpeg');`,
    'const x = import(`@react-three/fiber`);', `import type { X } from '@/components/ui/old';`,
    `import x from './components/ui/old';`, `import x from '@bigyeting/ui';`,
    `import x = require('motion/react');`, `const x = require.resolve('@phosphor-icons/react');`,
  ]) {
    await f.write('frontend/src/probe.ts', code);
    assert.equal((await f.check()).result, 'FAIL', code);
  }
});

test('CSS and HTML loading references fail', async t => {
  const f = await fixture(t);
  await f.write('frontend/src/theme.css', '@import "@mantine/core/styles.css";');
  await f.write('frontend/index.html', '<script src="legacy/app.js"></script>');
  assert.equal((await f.check()).violations.length, 2);
});

test('prohibited manifest dependency fails even without a source import', async t => {
  const f = await fixture(t);
  await f.write('frontend/package.json', JSON.stringify({ devDependencies: { three: '0.184.0' } }));
  assert.match((await f.check()).violations.join('\n'), /prohibited dependency three/);
});

test('modified allowlist cannot approve a legacy visual source', async t => {
  const f = await fixture(t);
  f.policy.reviewed_headless_extracts.push({
    source_path: f.inventory.legacy_exclusions[0].path,
    source_sha256: f.inventory.legacy_exclusions[0].sha256, source_revision: REVISION,
    destination: 'frontend/src/shared/ui/old.ts', destination_sha256: digest(LEGACY),
    kind: 'headless-extract', reviewed_by: 'fixture', symbols: ['OldControl'], tests: [],
  });
  await f.save();
  await assert.rejects(f.check, /not a nonvisual inventoried candidate/);
});

test('candidate bytes require reviewed source revision/hash, destination hash, and tests', async t => {
  const f = await fixture(t);
  const destination = 'frontend/src/shared/normalize.ts';
  const regression = 'frontend/src/shared/normalize.test.ts';
  await f.write(destination, HEADLESS);
  assert.match((await f.check()).violations.join('\n'), /unreviewed headless source copy/);
  const entry = { source_path: 'frontend/src/lib/helper0.ts', source_sha256: digest(HEADLESS),
    source_revision: REVISION, destination, destination_sha256: digest(HEADLESS),
    kind: 'headless-extract', reviewed_by: 'fixture integrator', symbols: ['normalize'], tests: [regression] };
  f.policy.reviewed_headless_extracts.push(entry);
  await f.write(regression, 'export const semanticFixture = true;');
  await f.save();
  assert.equal((await f.check()).result, 'PASS');
  for (const [field, invalid, message] of [
    ['source_revision', '0'.repeat(40), /nonvisual inventoried candidate/],
    ['source_sha256', '0'.repeat(64), /nonvisual inventoried candidate/],
    ['tests', [], /Regression test paths/],
    ['tests', ['frontend/tests/missing.test.ts'], /ENOENT/],
    ['destination_sha256', '0'.repeat(64), /destination SHA-256 mismatch/],
    ['destination', '../outside.ts', /Invalid\/visual\/duplicate/],
  ]) {
    const saved = entry[field];
    entry[field] = invalid;
    await f.save();
    await assert.rejects(f.check, message);
    entry[field] = saved;
  }
});

test('visual destination hash cannot be approved using a headless source', async t => {
  const f = await fixture(t);
  f.policy.reviewed_headless_extracts.push({ source_path: 'frontend/src/lib/helper0.ts',
    source_sha256: digest(HEADLESS), source_revision: REVISION,
    destination: 'frontend/src/shared/old.ts', destination_sha256: digest(LEGACY) });
  await f.save();
  await assert.rejects(f.check, /Invalid\/visual\/duplicate allowlist destination/);
});

test('inventory hash, exact schema versions, source revision and counts fail closed', async t => {
  const f = await fixture(t);
  await f.write(f.policy.inventory.path, '{}');
  await assert.rejects(f.check, /Inventory SHA-256 mismatch/);
  for (const mutate of [
    () => { f.policy.schema_version = 2; },
    () => { f.inventory.schema_version = 2; },
    () => { f.inventory.source.revision = '0'.repeat(40); },
    () => { f.inventory.legacy_exclusions.pop(); },
    () => { f.inventory.reuse_candidates.pop(); },
    () => { f.inventory.legacy_exclusions[0].sha256 = 'not-a-sha256'; },
  ]) {
    const oldPolicy = structuredClone(f.policy), oldInventory = structuredClone(f.inventory);
    mutate();
    await f.save();
    await assert.rejects(f.check);
    Object.assign(f.policy, oldPolicy);
    Object.assign(f.inventory, oldInventory);
  }
});

test('dependencies and build outputs are excluded; source symlinks fail closed', async t => {
  const f = await fixture(t);
  await f.write('frontend/node_modules/old/control.tsx', LEGACY);
  await f.write('frontend/dist/control.tsx', LEGACY);
  assert.equal((await f.check()).result, 'PASS');
  await symlink('/nonexistent/mcr-fixture', path.join(f.root, 'frontend/src/link.ts'));
  await assert.rejects(f.check, /Symlink/);
});

test('no frontend content reports NOT_RUN rather than a passing application check', async t => {
  const f = await fixture(t);
  await rm(path.join(f.root, 'frontend/src'), { recursive: true });
  await f.write('frontend/README.md', 'Implementation has not started.');
  assert.equal((await f.check()).result, 'NOT_RUN');
});

test('portable CLI --root, invalid flags, and missing inventory', async t => {
  const f = await fixture(t);
  const run = (...args) => spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8', cwd: tmpdir() });
  const clean = run('--root', f.root);
  assert.equal(clean.status, 0, clean.stderr);
  assert.equal(JSON.parse(clean.stdout).result, 'PASS');
  assert.equal(run('--root', f.root, '--allowlist', path.join(f.root, 'docs/mcr/reuse-allowlist.json')).status, 0);
  await f.write('frontend/e2e/renamed.spec.tsx', LEGACY);
  assert.equal(run('--root', f.root).status, 1);
  assert.equal(run('--root').status, 2);
  assert.equal(run('--unknown').status, 2);
  await rm(path.join(f.root, f.policy.inventory.path));
  assert.equal(run('--root', f.root).status, 2);
});

test('shipped policy pins the audited inventory and only the four reviewed headless extracts', async () => {
  const policy = JSON.parse(await readFile(new URL('../../../docs/mcr/reuse-allowlist.json', import.meta.url)));
  assert.equal(policy.schema_version, 1);
  assert.equal(policy.inventory.source_revision, REVISION);
  assert.equal(policy.inventory.sha256, '9d1202211cd136948a7afeb5588238ba9de13613980dab42616011f47762a079');
  const inventoryBytes = await readFile(new URL('../../../docs/mcr/frontend/reuse-inventory.json', import.meta.url));
  assert.equal(digest(inventoryBytes), policy.inventory.sha256);
  const inventory = JSON.parse(inventoryBytes);
  const candidates = inventory.reuse_candidates.flatMap(candidate => candidate.source_files);
  const excluded = new Set(inventory.legacy_exclusions.map(entry => entry.sha256));
  // FE03's reviewed additions are pure validation/policy, never controls, layouts or themes.
  assert.deepEqual(policy.reviewed_headless_extracts.map(entry => entry.destination).sort(), [
    'frontend/src/entities/intro/model/age.ts',
    'frontend/src/entities/intro/model/publication-contact.ts',
    'frontend/src/shared/validation/tags.ts',
    'frontend/src/shared/validation/text.ts',
  ]);
  for (const entry of policy.reviewed_headless_extracts) {
    assert.equal(entry.kind, 'headless-extract');
    assert.equal(entry.source_revision, REVISION);
    assert.ok(candidates.some(source => source.path === entry.source_path && source.sha256 === entry.source_sha256));
    assert.equal(excluded.has(entry.source_sha256), false);
    assert.equal(excluded.has(entry.destination_sha256), false);
    assert.ok(entry.reviewed_by && entry.symbols.length && entry.tests.length);
    const destination = await readFile(new URL(`../../../${entry.destination}`, import.meta.url));
    assert.equal(digest(destination), entry.destination_sha256);
    for (const regression of entry.tests) await readFile(new URL(`../../../${regression}`, import.meta.url));
  }
});
