// Builds the QA06 comparison artifact from captures and the unchanged reference images.
// Crops are developer-measured; the originals stay byte-identical.
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'

const repo = resolve(import.meta.dirname, '../../..')
const out = process.env.QA06_MOBILE_OUTPUT
if (!out?.startsWith('/')) throw Error('QA06_MOBILE_OUTPUT required')
const report = JSON.parse(await readFile(resolve(out, 'report.json'), 'utf8'))
const targetCrops = [
  ['A01', 'landing', { x: 4, y: 3, width: 300, height: 526 }],
  ['A02', 'login', { x: 309, y: 3, width: 275, height: 526 }],
  ['W01', 'world', { x: 589, y: 3, width: 370, height: 526 }],
  ['W02', 'map', { x: 963, y: 3, width: 272, height: 526 }],
  ['U02', 'customize', { x: 1240, y: 3, width: 294, height: 526 }],
  ['F01', 'feed', { x: 3, y: 539, width: 298, height: 482 }],
  ['M01', 'meet', { x: 307, y: 539, width: 295, height: 482 }],
  ['G01', 'gifts', { x: 607, y: 539, width: 309, height: 482 }],
  ['R01', 'wall', { x: 920, y: 539, width: 294, height: 482 }],
  ['U01', 'me', { x: 1218, y: 539, width: 316, height: 482 }],
]
const paradeCrops = [
  ['feed', '/tmp/pi-feed.png'], ['world', '/tmp/pi-world.png'], ['avatar-panel', '/tmp/pi-popover.png'],
  ['wall-editor', '/tmp/pi-wall.png'], ['tabs-toolbar', '/tmp/pi-tabs.png'], ['text-only-card', '/tmp/pi-textonly.png'],
]
await mkdir(resolve(out, 'reference-crops'), { recursive: true })
const py = `from PIL import Image\nimport json\nsrc=Image.open(${JSON.stringify(resolve(repo, 'docs/mcr/design/reference-target-2026-11-04.png'))})\ncrops=${JSON.stringify(targetCrops.map(([id, , r]) => [id, r]))}\nfor id,r in crops: src.crop((r['x'],r['y'],r['x']+r['width'],r['y']+r['height'])).save(${JSON.stringify(resolve(out, 'reference-crops'))}+'/'+id+'.png')\n`
execFileSync('python3', ['-c', py])
for (const [name, path] of paradeCrops) execFileSync('cp', [path, resolve(out, 'reference-crops', 'parade-' + name + '.png')])
const esc = value => String(value).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]))
const shot = (c, w = 270) => `<figure><figcaption>${esc(c.caseId)} ${esc(c.verdict)} · ${c.width}×${c.height}</figcaption><a href="${esc(c.capture)}"><img src="${esc(c.capture)}" style="width:${w}px"></a>${c.blocker ? `<p class="blocker">${esc(c.blocker)}</p>` : ''}</figure>`
const ten = targetCrops.map(([id, name, rect]) => {
  const shots = report.cases.filter(c => (c.caseId === id || (id === 'W01' && c.caseId === 'W01-parade')) && c.width === 390 && c.capture)
  const w = 270, h = Math.round(w * rect.height / rect.width)
  return `<article id="target-${esc(id)}"><h2>${esc(id)} ${esc(name)}</h2><div class="row"><figure><figcaption>Supplied reference ${rect.width}×${rect.height}</figcaption><div class="crop" style="width:${w}px;height:${h}px"><img alt="reference" src="reference-crops/${esc(id)}.png" style="width:${w}px"></div></figure>${shots.map(c => shot(c)).join('')}</div></article>`
}).join('')
const parade = paradeCrops.map(([name]) => {
  const map = { feed: 'F01', world: 'W01-parade', 'avatar-panel': 'U02', 'wall-editor': 'R01', 'tabs-toolbar': 'F01', 'text-only-card': 'PI02-text-only' }
  const id = map[name], shots = report.cases.filter(c => c.caseId === id && c.width === 390 && c.capture)
  return `<article><h2>Parade Ink component · ${esc(name)} → ${esc(id)}</h2><div class="row"><figure><figcaption>Developer-measured crop of the unchanged Photo 2</figcaption><img alt="reference" src="reference-crops/parade-${esc(name)}.png" style="width:270px"></figure>${shots.map(c => shot(c)).join('')}</div></article>`
}).join('')
const familyRows = report.families.map(f => {
  const cases = report.cases.filter(c => c.family === f.id)
  const counts = cases.reduce((all, c) => ({ ...all, [c.verdict]: (all[c.verdict] ?? 0) + 1 }), {})
  return `<tr><th>${esc(f.id)}</th><td>${esc(f.name)}</td><td>${esc(Object.entries(counts).map(([k, v]) => `${k}: ${v}`).join(' · '))}</td></tr>`
}).join('')
const ia = [...new Set(report.cases.filter(c => c.ia).map(c => c.ia))].sort()
const iaRows = ia.map(id => {
  const cases = report.cases.filter(c => c.ia === id)
  const verdicts = cases.map(c => `${c.width}:${c.verdict}`).join(' ')
  const blocker = cases.find(c => c.blocker)?.blocker
  return `<tr><th>${esc(id)}</th><td>${esc(cases[0]?.route ?? '')}</td><td>${esc(verdicts)}</td><td>${esc(blocker ?? '')}</td></tr>`
}).join('')
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>QA06 mobile visual acceptance</title>
<style>body{margin:0 auto;padding:24px;max-width:1500px;background:#f5f3eb;color:#232320;font:15px/1.45 system-ui}h1{font-size:26px}h2{font-size:19px;margin-top:28px}small{font-weight:400}table{border-collapse:collapse;width:100%;font-size:13px}td,th{border:1px solid #c9c6b8;padding:4px 6px;text-align:left;vertical-align:top}th{background:#efece1;white-space:nowrap}.row{display:flex;gap:16px;overflow:auto;align-items:flex-start}figure{margin:0;flex:none}figcaption{max-width:270px;font-size:12px;min-height:32px}a{color:#284b6b}img{display:block;height:auto;border:1px solid #b9b5a6}.crop{overflow:hidden}.blocker{max-width:270px;font-size:12px;color:#7a2b2b}</style>
<h1>QA06 mobile visual acceptance</h1><p>Candidate <code>${esc(report.candidate)}</code> · ${report.cases.length} capture cases · ${esc(report.browser)} · ${esc(report.qualification)}</p>
<h2>12 design families</h2><table><tr><th>Family</th><th>Scope</th><th>Case verdicts</th></tr>${familyRows}</table>
<h2>61 retained IA entries</h2><p>${ia.length} retained entries mapped; M10, R02 and X02 are the optional P1 rows outside the required 61.</p><table><tr><th>IA</th><th>Route</th><th>Verdicts 360/390/430</th><th>Blocking producer</th></tr>${iaRows}</table>
<h2>Ten-screen target · 390px same-aspect pairs</h2>${ten}
<h2>Parade Ink component crops</h2>${parade}
<p>Reference originals are unchanged. Crops are developer-measured and keep the original aspect; no nonuniform scaling.</p>
</html>`
await writeFile(resolve(out, 'comparison.html'), html)
console.log(resolve(out, 'comparison.html'))
