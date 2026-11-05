# BE10 admission tickets and BE11 membership leases

Status: **BE10 qualification complete; BE11 membership qualification complete; committed publication pending. BE11/W04 remains IN_PROGRESS because party invitation is OPEN.** Owner: implementation chat `01a1a1fd-6b5d-75b0-8d42-01ade79b9393`. Fresh focused validation: **8 application tests PASS, 2 actual PostgreSQL cases PASS, migration verification and cleanup PASS**. Historical results retain their original source and limits.

## Durable assignment and completion criteria

Deliver the whole original BE10 and BE11 tasks from `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, rows 465–466, rather than a new task slice. Reconcile original criteria with actual source and responsible-owner evidence, implement genuine remaining gaps, perform relevant developer-owned validation and fixes, record original-task dispositions and limits, and provide a committed handoff directly to standing publisher412e (`01a1a5d9-332b-7822-a8f7-412e295fa211`, `mcr(delivery) Master publication`) for publication. Earlier Root1fc publication routing is superseded; there is no Root/King/SL review or acknowledgment gate. No king/SL review or acknowledgment is required. IN01/IN04, QA05, BE13/14 and QA08 retain their separate scopes; the user-deferred QA08 200-person/device/performance workload is not a prerequisite invented for these tasks.

The persistent-goal API was inspected on assignment: its historical `BE-PROFILE-SELF` goal is `BLOCKED` and incompatible with this assignment. It remains truthful and unchanged. This durable objective records the separately authorized whole-task assignment; it does not claim a new API goal is active or complete.

Immutable base: `2492e35bb9fb61380bf97fa936983ec16c0722f4`. Branch: `codex/mcr-be-BE10-BE11-admission-leases`. This isolated sparse worktree does not modify the frozen BE06 candidate `1289d90e9fe5bc6fee04858aee829c9e54b030ab` or its validation queue. The BE04/07 document from that candidate was separately published as `df244a715a7a7d109282029dee78df4bf65de10d`. BE06 subsequently passed all three ordered native cases, real PostgreSQL restart, ledger checks and cleanup at helper `de93c210d`; final source `d02fbf3bd` was published as `66551b07dacf5fd09d6f959b03c1ef6246d9b4ba`. That completed work is not replayed as a prerequisite here.

## Custody and provenance

World20f explicitly stopped its existing writers. Root1fc then transferred only application `world_ticket/mod.rs` and `tests/world_ticket.rs`, service World membership/ticket/room repository hunks and direct tests, existing `world_admission.rs`, the two historical admission smoke scripts, and new completion/evidence/test support paths. World RPC, admission bridge, runtime, session/recovery, ingress/invalidation, root composition/exports/configuration, Auth authority, shared contracts/proto, manifests and lockfiles remain outside this patch. No source-pack or frozen `docs/source/` bytes change.

The application and repository are retained MCR code at the immutable base; this patch adds no imported rendering, package, migration or dependency. New native cases reuse the existing marker/inode-owned fixture and its real adapter, with a two-connection pool. The dedicated runner `backend/services/concert-service/tests/support/world_admission_leases/round.py` adapts the BE06 runner protocol from `1289d90e9fe5bc6fee04858aee829c9e54b030ab`, retains `migration_smoke.Cluster` custody, applies migrations through the exact frozen service executable and verifies every migration ledger SHA-384. The old admission smoke runner's four historical direct-SQL migrations are insufficient for the current adapter's deletion and event-closure dependencies; historical receipts are preserved rather than rerun with an incomplete schema.

## Original criterion reconciliation

| Task / criterion | Current implementation and existing owner evidence | Delivered qualification |
| --- | --- | --- |
| BE10 user/session/event/room-bound 30-second ticket | `concert-application/src/world_ticket/mod.rs`, `concert-service/src/world_rpc.rs` and durable `world/mod.rs`; expiry is the minimum of 30 seconds and access expiry. Session is the canonical Auth refresh-family UUID, never a provider identity or refresh-token row. Existing application and producer receipts cover these bindings. | Milliseconds correction delivered; eight-test application suite PASS, including the five-second boundary. |
| BE10 atomic one use; reuse/wrong-room/expiry denial | Digest-only PostgreSQL ticket row, serialized consumption, wrong pin consumed before rejection; `ticket_digest_is_single_use_under_competing_connections`, existing application cases and real Auth runtime51 evidence. Admission bridge consumes before its subsequent Auth/Concert checks, so later denial cannot replay the ticket. | Reuse unchanged-path evidence; no new full matrix. |
| BE10 no credentials in URLs/logs; wrong Origin and unauthenticated frames denied | Gateway `concert_world.rs`, runtime/socket ingress, first-frame three-second deadline, opaque-ticket redaction; historical runtime/socket/native producer records and present closed schemas. | No ownership or policy change in these paths; preserve their historical evidence limits. |
| BE11 one owner, bounded capacity, session membership | Owner process/epoch leases; transactional room locks and capacity; primary key `(event_id,user_id)` plus unique connection; historical four native admission cases and later native bridge receipts. | Both stored owner predicates delivered; actual-PG process-only and epoch-only takeover case PASS. |
| BE11 join/switch/leave without duplicate membership | Durable admit/switch transactions, existing actual concurrent capacity/switch evidence; failed full-room switch preserves current membership. | Actual-PG leave authorization and canonical-row removal PASS; existing capacity/switch evidence retained. |
| BE11 disconnect grace and ghost expiry | Durable disconnect marks the row disconnected; strict lease expiry and bounded prune remove expired rows. | Actual-PG grace, exact expiry and canonical ghost-row pruning PASS, without a wall-clock sleep. |

## Corrections and focused regressions

`WorldService::issue_verified`, called by the actual ticket issuance RPC, used `Duration::seconds(FRESH_AUTHORITY_MILLISECONDS)` with constant `5000`. This admitted release observations for up to 5000 seconds. It now uses milliseconds. `typed_issuance_enforces_release_freshness_before_persisting` checks ages 4999, 5000, 5001 and 60000 milliseconds and a future observation. Only 4999 milliseconds can persist a ticket, with its existing 30-second expiry.

`PgWorldRepository::current_owned_membership` previously joined live membership and room-owner leases only by event and room. `reauthorize` matched the supplied owner to that lease but omitted the membership's stored owner. Both could label a still-live previous-generation row with a replacement owner. Both queries now require the membership's stored process and epoch to match the current owner lease. `owner_takeover_does_not_relabel_an_existing_membership` independently changes process and epoch, keeps the old membership lease live, denies old and relabeled subscriptions, and recognizes the replacement only after an actual fresh ticket/admission. `membership_leave_and_disconnect_grace_expire_canonical_rows` checks wrong-session leave denial, correct leave removal, disconnected grace state, exact expiry denial and actual ghost pruning. Injected times are microsecond-aligned to PostgreSQL precision.

## Existing evidence and limits

Responsible World20f and Auth207 owners provided their current dispositions directly. Existing qualified original components are not reopened for a duplicate whole matrix. Evidence anchors include:

- Admission source `279aad1b391df8fa6589e2f69d6bf3cdf6c6b255`, native receipt SHA-256 `c68a2a057eb6c01485f9a43f059c0f73abcd6ebe306bb97ed14d56baa3a970c6`: four actual-PG cases, restart and cleanup PASS. See `world-admission.md` for application commands and the historical fixture boundaries.
- Producer source `f7451692f4270984ab75fb37a9a589fa3fda6bc7`, native receipt SHA-256 `f25ca6a36a5837ce955de85a2d0341b6f5aaa7fe3128e0dbb7c60f0034ddea71`: four native cases/restart/cleanup; recorded Gateway21 and WorldRPC4 focused passes. These are not a new public-cohort result.
- Remote invalidation source `4c0a04689a6f57e078d3a15d80583700243a553c`, receipt SHA-256 `b280b936f1b907cb8b6c67cbea683471f2f719182b4676fec075e24caa233b95`: twelve native cases, configuration and physical restart/cleanup. Later combined source and original BE10/11 criterion mapping are recorded in `world-remote-invalidation.md`.
- Actual Auth runtime51 motion/legacy pair source `9c12e1c`, publication `1e20c294`, passed twice. See `world-capacity-qualification.md` and `qa05-runtime51-fault-probe-intake.json` for real signup/verification/login, two-person/two-room boundary and the original clock, fault/restart/cleanup limits.
- Latest recovery source `b966f0ff4368d8ce4b4f60bfd4098282657deb75`, publication `9e4ace4dc`, selected 89 passes and four ignored PostgreSQL bridge cases **NOT_RUN** on that source. `world-recovery-completion.md` preserves that distinction. Old native bridge results are not fresh passes of those four ignored cases.

Auth207's read-only reconciliation rehashed its cited receipts; this assignment has not rerun them. The new queries and five-second boundary passed the targeted fresh qualification below. The Auth read port proves current active account and exact live refresh family within its bounds; it alone does not replace Concert admission, consent or continuous invalidation. New BE38 release-carry work is separate. The historical public attempt09 FAIL remains FAIL; Root's latest same-cohort IN04 journey and actual managed/operator/Admin/device/load acceptance are not claimed here.

## Fresh qualification and source equivalence

Implementation commit `bd78f02bb` contains the corrections and focused regressions.
Build02 compiled source `fa192614f06862ae185e2e7c9e79caca06ccb798`, with all
2,536 compilation/source inputs pinned by readset SHA-256
`40dd5ffb791966146083923cada9e195e4a9439dd03899b59477608a95a12d30`.
Native03 ran committed helper/source `02c9922912e02bf30037fbc5f3b8057c5c3403d6`,
with new readset SHA-256 `3e5388b7d591fbea0ad1f0d537d97d44ec690ae16d9377950613f17d83b8af84`.
All 2,536 pins were freshly compared: only the Python fixture helper changed.
Every Rust input and all 178 SQL migration input bytes remained identical to the
actual build. The final completion-record successor changes documentation only;
it does not require a binary rebuild or repeat of already qualified tests.

The two retained sparse includes were materialized byte-for-byte from the source:
`contracts/generated/mcr-domain.openapi.json`, 2,310,694 bytes, SHA-256
`9ae294507f6c2587c7354bd5014d45e76686a8a1cec3d783dfd79db6b6c49d65`, and
`docs/mcr/contracts/avatar-motion-v2/review/visual-review.json`, 8,109 bytes,
SHA-256 `27dd1f7f1d31b792f4da415cf6daf6eafc97e91023443a27ac70480f75ce5956`.
Neither is a contract/art modification or an independent acceptance result.

### Build02 and eight application tests

Offer `BE10-BE11-FINISH-BUILD02-0326` accepted at epoch `1793850548.665846`.
Both exact commands exited **0**, taking 104.022 seconds in total:

```sh
cargo +1.88.0 test --manifest-path /Users/lhs/Desktop/mcr-worktrees/be-world-admission-leases-6b5d/backend/Cargo.toml --locked --offline -j 1 --config profile.dev.package.contracts-build.opt-level=2 --message-format=json -p concert-application --test world_ticket --no-run
cargo +1.88.0 test --manifest-path /Users/lhs/Desktop/mcr-worktrees/be-world-admission-leases-6b5d/backend/Cargo.toml --locked --offline -j 1 --config profile.dev.package.contracts-build.opt-level=2 --message-format=json -p concert-service --test world_admission --no-run
```

The allocated existing warm target used jobs1, dev/test debug0, incremental0,
private TMP, and cleared inherited compiler wrappers/flags and make job controls.
The actual toolchain was selected by `cargo +1.88.0`; no separate version command
was recorded. Three immutable copies were frozen before immediate target release:

| Executable | Bytes | SHA-256 |
| --- | ---: | --- |
| `world_ticket` | 2,490,352 | `228030246dc8443b3cd59fa7c773899d688a3cdfe2fa391559d44805ae5cfb39` |
| `world_admission` | 13,052,224 | `9e95bd0dd95805e68e1d77ba84dd3dcd9488f29edb05bfd972c5e77c9991e8ba` |
| `concert-service` | 65,201,024 | `c1ea053d010818ba81014728a2d1372ac9f5dba506ff5ee564fc1c34d3788b83` |

TARGET released at `1793850685.937458`. The frozen `world_ticket --test-threads=1`
then passed **8 tests, 0 failed, 0 ignored, exit0**, including the new typed freshness
boundary. FULL released at `1793850693.613914`, tool59852 exit0, no failure,
remaining owned PIDs or reservation, workflow0 and target access NONE.
Both independent initial baselines passed; the guardian exited without failure.

Immutable raw directory:
`/private/tmp/mcr-be10-be11-357e-preparation-20261105/build02-preparation/`.

| Receipt | SHA-256 |
| --- | --- |
| `TARGET.json` | `7277a8aba2e741b42cb26deb3c827ae3a254351bf5b97620a5c0235d0d6ac0c0` |
| `FULL.json` | `6e96315adf0b5e11be123c4238d8185e80b07a4ef393353627378274219f126c` |
| `build-receipt.json` | `97415673edd6212ef4813243b2f782b3705114e6d832d72462b764afd25f8771` |
| `world_ticket.stdout` | `b170dbdfda332f5f9229caf9b42ba3f34833295a37ee6a631d4f943cdcb9c596` |

The related `unit-result.json` preserves the executable invocation and outcome.
The separate `build02-target-input-handoff.json` in the parent directory has SHA-256
`1b0c07250b7d437506da9f31e3848495e2ed9d24e3ecf2f056a8cdcd9060ddb8`.
It records actual final unique-inode target bytes 19,629,146,112 and final actual
root union 27,477,065,728. Numeric initial root inventories were not separately
retained; initial completion/time and final census were retained. The carried
WHOLE maximum 59,164,655,008 is a nonshrinking accounting floor, not current measured
occupancy. Build peak NEW383,475,712/outside216,539,136/target growth166,936,576;
max7 observed PIDs, RSS2,300,362,752 and minimum free22,978,371,584 bytes.

### Native03: actual PG16.14 membership cases

Offer `BE10-BE11-COPIED-NATIVE03-0415` accepted at `1793852130.979421`;
original WORK/FULL deadlines were `1793852430.979421` / `1793852610.979421`.
Actual FULL was `1793852160.967514`, tool61994 exit0, failureNULL, workflow0,
remaining owned PIDs empty, reservation0 and target access NONE.
Both independently complete initial baselines preceded payload; independent
failureNULL and its guardian was reaped. No target, Cargo, executable copy,
browser, HTTP stack or unit-suite replay occurred in this round.

Existing Homebrew PostgreSQL16.14 was used under marker/inode-owned short root
`/private/tmp/mcr-be10-be11-357e-preparation-20261105/n3/p` (78-byte socket path).
The frozen production service actually migrated and verified dedicated
`mcr_concert`. The committed helper then cloned that database as `mcr_world`,
checked matching ledgers and ran the immutable tests with their strict
`mcr_world` guard intact. Migration/verify/clone and both exact tests exited0:

- `owner_takeover_does_not_relabel_an_existing_membership`: **PASS**, 1 passed,
  0 failed, 0 ignored, 5 filtered out.
- `membership_leave_and_disconnect_grace_expire_canonical_rows`: **PASS**,
  1 passed, 0 failed, 0 ignored, 5 filtered out.

The wrapper checked the exact 178 SQL input set/hashes and both binaries before
and after every CLI/case; both databases' SHA-384 ledgers were checked before and
after each case, followed by final source/binary checks. Cleanup **PASS** removed
the owned cluster/processes. Native proof uses fixture authority; it does not
claim a fresh public Auth/World vertical journey, restart, device or 200-user test.

Immutable raw directory:
`/private/tmp/mcr-be10-be11-357e-preparation-20261105/n3/`.

| Receipt | SHA-256 |
| --- | --- |
| `FULL.json` | `34fd68f17ee1f0185f979917c37505ed4c32dcf42538a28df99c621d225054e3` |
| `native-report.json` | `89d15c597745d40f3b2ff919cae24c327c017b87b25dbb88220451d60796798b` |
| `native-cohort-phases.json` | `2171c5ab5551d849e5b13e90c270d7c462788923fdb5462d1bdf745f982cd316` |

Higher independent NEW651,546,624/outside484,610,048 bytes is retained over the
primary NEW569,344,000/outside402,407,424 sample. WHOLE floor59,231,763,872 and
historical target growth166,936,576 remain nonshrinking. Primary max11 observed
PIDs, RSS429,998,080, minimum free22,356,602,880 bytes. Physical PostgreSQL phase
footprints, marker/inode custody, controller/kernel executable identities and
ledger comparisons remain in the private raw proof. No deletion credit is taken.

## Closed failures and fixes

All failed receipts remain unchanged and are not upgraded to passes.

| Round | Actual outcome and release | Fix / immutable proof |
| --- | --- | --- |
| Build01 `BE10-BE11-FINISH-BUILD01-0252` | Accepted1793848651.046644; capacity refresh refused four other workflows but outer caller used a stale snapshot. Owner stopped before census marker, Cargo/copy/tests/native NOT_RUN, exit1 `RuntimeError: guard signal`. TARGET1793848665.392401; FULL1793848665.4361951, resources0. | Fail-closed checked refresh, unique nonce/output, exact offer/source binding and fresh second admission implemented and actually qualified by Build02. `build-controls/TARGET.json` SHA `34af8826a7775147c8258de4ca1360f7c158b50add3bac8dbb9293332f74d34f`; `build-controls/FULL.json` SHA `b2bf9918377a6e18048d4dc4c8fc0683d4017b03aa4a2aad63b4f199165ad7e2`. |
| Native01 `BE10-BE11-COPIED-NATIVE01-0356` | Accepted1793851153.1226082; pg_ctl start failed, zero cases. Original raw1793851181.314311 was CLEANUP_REQUIRED, not FULL: it listed an already exited observer. Actual kernel-identified cleanup verified every owned process/group/root absent; FULL1793851252.813879 within original deadline. | Shortened the 117-byte socket path; original startup log was deleted during cleanup, so path length is a probable cause, not a confirmed log diagnosis. `native01-preparation/FULL.json` SHA `e044062aff64bdac2be80d1bf827d471547c30d946223136206ba8534732b73b`; separate `FULL-CLEANUP.json` SHA `6929d55bc337aa5b54ea14dd2bf5d4c4126348e2fb7215c0045e31d712882738`. Entire fresh128MiB conservatively charged once for missed fast PG materialization. |
| Native02 `BE10-BE11-COPIED-NATIVE02-0405` | Accepted1793851702.635516; actual PG16.14 start succeeded, service --migrate exit1: `CONCERT_DATABASE_URL requires a dedicated mcr_concert database`. Zero cases; cleanupPASS; actual FULL1793851728.7540858, all resources0. | Helper-only committed correction `02c9922912e02bf30037fbc5f3b8057c5c3403d6` migrates/verifies mcr_concert then clones mcr_world. Native03 qualifies this actual fix. `n2/FULL.json` SHA `5a2ba375ea8b0649ad4450307d533a6a4442099f9eab2a254a069ed0e73229a3`. |

Paths in this table are relative to the same private parent above. No old round is
reopened. The ordinary preparation booking remained one128MiB total, with retained
charge98,681,299 and remaining35,536,429 bytes, time-only extended through
2026-11-05 04:30UTC. It does not authorize execution. Postqualification authoring
carries native03's higher maximum plus a bounded1MiB doc/Git/handoff upper:
NEW652,595,200/outside485,658,624/WHOLE59,231,763,872. The original cumulative
family2GiB and outside0.5GiB caps remain unchanged. All execution offers are FULL.

## Delivery disposition

Exactly six changed paths versus immutable base `2492e35bb9fb61380bf97fa936983ec16c0722f4`:

1. `backend/crates/concert-application/src/world_ticket/mod.rs`
2. `backend/crates/concert-application/tests/world_ticket.rs`
3. `backend/services/concert-service/src/world/mod.rs` (only granted repository queries)
4. `backend/services/concert-service/tests/world_admission.rs`
5. `backend/services/concert-service/tests/support/world_admission_leases/round.py`
6. `docs/mcr/backend/world-admission-leases-completion.md`

No manifest, lockfile, shared contract/proto, migration, RPC, runtime/recovery,
frontend or frozen source changes. The qualified committed scope goes directly
to standing publisher412e, who owns combined integration checks and remote
publication; no King/SL review or acknowledgment is required. Sent handoff is not
proof of publication. BE10 is qualified pending publication; BE11 membership is
qualified pending publication, with the following whole-product exception open.

## W04 invitation remains an open product obligation

The specification's W04 `/world/rooms` entry explicitly includes capacity, room
switch and party invitation, mapped to FE18/BE11. Original BE11 row 466 describes
owner, capacity, session membership, atomic join/switch/leave and disconnect grace;
it does not define an invitation authority lifecycle. The current source search
found no pinned virtual-party invitation producer or API. This assignment's six
paths exclude World RPC, the admission bridge, shared contracts/proto and migrations;
no invitation create/accept/expiry/revoke producer has been transferred here.

W04 invitation therefore remains **OPEN**, with exact producer/contract/write-seam
ownership to be assigned explicitly. It is not satisfied by physical place or
meeting-link behavior. Qualification of this assignment's membership criteria must
not silently close W04 or remove its invitation requirement. This exception was
reported directly to World20f and Root1fc; no manager review gate or additional
whole-matrix prerequisite is introduced.
