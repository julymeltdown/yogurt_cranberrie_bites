import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const out = process.env.QA06_MOBILE_OUTPUT
if (!out?.startsWith('/')) throw Error('QA06_MOBILE_OUTPUT required')
const report = JSON.parse(await readFile(resolve(out, 'report.json'), 'utf8'))
let android = null
try { android = JSON.parse(await readFile(resolve(out, 'android.json'), 'utf8')) } catch { /* optional */ }
const counts = report.cases.reduce((all, c) => ({ ...all, [c.verdict]: (all[c.verdict] ?? 0) + 1 }), {})
const iaIds = [...new Set(report.cases.filter(c => c.ia).map(c => c.ia))].sort()
const families = report.families.map(f => {
  const cases = report.cases.filter(c => c.family === f.id)
  const tally = cases.reduce((all, c) => ({ ...all, [c.verdict]: (all[c.verdict] ?? 0) + 1 }), {})
  return { id: f.id, name: f.name, tally }
})
const blockers = {}
for (const c of report.cases.filter(c => c.blocker)) (blockers[c.blocker] ??= new Set()).add(c.caseId)
const blockerLines = Object.entries(blockers).map(([blocker, ids]) => `| ${blocker} | ${[...ids].join(', ')} |`).join('\n')
const familyLines = families.map(f => {
  const status = f.tally.DIFFER || f.tally.FAIL ? 'DIFFER' : f.tally.PASS && f.tally.NOT_REACHABLE ? 'PARTIAL' : f.tally.PASS ? 'PASS' : 'NOT_REACHABLE'
  return `| ${f.id} | ${f.name} | ${status} | ${Object.entries(f.tally).map(([k, v]) => `${k} ${v}`).join(' · ')} |`
}).join('\n')
const iaLines = iaIds.map(id => {
  const cases = report.cases.filter(c => c.ia === id)
  const verdicts = report.widths.map(width => { const c = cases.find(x => x.width === width); return c ? `${width}:${c.verdict}` : `${width}:MISSING` }).join(' ')
  return `| ${id} | ${cases[0]?.route ?? ''} | ${verdicts} |`
}).join('\n')
const androidCounts = android ? android.cases.reduce((all, c) => ({ ...all, [c.verdict]: (all[c.verdict] ?? 0) + 1 }), {}) : null
const markdown = `# QA06 mobile visual acceptance — task 36

Candidate \`${report.candidate}\` on \`${report.branch}\`, base \`${report.base}\`. Harness: \`frontend/e2e/qa06-mobile/\` drives the actual default router, production ports, canonical Auth, Phaser world and the selected whole-character renderer through \`e2e/visual/qa06/preview.html\`. HTTP/C05 are deterministic synthetic fixtures; map/theme admission is TEST ONLY. This is desktop Chrome touch-viewport and Android-emulator evidence, not physical-device acceptance.

## Counts

Desktop Chrome ${report.browser}, viewports 360/390/430 CSS px, height 844 (one 420 short-height case), DPR 1:

| Verdict | Cases |
| --- | --- |
| PASS | ${counts.PASS ?? 0} |
| DIFFER | ${counts.DIFFER ?? 0} |
| NOT_REACHABLE | ${counts.NOT_REACHABLE ?? 0} |
| FAIL | ${counts.FAIL ?? 0} |
| **Total** | **${report.cases.length}** |

Case matrix: 61 retained IA entries (M10/R02/X02 are the optional P1 rows outside the required 61) at three widths, plus nine family-state cases (PARADE/REVENGE/DANGER era worlds, text-only feed, thread reply composer, admin status chrome, wall grid, short-height and keyboard-focus chrome) and one synthetic route-error-boundary case per width.

${android ? `Android ${android.device.model}, Android ${android.device.android} (SDK ${android.device.sdk}), Chrome ${android.chromeVersion}: ${android.cases.length} captures, ${androidCounts.CAPTURED ?? 0} captured, ${androidCounts.FAIL ?? 0} failed. Qualification: \`ANDROID_15_EMULATOR_CHROME_NOT_PHYSICAL\`. These emulator captures are from the earlier capture session and were not re-run here because no emulator was attached; the desktop matrix above was re-produced at the candidate shown.` : 'Android Chrome: NOT_RUN (see below).'}

## Checks

- Vitest, changed-file scope for the delivered source diff (\`npx vitest run --changed 7562f3b96\` from \`frontend/\`): 50 test files, 464 tests, all passed (exit 0). The earlier handoff note's 549-test figure was not reproduced; 464 is the measured count.
- \`tsc -b\`: exit 0. \`python3 scripts/mcr/check-plan.py\`: PASS, exit 0.

## 12 design families

| Family | Scope | Family verdict | Case verdicts |
| --- | --- | --- | --- |
${familyLines}

## 61 retained IA entries

| IA | Route | Verdicts 360/390/430 |
| --- | --- | --- |
${iaLines}

## Material differences fixed

1. **Profile edit footer link target** — \`frontend/src/features/me/ui/MePages.tsx:39\`: the "Back to world" link measured 21px high at all widths. It now uses the shared \`shell-link\` class (44px minimum, glyph size unchanged), verified at 360/390/430.
2. **Admin legacy access action target** — \`frontend/src/app/shell.css:54\`: the native "Check current access" button measured 25px high. \`.shell-actions > button\` now enforces a 44px minimum, verified at 360/390/430.

Both were the only automated target-size findings; after the fixes the audited set has zero \`TARGET_UNDER_44\` findings.

## Remaining material differences (recorded, not fixable in this scope)

- **Landing (PI10/FE08)**: the supplied ten-screen poster uses a distressed band wordmark, rose, moon/skyline and four-character skyline. The current app deliberately ships an independent fan-project title ("ONE WORLD. EVERY ERA."), the four selected source-pack characters, a crimson entry action and a legal/status block; brand-specific reference art is not reintroduced. The poster ends mid-viewport at 390/430 with plain paper below.
- **World (PI08)**: the target's festival scene, banners and crowd density are not the current studio scene supplied by the reviewed local input; HUD frame, era tabs, context strip and six-tab navigation match the Parade Ink anatomy. The reference level/points chip is omitted because no leveling system exists.
- **Feed (PI02/PI03)**: the target's illustrated media post cannot be rendered because no media/upload provider is composed; text-only cards, author row, reaction/share row and bottom composer match. Second post is test-fixture content.
- **Customize (PI07)**: the selected whole-character producer exposes a preset carousel (1–3/6, Random/Reset/Save), not the reference right-hand category rail and hair palette; layered per-part editing is not an available authority.
- **Profile / People (PI06/PI05)**: recent activity rows and other members' portraits are unavailable because activity/share/avatar providers are not composed; the current UI states this explicitly instead of fabricating rows.
- **Short height (PI01)**: at 420px the feed keeps its title, tabs and footer visible with the post list scrolling; no clipping or horizontal overflow.

## NOT_REACHABLE cases and their blocking producer

| Blocking producer | IA cases |
| --- | --- |
${blockerLines}

## NOT_RUN

- **Physical iOS Safari**: no iOS simulator or physical device is attached (\`xcrun simctl list devices booted\` returned none). Desktop viewport resizing is not substituted.
- **Physical Android Chrome**: no physical Android device is attached. The Android 15 emulator with Chrome ${android?.chromeVersion ?? '124.0.6367.219'} is used and labelled as emulator evidence.
- **Software keyboard and hardware safe-area insets**: not exercised; the captures use a zero-inset desktop/emulator viewport. Keyboard-open and notch/home-indicator behaviour remain NOT_RUN.
- **Live services and real multiuser journeys**: synthetic HTTP/C05 only; no live server, database or second account.

## Reproduction

\`\`\`sh
# from the worktree root with the retained dependency tree linked into frontend/node_modules
PATH=/tmp/mcr-node24/bin:$PATH QA06_MOBILE_OUTPUT=$PWD/.omo/evidence/mcr-finish-project/task-36-qa06 \\
  node frontend/e2e/qa06-mobile/native.mjs
PATH=/tmp/mcr-node24/bin:$PATH QA06_MOBILE_OUTPUT=$PWD/.omo/evidence/mcr-finish-project/task-36-qa06 \\
  node frontend/e2e/qa06-mobile/compare.mjs
\`\`\`

The Android pass additionally requires a booted emulator, \`adb reverse tcp:5319 tcp:5319\` and \`adb forward tcp:9222 localabstract:chrome_devtools_remote\`; run \`android.mjs\` after those forwards.

## Evidence files

- \`report.json\` — every case with case id, family/IA, width, capture path, verdict, measurements, blocking producer and material difference where applicable.
- \`captures/\` — ${report.cases.filter(c => c.capture).length} desktop PNGs${android ? ` + ${android.cases.length} Android PNGs` : ''}.
- \`sha256.txt\` — capture hashes.
- \`comparison.html\` — same-aspect target crops, Parade Ink component crops, family and IA tables.
- \`reference-crops/\` — developer-measured crops of the unchanged references.
- \`audit-pass/\` — the pre-fix audit run (U03/O05 target-size findings) retained for provenance.
- \`android.json\` — emulator device, Chrome version and per-case captures.
- \`venue-state-map.json\` — the 48 venue state-map cases with their QA06 NOT_REACHABLE disposition and blocking inputs.
- \`harness/\` and \`harness-sha256.txt\` — frozen copies and hashes of the capture harness used for this run.
`
await writeFile(resolve(out, 'README.md'), markdown)
console.log(resolve(out, 'README.md'))
