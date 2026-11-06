// Records the developer disposition for every case that has a material reference
// difference. Verdicts come from native.mjs; this only attaches the named difference.
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const out = process.env.QA06_MOBILE_OUTPUT
if (!out?.startsWith('/')) throw Error('QA06_MOBILE_OUTPUT required')
const path = resolve(out, 'report.json')
const report = JSON.parse(await readFile(path, 'utf8'))
const differences = {
  A01: 'Reference band wordmark/rose/moon/skyline art is not reproduced; independent fan-project poster with selected source-pack characters keeps the title/character/action hierarchy. Public document ends mid-viewport.',
  A02: 'Reference Remember me control is absent; provider controls are present but explicitly unavailable. Email sign-in layout and action match.',
  A05: 'Reset request is a real form; delivery depends on the unavailable email account provider.',
  A08: 'Deletion status copy is present; retained recovery and completion actions stay unavailable after expiry/sign-out.',
  W01: 'Reviewed studio scene is used instead of the reference festival imagery; HUD frame, era tabs, context strip and six-tab navigation match. No level/points chip is fabricated.',
  F01: 'The reference illustrated media post cannot render without a media provider; text-only cards, author row, reactions/share and bottom composer match. The second post is deterministic fixture content.',
  F02: 'Image upload is explicitly unavailable; text composer and publish action match the shared control system.',
  F03: 'Reply composer and thread chrome match; comments are empty fixture data, not a layout difference.',
  F04: 'Image upload unavailable; save/discard/reload actions match, save label wraps at 360.',
  W04: 'Room switch actions are disabled while the covered panel suspends the world connection; row layout and copy are truthful.',
  W05: 'People list is empty while the world is paused; visibility/empty-state copy is truthful, no physical distance is implied.',
  W06: 'Era selection is local; server preference synchronization is explicitly unavailable.',
  M07: 'Other members\' portraits are unavailable without an authorized avatar provider; rows and search match.',
  M08: 'Introduction editor layout matches; audience/expiry capabilities are server-enforced.',
  M09: 'Recent activity rows require an activity provider and stay unavailable; profile framing and tabs match.',
  U01: 'Recent activity rows require an activity provider and stay unavailable; profile header, tabs and footer match.',
  U02: 'The selected whole-character producer exposes a preset carousel, not the reference layered category rail and hair palette.',
  U03: 'Profile form matches; optional ticket section/visibility controls are unavailable in this connection.',
  U06: 'Age and privacy copy matches; verification provider is unavailable and status is honest.',
  U07: 'Settings layout matches; sound applies only on explicit action.',
  U08: 'Safety service unavailable notice is shown instead of the blocked list; no action is sent.',
  U09: 'Account controls match; deletion is a real request flow and remains pending/recoverable.',
  U10: 'Inbox rows and unread state match; refresh/mark-read are real actions.',
  S01: 'Report form cannot start because the safety service producer is unavailable; no action is sent.',
  S02: 'Synthetic test-only route failure exercises the published unavailable boundary; no product data lost.',
  S03: 'Disconnected socket produces the reconnect state and clears presence.',
  'PI01-short': 'At 420px the feed keeps title, tabs and footer visible; the list scrolls without clipping.',
  'PI01-focus': 'Keyboard focus outline is present on the first feed control (crimson 3px ring).',
}
let annotated = 0
for (const item of report.cases) {
  const difference = differences[item.caseId]
  if (!difference) continue
  if (item.materialDifference) continue
  item.materialDifference = difference
  annotated++
}
report.materialDifferenceCount = annotated
try {
  const android = JSON.parse(await readFile(resolve(out, 'android.json'), 'utf8'))
  report.androidEmulator = {
    status: 'CAPTURED', qualification: android.device.qualification, device: android.device,
    chromeVersion: android.chromeVersion, cases: android.cases.length,
    captured: android.cases.filter(c => c.verdict === 'CAPTURED').length,
    failed: android.cases.filter(c => c.verdict === 'FAIL').length,
  }
} catch { report.androidEmulator = 'NOT_RUN' }
await writeFile(path, JSON.stringify(report, null, 2) + '\n')
console.log(JSON.stringify({ annotated, cases: report.cases.length }))
