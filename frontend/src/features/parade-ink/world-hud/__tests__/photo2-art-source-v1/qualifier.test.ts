import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { test } from 'node:test';
import { inflateSync } from 'node:zlib';

type Point = { x: number; y: number };
type Rect = Point & { width: number; height: number };
type Frame = { id: string; atlasRect: Rect; displayRect: Rect; native?: Record<string, unknown>; nativeFoot?: Point; displayFoot?: Point; origin?: Point; destination?: string; interactionId?: null };
type Contract = { mesh: { columns: number[]; rows: number[]; display: Point[] }; frames: Frame[]; boards: Frame[]; nativeSource: { commit: string; path: string }; ground: { displayBounds: Rect } };
type Art = { scenery: { ground: Resource; atlases: Resource[]; objects: Record<string, unknown>[] }; sceneDescriptor: { mesh: Contract['mesh']; displayBounds: Rect; frames: Frame[]; boards: Frame[]; collisionSupport: { proofSHA256: string } }; resourceUnits: { sourcePixelCount: number; decodedRGBABytes: number; actualPngTransferBytes: number }; activation: { state: string } };
type Resource = { path: string; sha256: string; bytes: number; width_px: number; height_px: number };
type Support = { blockedCells: number[]; pieces: { cell: number; triangle: number; nativePolygon: number[][]; displayPolygon: number[][]; nativeArea: number }[] };
const root = process.cwd();
const folder = resolve(root, 'frontend/public/world/reference-scene-v2/photo2-world-display-v1/parade');
const evidence = resolve(root, 'docs/mcr/art/evidence/photo2-world-display-v1');
const hash = (b: Uint8Array) => createHash('sha256').update(b).digest('hex');
const parse = <T>(b: Uint8Array): T => JSON.parse(Buffer.from(b).toString()) as T;
const rawContract = execFileSync('/usr/bin/git', ['show', '33491ccf9b0ae3835ad691072d8479b299f543d8:docs/mcr/evidence/pi08-finish-01/photo2-world-contract/producer-contract-v2.json']);
assert.equal(hash(rawContract), '3fce374b7f60d46e8561559c42470a3343708f9eb3236a1d01050c4f0aa3a12d');
const contract = parse<Contract>(rawContract);
const nativeRaw = execFileSync('/usr/bin/git', ['show', `${contract.nativeSource.commit}:${contract.nativeSource.path}`]);
assert.equal(hash(nativeRaw), '847d9c92a4ddfa22b8187bb6afe9a9fc2e6e09b1c7fe8e5f85c5a7f5c0d846a9');
const native = parse<{ geometry: { collision_grid: number[][] }; scenes: { PARADE: { scenery: { objects: Record<string, unknown>[] } } } }>(nativeRaw);
const art = parse<Art>(readFileSync(resolve(folder, 'asset-interface.json')));
const support = parse<Support>(readFileSync(resolve(evidence, 'collision-support.json')));
const allFrames = [...contract.frames, ...contract.boards];

function crc32(b: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of b) { crc ^= byte; for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1)); }
  return (crc ^ 0xffffffff) >>> 0;
}
function decode(b: Buffer) {
  assert.equal(b.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  let at = 8; let width = 0; let height = 0; let channels = 0; let ended = false;
  const chunks: Buffer[] = [];
  while (at < b.length) {
    const size = b.readUInt32BE(at); assert.ok(at + 12 + size <= b.length);
    const name = b.toString('ascii', at + 4, at + 8); const data = b.subarray(at + 8, at + 8 + size);
    assert.equal(crc32(b.subarray(at + 4, at + 8 + size)), b.readUInt32BE(at + 8 + size));
    if (name === 'IHDR') { assert.equal(at, 8); assert.equal(size, 13); width = data.readUInt32BE(0); height = data.readUInt32BE(4); assert.equal(data[8], 8); channels = data[9] === 6 ? 4 : data[9] === 2 ? 3 : 0; assert.ok(channels); assert.equal(data[10], 0); assert.equal(data[11], 0); assert.equal(data[12], 0); assert.ok(width * height <= 2097152); }
    if (name === 'IDAT') chunks.push(data);
    at += size + 12;
    if (name === 'IEND') { assert.equal(size, 0); ended = true; break; }
  }
  assert.ok(ended); assert.equal(at, b.length);
  const rowBytes = width * channels; const inflated = inflateSync(Buffer.concat(chunks), { maxOutputLength: (rowBytes + 1) * height });
  assert.equal(inflated.length, (rowBytes + 1) * height); const pixels = Buffer.alloc(rowBytes * height);
  for (let y = 0; y < height; y++) {
    const kind = inflated[y * (rowBytes + 1)]!; assert.ok(kind <= 4);
    for (let x = 0; x < rowBytes; x++) {
      const left = x >= channels ? pixels[y * rowBytes + x - channels]! : 0;
      const above = y ? pixels[(y - 1) * rowBytes + x]! : 0;
      const corner = y && x >= channels ? pixels[(y - 1) * rowBytes + x - channels]! : 0;
      const p = left + above - corner; const distances = [Math.abs(p - left), Math.abs(p - above), Math.abs(p - corner)];
      const paeth = distances[0]! <= distances[1]! && distances[0]! <= distances[2]! ? left : distances[1]! <= distances[2]! ? above : corner;
      const prediction = [0, left, above, Math.floor((left + above) / 2), paeth][kind]!;
      pixels[y * rowBytes + x] = (inflated[y * (rowBytes + 1) + 1 + x]! + prediction) & 255;
    }
  }
  return { width, height, channels, pixels };
}
function qualify(a: Art) {
  assert.deepEqual(a.sceneDescriptor.mesh, contract.mesh); assert.deepEqual(a.sceneDescriptor.frames, contract.frames); assert.deepEqual(a.sceneDescriptor.boards, contract.boards); assert.deepEqual(a.sceneDescriptor.displayBounds, contract.ground.displayBounds);
  assert.equal(a.scenery.objects.length, 22); assert.equal(a.scenery.atlases.length, 1);
  for (const [i, old] of native.scenes.PARADE.scenery.objects.entries()) {
    const changed = { ...a.scenery.objects[i] }; delete changed.atlas_id; delete changed.frame;
    const expected = { ...old }; delete expected.atlas_id; delete expected.frame;
    assert.deepEqual(changed, expected); assert.deepEqual(a.scenery.objects[i]!.frame, contract.frames.find(f => f.id === old.id)!.atlasRect);
  }
  let bytes = 0; let pixels = 0;
  for (const r of [a.scenery.ground, ...a.scenery.atlases]) {
    assert.ok(r.path.startsWith('world/reference-scene-v2/photo2-world-display-v1/parade/')); assert.ok(!r.path.includes('..'));
    const raw = readFileSync(resolve(root, 'frontend/public', r.path)); assert.equal('sha256:' + hash(raw), r.sha256); assert.equal(raw.length, r.bytes);
    const image = decode(raw); assert.equal(image.width, r.width_px); assert.equal(image.height, r.height_px); bytes += raw.length; pixels += image.width * image.height;
  }
  assert.equal(pixels, 1123088); assert.equal(a.resourceUnits.sourcePixelCount, pixels); assert.equal(a.resourceUnits.decodedRGBABytes, pixels * 4); assert.equal(a.resourceUnits.actualPngTransferBytes, bytes); assert.ok(bytes <= 2097152); assert.equal(a.activation.state, 'NOT_ADMITTED');
}
test('qualified source pins, exact native objects/frames/mesh and actual resource units', () => qualify(art));
test('foreground scenery alpha leaves both shortcut boards fully visible', () => {
  const image = decode(readFileSync(resolve(folder, 'atlas.png')));
  for (const board of contract.boards) for (const f of contract.frames) {
    const b = board.displayRect; const r = f.displayRect;
    if (f.displayFoot!.y <= b.y + b.height) continue;
    const ox = Math.round(r.x); const oy = Math.round(r.y);
    for (let y = Math.max(oy, b.y); y < Math.min(oy + r.height, b.y + b.height); y++)
      for (let x = Math.max(ox, b.x); x < Math.min(ox + r.width, b.x + b.width); x++) {
        const p = ((f.atlasRect.y + y - oy) * image.width + f.atlasRect.x + x - ox) * 4 + 3;
        assert.equal(image.pixels[p], 0, `${f.id} covers ${board.id} at ${x},${y}`);
      }
  }
});
test('transparent atlas has no paint outside its 24 disjoint frames', () => {
  const image = decode(readFileSync(resolve(folder, 'atlas.png'))); assert.deepEqual([image.width, image.height, image.channels], [512, 1024, 4]);
  const occupied = new Uint8Array(512 * 1024);
  for (const f of allFrames) { const r = f.atlasRect; let painted = 0; for (let y = r.y; y < r.y + r.height; y++) for (let x = r.x; x < r.x + r.width; x++) { const p = y * 512 + x; assert.equal(occupied[p], 0); occupied[p] = 1; if (image.pixels[p * 4 + 3]) painted++; } assert.ok(painted > 100, f.id); }
  for (let p = 0; p < occupied.length; p++) if (!occupied[p]) assert.equal(image.pixels[p * 4 + 3], 0);
});
const area = (p: number[][]) => Math.abs(p.reduce((sum, a, i) => { const b = p[(i + 1) % p.length]!; return sum + a[0]! * b[1]! - b[0]! * a[1]!; }, 0)) / 2;
function coverage(s: Support) {
  const expected = native.geometry.collision_grid.flatMap((row, y) => row.flatMap((blocked, x) => blocked ? [y * 48 + x] : []));
  assert.deepEqual(s.blockedCells, expected); const totals = new Map<number, number>();
  for (const p of s.pieces) { assert.ok(expected.includes(p.cell)); const x = (p.cell % 48) * 16; const y = Math.floor(p.cell / 48) * 16; assert.equal(p.nativePolygon.length, p.displayPolygon.length); for (const v of p.nativePolygon) assert.ok(v[0]! >= x - 1e-7 && v[0]! <= x + 16 + 1e-7 && v[1]! >= y - 1e-7 && v[1]! <= y + 16 + 1e-7); assert.ok(Math.abs(area(p.nativePolygon) - p.nativeArea) < 1e-6); assert.ok(area(p.displayPolygon) > 0); totals.set(p.cell, (totals.get(p.cell) ?? 0) + area(p.nativePolygon)); }
  for (const piece of s.pieces) {
    const mesh = contract.mesh; const cols = mesh.columns.length; const cell = Math.floor(piece.triangle / 2); const x = cell % (cols - 1); const y = Math.floor(cell / (cols - 1));
    const indexes = piece.triangle % 2 === 0 ? [y * cols + x, y * cols + x + 1, (y + 1) * cols + x + 1] : [y * cols + x, (y + 1) * cols + x + 1, (y + 1) * cols + x];
    const from = indexes.map(i => [mesh.columns[i % cols]!, mesh.rows[Math.floor(i / cols)]!] as const); const to = indexes.map(i => mesh.display[i]!);
    const [a, b, c] = from as [readonly [number, number], readonly [number, number], readonly [number, number]];
    const det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
    for (const [i, p] of piece.nativePolygon.entries()) {
      const u = ((b[1] - c[1]) * (p[0]! - c[0]) + (c[0] - b[0]) * (p[1]! - c[1])) / det;
      const v = ((c[1] - a[1]) * (p[0]! - c[0]) + (a[0] - c[0]) * (p[1]! - c[1])) / det;
      const w = 1 - u - v; assert.ok(u >= -1e-6 && v >= -1e-6 && w >= -1e-6);
      const display = piece.displayPolygon[i]!;
      assert.ok(Math.abs(display[0]! - (u * to[0]!.x + v * to[1]!.x + w * to[2]!.x)) < 1e-6);
      assert.ok(Math.abs(display[1]! - (u * to[0]!.y + v * to[1]!.y + w * to[2]!.y)) < 1e-6);
    }
  }
  for (const cell of expected) assert.ok(Math.abs((totals.get(cell) ?? 0) - 256) < 1e-6, `blocked cell ${cell}`);
  const svg = readFileSync(resolve(folder, 'ground.svg'), 'utf8'); for (const cell of expected) assert.ok(svg.includes(`blocked-native-cell-${cell}`));
}
test('every frozen blocked cell has its complete native support painted through the mesh', () => { coverage(support); assert.equal(hash(readFileSync(resolve(evidence, 'collision-support.json'))), art.sceneDescriptor.collisionSupport.proofSHA256); });
test('hostile mixed hash, moved anchor, aliased frame, physical board, mesh drift and false units are rejected', () => {
  const changes: ((a: Art) => void)[] = [a => { a.scenery.ground.sha256 = 'sha256:' + '0'.repeat(64); }, a => { a.scenery.objects[0]!.x = 1; }, a => { a.scenery.objects[0]!.frame = { x: 0, y: 0, width: 1, height: 1 }; }, a => { a.sceneDescriptor.boards[0]!.interactionId = 'invented' as unknown as null; }, a => { a.sceneDescriptor.mesh.display[0]!.x += 1; }, a => { a.resourceUnits.actualPngTransferBytes += 1; }];
  for (const change of changes) { const bad = structuredClone(art); change(bad); assert.throws(() => qualify(bad)); }
});
test('missing blocked support and corrupt PNG CRC fail closed', () => { const bad = structuredClone(support); bad.pieces = bad.pieces.filter(p => p.cell !== bad.blockedCells[0]); assert.throws(() => coverage(bad)); const png = readFileSync(resolve(folder, 'atlas.png')); png[50] = png[50]! ^ 1; assert.throws(() => decode(png)); });
