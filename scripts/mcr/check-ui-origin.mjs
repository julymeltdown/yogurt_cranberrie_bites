#!/usr/bin/env node
/** C08: exact legacy-byte and static import guard; manual visual review remains required. */
import { createHash } from 'node:crypto';
import { readFile, readdir, lstat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { TextDecoder } from 'node:util';

const REVISION = '5eecdcaffd1a9d505b9ab2a974f8d5623217e90a';
const INVENTORY = 'docs/mcr/frontend/reuse-inventory.json';
const SKIP = new Set(['node_modules', 'dist', 'build', 'coverage', '.git', '.vite', '.cache',
  'test-results', 'playwright-report']);
const CODE = /\.(?:[cm]?[jt]sx?|css|scss|html)$/i;
// The specification deliberately reuses these entrypoint paths for newly authored code.
const NEW_ENTRYPOINTS = new Set(['frontend/src/main', 'frontend/src/app/router']);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const require = (condition, message) => { if (!condition) throw new Error(message); };
const sha = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const safe = value => typeof value === 'string' && value.length > 0 &&
  !value.includes('\\') && !value.includes('\0') &&
  value.split('/').every(part => part && part !== '.' && part !== '..');
const modulePath = value => value.replace(/\.(?:[cm]?[jt]sx?|css|scss)$/, '').replace(/\/index$/, '');

async function read(root, relative) {
  require(safe(relative), `Unsafe path: ${JSON.stringify(relative)}`);
  let current = root;
  for (const part of relative.split('/')) {
    current = path.join(current, part);
    require(!(await lstat(current)).isSymbolicLink(), `Symlink is not permitted: ${relative}`);
  }
  require((await lstat(current)).isFile(), `Not a regular file: ${relative}`);
  return readFile(current);
}

async function files(root, relative = 'frontend') {
  const location = path.join(root, relative);
  const info = await lstat(location);
  require(!info.isSymbolicLink(), `Symlink is not permitted: ${relative}`);
  if (info.isFile()) return [relative];
  require(info.isDirectory(), `Not a regular file/directory: ${relative}`);
  const result = [];
  for (const name of (await readdir(location)).sort()) {
    if (!SKIP.has(name)) result.push(...await files(root, `${relative}/${name}`));
  }
  return result;
}

// String-aware lexer, not a complete JS/TS/CSS/HTML parser. Documentation is never tokenized.
function imports(source) {
  const tokens = [...source.matchAll(
    /(?<comment>\/\/[^\n]*|\/\*[\s\S]*?\*\/|<!--[\s\S]*?-->)|(?<string>"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'|`(?:\\[\s\S]|[^`\\])*`)|(?<word>[\w$-]+)|(?<punct>[^\s])/g,
  )].filter(token => !token.groups.comment);
  return tokens.flatMap((token, index) => {
    if (!token.groups.string) return [];
    const before = tokens.slice(Math.max(0, index - 4), index).map(t => t[0]);
    const last = before.at(-1);
    const direct = last === 'from' || last === 'import';
    const call = last === '(' && ['import', 'require', 'url'].includes(before.at(-2));
    const resolve = before.join(' ') === 'require . resolve (';
    const html = last === '=' && ['src', 'href'].includes(before.at(-2));
    if (!direct && !call && !resolve && !html) return [];
    const value = token[0].slice(1, -1);
    return value.includes('${') ? [] : [{ value, line: source.slice(0, token.index).split('\n').length }];
  });
}

function forbiddenImport(file, value, excludedModules, packages) {
  const spec = value.split(/[?#]/, 1)[0];
  if (packages.some(pkg => spec === pkg || spec.startsWith(`${pkg}/`)) ||
      /^@(?:mantine|ffmpeg|react-three)\//.test(spec)) return 'prohibited rendering/runtime dependency';
  const resolved = spec.startsWith('.') ? path.posix.normalize(path.posix.join(path.posix.dirname(file), spec))
    : spec.startsWith('@/') ? `frontend/src/${spec.slice(2)}`
    : spec.startsWith('/src/') ? `frontend${spec}`
    : spec.startsWith('/') ? `frontend/public${spec}` : spec;
  if (/^(?:@\/|frontend\/src\/)(?:components|pages|widgets|styles|layouts|themes?)(?:\/|$)/.test(resolved) ||
      /(?:^|[/@])(?:bigyeting|legacy)(?:\/|$)/i.test(resolved) ||
      (excludedModules.has(modulePath(resolved)) && !NEW_ENTRYPOINTS.has(modulePath(resolved)))) {
    return 'legacy rendering namespace/module';
  }
  return null;
}

export async function checkUIOrigin({ root, allowlist = 'docs/mcr/reuse-allowlist.json' }) {
  root = path.resolve(root);
  // An explicit external allowlist lets the integrator review this patch before integration.
  const policy = JSON.parse(path.isAbsolute(allowlist)
    ? await readFile(allowlist) : await read(root, allowlist));
  require(policy.schema_version === 1, 'Unsupported allowlist schema_version');
  require(policy.inventory?.path === INVENTORY && sha(policy.inventory.sha256) &&
    policy.inventory.source_revision === REVISION, 'Invalid inventory pin/path/source revision');
  const bytes = await read(root, INVENTORY);
  require(hash(bytes) === policy.inventory.sha256, 'Inventory SHA-256 mismatch; review and repin explicitly');
  const inventory = JSON.parse(bytes);
  require(inventory.schema_version === 1 && inventory.source?.revision === REVISION &&
    inventory.source.repository === 'julymeltdown/bigyeting', 'Unsupported inventory version/source');
  require(inventory.legacy_exclusions?.length === 422 && inventory.reuse_candidates?.length === 7,
    'Expected 422 legacy exclusions and 7 reuse candidates');
  require(Array.isArray(policy.reviewed_headless_extracts), 'Missing reviewed_headless_extracts array');
  const excluded = new Map();
  const excludedPaths = new Set();
  for (const entry of inventory.legacy_exclusions) {
    require(safe(entry.path) && entry.path.startsWith('frontend/') && sha(entry.sha256) &&
      /^[a-f0-9]{40}$/.test(entry.git_blob) && entry.decision === 'DO_NOT_COPY' &&
      !excludedPaths.has(entry.path), 'Invalid/duplicate legacy exclusion');
    excluded.set(entry.sha256, entry.path);
    excludedPaths.add(entry.path);
  }
  const candidates = new Map();
  for (const candidate of inventory.reuse_candidates) {
    require(Array.isArray(candidate.source_files) && candidate.source_files.length > 0,
      'Invalid headless candidate source_files');
    for (const source of candidate.source_files) {
      require(safe(source.path) && sha(source.sha256) && /^[a-f0-9]{40}$/.test(source.git_blob) &&
        !excludedPaths.has(source.path) && !excluded.has(source.sha256), 'Invalid/visual headless candidate');
      const previous = candidates.get(source.path);
      require(!previous || previous.sha256 === source.sha256, 'Conflicting candidate hashes');
      candidates.set(source.path, source);
    }
  }
  const approved = new Map();
  for (const entry of policy.reviewed_headless_extracts) {
    const source = candidates.get(entry.source_path);
    require(source && source.sha256 === entry.source_sha256 && entry.source_revision === REVISION &&
      !excludedPaths.has(entry.source_path) && !excluded.has(entry.source_sha256) &&
      /^frontend\/src\/.*\.ts$/.test(entry.source_path), 'Allowlist source is not a nonvisual inventoried candidate');
    require(safe(entry.destination) && /^frontend\/src\/.*\.ts$/.test(entry.destination) &&
      sha(entry.destination_sha256) && !excluded.has(entry.destination_sha256) &&
      !approved.has(entry.destination), 'Invalid/visual/duplicate allowlist destination');
    require(entry.kind === 'headless-extract' && typeof entry.reviewed_by === 'string' &&
      entry.reviewed_by.trim() && Array.isArray(entry.symbols) && entry.symbols.length > 0 &&
      entry.symbols.every(s => typeof s === 'string' && s.trim()), 'Headless symbol review is required');
    require(Array.isArray(entry.tests) && entry.tests.length > 0, 'Regression test paths are required');
    for (const test of entry.tests) {
      require(safe(test) && test.startsWith('frontend/') && /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(test),
        'Invalid regression test path');
      await read(root, test); // Existence is evidence linkage, not evidence the test passed.
    }
    require(hash(await read(root, entry.destination)) === entry.destination_sha256,
      'Reviewed destination SHA-256 mismatch');
    approved.set(entry.destination, entry);
  }
  const packages = [...new Set(['three', 'gsap', 'motion', 'framer-motion',
    ...(inventory.source_package?.direct_dependencies ?? [])
      .filter(dep => dep.disposition === 'EXCLUDE_FROM_FOUNDATION').map(dep => dep.name)])];
  const modules = new Set([...excludedPaths].map(modulePath));
  const candidateHashes = new Set([...candidates.values()].map(source => source.sha256));
  const violations = [];
  const paths = await files(root);
  let contentFiles = 0;
  for (const file of paths) {
    const content = await read(root, file);
    const digest = hash(content);
    if (/^frontend\/(?:src|public|tests?|e2e)\//.test(file) || file === 'frontend/index.html') contentFiles++;
    if (excluded.has(digest)) violations.push(`${file}: forbidden legacy bytes from ${excluded.get(digest)}`);
    if (candidateHashes.has(digest) && !approved.has(file)) violations.push(`${file}: unreviewed headless source copy`);
    if (CODE.test(file)) {
      const source = new TextDecoder('utf-8', { fatal: true }).decode(content);
      for (const { value, line } of imports(source)) {
        const reason = forbiddenImport(file, value, modules, packages);
        if (reason) violations.push(`${file}:${line}: ${reason}: ${JSON.stringify(value)}`);
      }
    }
    if (file.endsWith('/package.json')) {
      const manifest = JSON.parse(content);
      for (const section of ['dependencies', 'devDependencies', 'optionalDependencies', 'peerDependencies']) {
        for (const name of Object.keys(manifest[section] ?? {})) {
          if (forbiddenImport(file, name, new Set(), packages)) violations.push(`${file}: prohibited dependency ${name}`);
        }
      }
    }
  }
  return { task: 'C08', result: violations.length ? 'FAIL' : contentFiles ? 'PASS' : 'NOT_RUN',
    source_revision: REVISION, inventory_sha256: hash(bytes), files_scanned: paths.length,
    content_files: contentFiles, reviewed_extracts: approved.size, violations,
    scope: 'Exact bytes and static imports only; manual source/visual review and QA02 remain required' };
}

async function main(args) {
  const options = { root: process.cwd() };
  while (args.length) {
    const flag = args.shift();
    if (flag === '--help') {
      console.log('Usage: node check-ui-origin.mjs [--root PATH] [--allowlist PATH]');
      return 0;
    }
    require(['--root', '--allowlist'].includes(flag) && args.length && !args[0].startsWith('--'),
      `Unknown/missing option: ${flag}`);
    options[flag.slice(2)] = args.shift();
  }
  const report = await checkUIOrigin(options);
  console.log(JSON.stringify(report, null, 2));
  return report.result === 'PASS' ? 0 : report.result === 'FAIL' ? 1 : 2;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = await main(process.argv.slice(2)); }
  catch (error) { console.error(`UI ORIGIN ERROR: ${error.message}`); process.exitCode = 2; }
}
