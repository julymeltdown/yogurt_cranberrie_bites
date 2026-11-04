# FE18 rooms and authorized Nearby client completion

Status: IN_PROGRESS. The corrected three-width native qualification and actual
application build passed on source d5247c9c. A focused 390px scrolling/keyboard
check remains for the additional Nearby focus recovery, followed by publication.

## Authority and ownership

Isolated `fe-ref-customize`, branch `codex/mcr-fe-FE18-client-completion`, fresh
published base `9356b943900ae4b4bf2c45f11118555e2c5d72ff`. Prior World/canonical/
mural branches and evidence remain preserved. Grant
`74ede9bad189e14373c80c5d595fb690d2f52692` was recorded before dispatch. The new
document/evidence paths were absent before writes. Actual prior goal was null;
new unbudgeted goal `1793769944` is ACTIVE through implementation, qualification,
fixes, committed Root153 handoff, publication/remote verification and cleanup.

Only `frontend/src/features/world-rooms/**`, `frontend/src/features/world-nearby/**`,
this document and `docs/mcr/evidence/fe18-rooms-nearby/**` are writable. Historical
`world-access.md`, root composition/router/Auth/transport/replication/renderer,
shared UI/contracts/assets/manifests/lockfiles/CI/backend remain read-only.
Root153 confirmed no shared production seam is needed. No earlier World grants
are inherited and no dependency was added.

Original FE18 requires a chooser, switch feedback, capacity states and an
accessible list of authorized avatars; full targets retain current membership,
and filtered characters disappear from both representations. Existing FE05,
FE16 and FE17 delivered clients already support deterministic work. Matching live
backend or device completion is not a prerequisite for these client checks.

## Feature correction

Existing client logic already uses canonical no-store C05 catalog reads, keeps
old membership/roster until correlated admission and treats dispatch as pending.
This patch fixes a real accessibility gap: successful admission removes the
originating switch button, leaving keyboard focus without a useful destination.
The local view now focuses the actual admitted/rejected/interrupted result when
focus remains at the initiating button or falls to the document after its removal.
It preserves focus if the user deliberately moves elsewhere. Confirmed target
membership has explicit feedback, the current room row exposes `aria-current`,
and the authorized Nearby list has an accessible name. It creates no network,
replication, game, authority, private cache or server occupancy source.

## Qualification design and provenance

New owned completion fixtures borrow the unchanged actual default Root/Phaser
source fixture at the assignment base. A test-only wrapper observes its existing
MockSocket and controls explicit frozen C05 responses; it never replaces the
client controller/transport decoder/renderer with an always-success admission
fake. Room catalog changes use only existing availability fields. Target-ticket
403 denial and `switchRejected` keep old membership; admission supplies the actual
outbound request ID and a replacement stream/epoch. One authorized Root projection
feeds the existing feature bridge and actual renderer. A filtered delta removes
a visible identity; proof checks the real Nearby DOM, real Phaser sprite removal
and changed pixels in that actor's actual painted canvas region, preserving
camera and fixture feet. Epoch/privacy/late-admission and full cleanup are checked.

Native code is newly authored; bounded resource/accounting patterns adapt the
prior owned World-presentation driver at `f60ff2fc` / published `9356b943`.
Root fixture implementation and selected artwork remain read-only imports, not
copied product rendering or new assets. Unit fixtures import the existing
MockWorldTransport/MockWorldClock and frozen movement/ticket/filtered-viewer
contracts. Source/contract/art hashes will be captured before the native run.
Three phone widths use actual controls, automated desktop keyboard input, 44px
checks, routes/focus and same-game/socket assertions. These do not certify
physical hardware, live BE37 filtering, production-art admission or IN01/IN05.

## W04 / W05 accounting

W05's client criterion is an accessible list from authorized, block-filtered
presence, with both DOM and actual painted-avatar withdrawal. Original FE18's
client work is tracked separately from broader live multiuser acceptance.

W04 additionally requires **party invitation**. Current closed C05 `WorldRoom`
contains only `room_id`, `label`, `capacity`, `availability`; there is no occupancy
count or invitation command/grant. Domain party operations also expose no invite
operation. `/meet` navigation is retained but is not invitation delivery. No
required IA scope is cut or silently marked satisfied.

The exact unresolved invitation criterion was routed directly to Root153 and
BackendSL21d. The named BE11 producer is World20f
`01a1a20f-8d37-71c3-a429-8a55bb6d6c49`; BackendRoot1fc owns shared contract
composition. Producer has supplied a SOURCE PROPOSAL, not a frozen contract:
existing Party members invited to a virtual room versus new Party membership
through ordinary approval remains an actual product interpretation pending the
user's answer requested by Root. Proposal revisions correct the existing
same-transaction Party audience ports and withdraw an unassigned migration path.
No proposed API/policy is consumed as authority here. Create/accept/expiry/revoke,
privacy, idempotency and room/Party binding remain OPEN / NOT_IMPLEMENTED under
that separate owner. It does not block the independent original FE18 client work.

## Source checks before native

Node24.22.2, retained dependencies; no install or lockfile changes. Focused owned
feature/FE17 checks passed 54 tests. The final combined run passed **221 tests in
13 files** (feature/controller/focus, actual entry bridge/root/resolver, FE17 and
composition). Full application `tsc -p frontend/tsconfig.json --incremental false
--pretty false`, scoped `oxlint`, driver `node --check`, `git diff --check` and
`python3 scripts/mcr/check-plan.py` exit0. The initial root-CWD invocation failed
two existing C06 relative-Python checks; from frontend CWD, system Python lacked
required date-time/uri format checkers. That second failure log is preserved.
The unchanged tests then pass using the existing contracts-validation-venv via
`MCR_FE14_PYTHON`; no checker/test/contract was weakened or installed.

Browser/build plan registered directly with BackendSL21d: one Chrome/profile and
one Vite server, expected190MiB whole task temporary+evidence peak, durable12MiB,
hard0.2GiB/40MiB. Expanded launch guard is now16GiB free, with8GiB stop floor;
zero other validation Chrome roots and preserved interactive browser are required.
A NEW named allocation and explicit acceptance of the committed clean source are
required; no old World slot is inherited. Actual optimized app build follows
browser closure sequentially with public asset copying disabled.


## First targeted native attempt

Source `691768d02fad0e01275444a84b3398f809bb803c` ran in explicitly accepted
slot `FE18-CLIENT-NATIVE-202611040538Z`, with36,240,809,984 bytes free and no
competing validation browser. The360px case reached ordinary catalog full/
unavailable,503 recovery,403 target denial, pending/rejected/correlated admitted
membership, keyboard result focus and DOM/sprite removal. Its strict painted
region assertion failed before that case completed. No full native pass or
application build is inferred. The original three images/report remain in
`native-01/`; `review.json` records the failure and bounded diagnosis.

The initial pixel oracle used `(world-scroll)*zoom` and omitted Phaser's camera
matrix origin. Only the owned test probe changes to the actual public camera
matrix transform; `changedChannels >20` stays mandatory. The new runner retains
partial case metrics and the after-removal image before asserting. No production
renderer/camera/authority/art/feet change. The later final type check exposed that Phaser generated declarations omit
the runtime BaseCamera matrix. The owned probe now reads that runtime property
through Reflect with a typed TransformMatrix and a runtime function guard; no
camera behavior changes. Initial measured peak77,455,513 bytes; own browser/server/temp
and borrowed link were removed, and the slot was explicitly returned before
requesting a new named retry allocation.


## Corrected native qualification and focused follow-up

`native-02/report.json` is PASS on exact capture source
`d5247c9c87233621afc58fc556d7e4423786b26e`, allocated slot
`FE18-CLIENT-RETRY-202611040541Z`. All three 360/390/430 x 844 cases passed,
including full/unavailable/503 recovery, 403 ticket denial, pending/rejected/
correlated admitted membership, result focus, 44px controls, no horizontal
overflow, one retained game/socket, epoch replacement, privacy cancellation and
late admission disposal. The actual removed actor is absent from DOM and actual
Phaser sprite state; its sampled paint changed 2295/2704/3369 channels. Fourteen
original PNGs and source/contract pins remain with their actual capture times.
The actual optimized application build passed after browser/server/profile
closure. Whole-task measured peak was 113,864,857 bytes. Own browser, server,
temporary directory and dependency link were removed; that slot was returned.

Visual inspection found the people names below the half-panel fold. The next
check scrolls a real person link into view, verifies its bounds and keyboard Tab
navigation, then records visible DOM and actual canvas before/after removal.
It reuses the prior three-width qualification only after comparing every shared
runtime/contract pin. It does not repeat that unchanged matrix. Nearby now also
recovers focus to the Other people region if the currently focused authorized
person disappears, while preserving deliberate focus elsewhere. Two added unit
cases cover those distinct behaviors; the combined regression run passes 223
checks in 13 files. The focused run includes a new sequential app build because
this additional Nearby accessibility behavior changes product source.
