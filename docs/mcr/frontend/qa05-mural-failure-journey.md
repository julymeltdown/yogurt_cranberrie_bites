# QA05 mural failure journey

This is an independently runnable **fixture contribution to original QA05**.
It does not complete whole QA05, admit a production Wall supplier, or satisfy
live-service/device/IN04 acceptance. Community allocated this exact new-only
scope from `93ef0d895dcc4b247b727e2cccba0bb84576b262` on
`codex/mcr-qa-QA05-mural-failures`, in isolated `fe-ref-customize`.

Only `frontend/e2e/realtime/mural/**`,
`docs/mcr/evidence/realtime/mural/**` and this new document are writable.
No application, backend, generated/contract, shared helper, parent config,
manifest, lock or existing World-suite file changes belong to this assignment.
The original QA10 work/branches and World-only QA05 contribution remain intact.
The new unbudgeted goal began at epoch `1793760667`; the developer owns checks
and fixes through designated-integrator publication and remote verification.

## Inputs and actual execution boundary

The fixture consumes the published FE33 lifecycle `7c2f3af7b9fd7bab2ed2d863cc2262527cc8553e`,
caller `ebf0473dd8f2123b74d77ca3f4de3da5abd0df9a` published through
`eeae69be3ebb7fafafa92f5e6b8f4141797ed102`, frozen app mural port
`aedbb020e637993fe9ef1831e31bd6ae5316ae0c`, and C04 own-result
`bff7e53c0638d5462af819bf560468a1f857c88e`. Current native reports pin actual
source/contract bytes rather than assuming these historical commits are current.
All rendering and state transitions exercised by the journey are incumbent code.

Two isolated native Chrome contexts run actual canonical Auth, production custom
load, World composition, MapScene/Phaser, WorldPanels/MuralEntryProvider, mural
sync/projection/editor, scene sink and C04 HTTP decoders. Each shell uses one Auth
fixture identity, one actual game and one synthetic world connection. Both clients
use the same canonical **fixture account**; this is not two real participants.

The local Vite middleware is an explicitly synthetic C04 mural peer. It supplies
shared state and selected HTTP faults; it imports no backend implementation and
opens no extra mural socket. Snapshot/delta/place/private own-result pass through
the actual authenticated client transport/decoders. Auth login, cookies, world
tickets/admission/MockSocket, synthetic C06 terrain, write eligibility and the
virtual mural anchor are fixtures. None establishes a production supplier or
server permission. Omitted supplier is separately tested to stay closed with
zero mural requests while the World remains live.

Reordered deltas use the published **leased in-process receiveDelta interface**;
no new public mural WebSocket message is invented. HTTP polling/snapshot recovery
is independently exercised. Shared source fixtures and the existing World QA05
native-browser helper are read-only imports, with source pins and attribution.
No existing caller, helper, test selector or production behavior is modified.

The peer models separate pause, restore and resume boundaries. Each increments
revision and changes snapshot UUID; old cursors require a fresh snapshot. Restore
requires PAUSED and remains PAUSED, while prior command receipts/operation history
survive. The real producer owner supplied this compatibility clarification for
user Wall `57dc31e1c8f8e1ba0f203d71a50cc3f1629bc8fa` and additive operators
`bcf81212036872e8b0d2b9694a21f6d97b3f091b`. This fixture is not execution of
those backend commits or public operator authorization.

## Original QA05 criterion matrix

| Original criterion | This contribution | Remaining boundary |
| --- | --- | --- |
| Packet delay/gap | Native fixture reorders adjacent deltas, duplicates a delivered range, omits a revision through the real gap deadline and HTTP snapshot recovery. | Real admitted transport/network qualification remains separate. |
| Slow client | Native 390px case holds a real HTTP delta until the consumer's 8s deadline, bounds single-flight reads, withdraws raster and recovers by snapshot. | No backend load/capacity/mobile network claim. |
| Background/foreground | Dedicated native Chrome via CDP `noDefaults` switches actual tabs, observes hidden/visible, withdraws the qualified raster and requires explicit fresh qualification. | No physical iOS/Android result. Ordinary contexts deliberately emulate visibility for the two-client cases. |
| Ticket replay | NOT_RUN here. | Actual one-use world ticket replay belongs to the existing World/backend qualification. Synthetic tickets are not proof. |
| Server restart | Fixture peer generation restart retains simulated committed command state; fresh qualification checks the original own-result without another POST. | **Real-service restart NOT_RUN.** No process/DB/outbox durability result is claimed. |
| Room switch | NOT_RUN here. | Existing World QA05 owner retains authoritative room-switch and membership coverage; this package does not duplicate it. |
| Bounded queues | Actual mural projection reaches 512 future operations; the next operation resets rather than growing the queue. Native rasters recover to current authority; inflight client requests stay at most two. | No global server queue/capacity acceptance. |
| No ghost player / duplicate membership | Actual client checks one game/connection per shell and no mural texture/object, source/socket listeners or pending HTTP after withdrawal/privacy/disposal. | These are **mural resource assertions**, not authoritative player or room-membership proof. Full original criterion remains open. |
| Eventual convergence | Both actual confirmed-pixel hashes and Phaser RGBA texture hashes equal the synthetic authority after gap, overflow, result recovery, pause/restore and resume. | Real multiuser/IN04 remains open. |

Additional mural-specific assertions use normal visible DOM preview/Confirm clicks,
verify definitive rejection clears selection while preserving confirmed revision
and pixels, retain unknown commands across withdrawal, prevent automatic POST
replay, and reject captured obsolete leases after restore and tab withdrawal.
All final World-route/authority/privacy behavior remains incumbent code.

## Reproduction and evidence

Use retained Node **24.22.2** and the existing dependency installation; do not
install or change the lockfile. From `frontend/`:

```sh
node e2e/realtime/mural/check-types.mjs
node --test e2e/realtime/mural/peer.node.mjs
node node_modules/oxlint/bin/oxlint --deny-warnings e2e/realtime/mural
node e2e/realtime/mural/native.mjs --candidate FULL_COMMITTED_HEAD
```

The native runner requires exact committed HEAD, clean tracked source, hashes
all relevant source/contract inputs, and checks them again at completion. It
writes a fresh candidate-specific directory without overwriting earlier runs, and
records exact candidate/tree, Chrome versions, viewport steps, observed states,
peer requests, raster hashes, source pins and capture hashes. A development-only
`--development` run is explicitly marked uncommitted; it cannot certify the named
Git candidate. The runner retains a failure report on assertion/runtime failure,
closes its own browsers/peer/server and removes its own temporary cache/profile.
At least 8 GiB free disk is required. The only borrowed dependency link is the
explicit read-only target; it is removed at STOP.

The first development attempt failed a normal Confirm click because the fixture
World host lacked a positioned containing block: the existing interaction overlay
covered the editor. The fixture host now has relative positioning and clipping.
The original failure/log remain in `development-first-*`; no forced click,
selector suppression, blind sleep or application edit hides it. The corrected
development run passed three widths plus native visibility. Final candidate
qualification records the separate pause/restore boundaries and additional
resource assertions. These are developer-owned checks, without a managerial
review or approval stage.

Final execution, committed handoff and publication results are recorded below
when actually observed. No planned command is counted as a result.

## Executed qualification and ready handoff

Source commits: `a2b926b8b1188f00a2de211aeff1902148d49cbb` then
`7195d78cc72078d4f0403566bee01f47f9a85edf`. The exact final native candidate is
**`7195d78cc72078d4f0403566bee01f47f9a85edf`**, tree
`5b5b196d606d4cc90cf36dd5750f6b754f07d9d6`. Its tracked source was clean and
all 1,261 recorded input hashes matched the committed candidate before execution
and were checked again after it. All five supplied module/caller/app/C04 source
commits are actual ancestors (`inputs.json`). The retained pnpm lock digest is
`47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`, identical
to the borrowed installed lock. No dependencies were installed.

`provenance.json` records source base, Git blobs, SHA-256 and destinations for
MCR fixture adaptations: existing `assembly-data.ts` to `data.ts`, actual caller
native pattern to `preview.tsx`, the synthetic editor palette/DTO seed to
`peer.mjs`, and the focused compiler pattern to `check-types.mjs`. The World QA05
`browser.mjs` is imported read-only for real native visibility. This is MCR test
reuse, not legacy rendering reuse or a second implementation of production sync.
The actual HTTP/editor/projection/lifecycle/Phaser implementation is imported
unchanged and remains the subject under test.

| Executed command | Actual exit/result |
| --- | --- |
| `node e2e/realtime/mural/check-types.mjs` | 0; retained strict compiler options and actual transitive imports, private temporary compiler configuration. |
| `node --test e2e/realtime/mural/peer.node.mjs` | 0; 2/2 meaningful peer control-boundary/bounded-hold tests, no skipped cases. |
| `node node_modules/oxlint/bin/oxlint --deny-warnings e2e/realtime/mural` | 0; no warnings. |
| Vitest six explicit lifecycle/journey/entry-owner/sync-controller/projection/HTTP test files, `--configLoader runner --no-cache --maxWorkers=2 --reporter=json` | 0; **124/124 tests across 6 files**, zero failures/skips, exact paths in `consumer-tests.json`. |
| `node e2e/realtime/mural/native.mjs --candidate 7195d78cc72078d4f0403566bee01f47f9a85edf` | 0; **Chrome 156.0.8078.80**, 360/390/430px at 844px height, 11/12/11 recorded checkpoints, plus independent native hidden/visible and omitted-source cases; zero page errors. |
| `python3 scripts/mcr/check-plan.py` | 0; planning integrity only. |
| `git diff --check` and exact staged-path check | 0; assigned new paths only. |

The final native report is
`docs/mcr/evidence/realtime/mural/candidate-7195d78c/native.json`; three
`recovered-{360,390,430}.png` captures have recorded SHA-256. The 390px capture was
visually inspected to confirm the real fixture editor and scene rendered. This is
functional fixture inspection, not product visual fidelity/QA06 acceptance.

Each width reached confirmed revision **532** after separate pause 530, restore
531 and resume 532 boundaries. Both clients' confirmed pixels and actual scene
RGBA hashes equal the peer. The first client's observed projection queue peaked
at **512**, then reset to zero on overflow. Each shell created exactly one game
with peak one, and ordinary foreground cases retained exactly one world socket.
Client HTTP peak was one in the actual run (asserted ceiling two); the peer's
peak active HTTP was at most two and held reads at most one. Per width, exactly
two placement POSTs occurred: one definitively rejected, one committed with a lost
acknowledgement. Recovery used the original key/result; **zero replayed POSTs**.

The 390px held delta hit the actual 8s timeout and cleared the texture before
fresh snapshot recovery. Cancellation also removed a held HTTP snapshot. At final
privacy/disposal, both clients had disposed sync, null confirmed/selection/command,
zero queue, zero game/socket/source/socket listeners, and zero inflight HTTP;
scene texture/object were gone. The peer had zero pending/active reads/holds.

The dedicated native tab case observed `hidden`/World `suspended` with zero live
socket, no mural release/texture, then `visible`/World `live`. It kept one game,
created a replacement world socket sequentially (two lifetime sockets, **one live**),
and required fresh mural qualification before revision 11 appeared. The old lease
was rejected. The omitted-supplier control stayed live with no mural factory,
release or HTTP request. These are observable native states, not a dispatched
synthetic visibility event.

An initial frozen preflight exited 1 when shared free disk fell to 7.55 GiB, below
the 8 GiB entry floor. The log remains `candidate-preflight-disk.log`. The current
storage owner reclaimed only its own retired executable copies; this developer
performed no cross-owner cleanup. Fresh local `statvfs` measured 9.533 GiB, then
the unchanged final candidate passed preflight and executed. The earlier corrected
development PASS remains explicitly uncommitted evidence, not substituted for the
final candidate. All initial failure records remain intact.

The assigned **native fixture contribution is qualified**. Original QA05 remains
partial: real ticket replay, actual service restart, authoritative room switching,
no ghost players/duplicate membership, device/runtime load and live IN04 are not
promoted by this packet. Production mural supply/anchors/eligibility remain closed
as before. No backend or device prerequisite was imposed on the frozen-capable work.

The designated integrator Root153 publishes the scoped handoff. Actual remote
ancestry/payload verification and final cleanup/goal closure follow publication;
no developer push or king/SL review gate is required.
