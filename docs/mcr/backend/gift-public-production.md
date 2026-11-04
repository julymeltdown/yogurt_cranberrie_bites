# Gift public production flow

Task: `GIFT-PUBLIC-PRODUCTION-01`, including `BE42-PD-GIFT`.

Status: **IN_PROGRESS**. This source checkpoint is **NOT_RUN**; it is not
public-flow, media-delivery, account-deletion, physical-place or IN02 acceptance.

The implementation branch is `codex/mcr-gift-public-production-01` in the retained
Gift worktree. Its base is `5d20b9ef371b98a8abfd81b9b12f8b6cb83ac6e2`, the published
Gift component. That component's validation remains recorded in `gift-live.md`;
it does not establish this public integration's runtime behavior.

## Source changes

- `gift_live/protocol.rs` owns one closed Gift13 route/query parser. Native
  Gift settlement selection reuses it rather than maintaining a divergent list.
- `product_rpc/{mod.rs,query.rs,owner_media.rs}` adds the typed Gift operation,
  validates its exact body/key under the verified original deadline before owner
  acquisition, and handles the necessary exhaustive refusal arm. HTTP 410 is
  admitted only for Gift operations.
- Gateway `adapters/in/http/concert_product/{feature_routes.rs,mod.rs}` mounts
  Gift13, validates closed bodies and queries, keeps cancellation bodyless and
  keyed, and admits Gift-only 410. The OUT adapter remains unchanged.
- `config/native_resolver.rs` selects the actual Gift native resolver.
- `config/gift_composition.rs` loads actual guarded bootstrap/participant/Intro
  settings. At the original source freeze it owned the bounded sanitizer; the
  separate shared Media correction below moves that construction. Its opaque `GiftFeatureReadiness`
  exposes `is_enabled()`; the bootstrap owner owns the separate additive join
  into its single final release binding. `main.rs` loads these settings and
  composes Gift over the existing canonical owner composition after Reports.

## Current coherent source checkpoint

The actual Media owner dependency `c29db6b5b` is merged at `ad3c79f4f`.
Gift image reads now call its sorted current-user pair lock helper before event,
target or child SQL. This takes both actor and actual media owner, then rechecks
the locked media purpose/owner/target. Linked reads retain the actual Gift root,
READY media and current relationship exclusions. Unlinked Giveaway images remain
uploader-private and are clipped to their actual orphan expiry. The distinct
`GiftMedia` kind uses migration 0073, durable child intent ACK before bytes,
actual prepared transaction identity and strict native abort recovery. It never
impersonates `LocalPublic`, avatar or `GiftPrivate` authority.

The native media mutation resolver wraps the Media owner's existing settlement
validator/resolver, authenticates the exact stored registration and requires an
exact Gift transaction witness plus root completion for committed evidence.
An insert conflict is not an assumed witness match. Missing transactions,
elapsed deadlines and expired media are not evidence of qualified abortion.

ABI corrective dependency `cfa8793e1` was taken unchanged as `ecbee45f1`.
Published canonical `1be2c6873` and Social-owned typed readiness join
`43f9efab9` were consumed without conflicts; the join is `211de0759`.
Social still owns its original selection and single final release binding.
Gift shares one core, pool, instance, sanitizer and private keyring composition.

Validated Gift startup also constructs one supervised expiry worker. Every
second it invokes the retained database-clock sweep for at most 16 roots,
skips missed ticks, bounds a round to two seconds, and honors shared shutdown.
It supplies `UnavailableAuthority` because system expiry never impersonates an
actor or exercises a privileged read/write grant. Expiry truth remains durable.

The Gift account-deletion implementation and cleanup-only selective dependency
are described in `gift-account-deletion.md`. Party owns the sole combined category;
Media owns byte erasure. Neither category is certified by Gift alone.

## Authored validation and open gates

Run relevant fresh developer-owned build, store, restart, receipt/replay,
stock-race and actual three-user Gateway flows under one allocated finite round.
New tests in `tests/gift_public_admission.rs`, Gateway's Gift admission module,
`tests/gift_deletion_native.rs`, `tests/gift_publicmedia_native.rs`, and
`gift_live/native_deletion.rs` are authored but **NOT_RUN**. They include real
PG restart/partial progress and native identity refusal, actual typed query
exclusion, recreation refusal, and claim/cancel/expiry/deletion collisions.
The store fixtures explicitly use synthetic policy/source and cannot certify
production Auth or reviewed physical facts.

`scripts/mcr/gift_public_native.py` accepts exact hashes for the library and two
integration executables and creates only a fresh marker/inode-owned PG cluster
with 256 prepared slots. Its cleanup uses the actual harness `cleanup()` API and
records a cleanup failure as FAIL. Source and binary drift fail closed.

`scripts/mcr/gift_public_stack.py` is a derivative of the published IN03 launcher
at `1be2c6873`, original blob `2430884cf1f6956420300e278b7082991b766e4b`.
It preserves that launcher's binary/source pinning, seven actual services, Auth
mail/signup/verify/login, TLS, bounded guard and owned cleanup. Gift additions are
the validated live settings/independent ephemeral keys and the in-memory
`gift_public_http.py` consumer. It exercises three canonical users, Gift13 route
admission/missing-resource and unreviewed-place denials, actual sanitized
uploader-private READY media, idempotence/conflict, private receipts and durable
intent completion after actual HTTP EOF. No bearer or full C03 is persisted.
The original launcher is unchanged. The derivative does not qualify the original
IN03 journey, linked audience or positive physical stock/pickup flow.

Only formatting, `git diff --check` and Python AST parsing have run for this
checkpoint. Existing Rust 1.88.0 formatted new files. No compile, database,
service, native reservation or target has started. Before-first-byte concurrency,
production deletion Auth transport, linked media/restart and positive Gift13
stock/pickup flows remain required and unqualified.

Physical-place provenance remains **WAITING_INPUT** where an actual reviewed
place is required. It does not block independent server or internal deletion
work. No invented place, media binding, actor authority or synthetic fixture
will be used as public-flow acceptance.

## Ownership and delivery

Root explicitly transferred the listed Gift-only shared hunks; Report/Ops,
Party, Wall and the bootstrap selection/binding hunks remain with their owners.
The current publishing developer is Delivery 412e. Root retains shared composition.
No king/SL review or acknowledgement is a
delivery gate. The historical BE41 goal remains BLOCKED and incompatible; this
assignment does not reset or falsely complete it.

## Retained40 control failure and repair, 2026-11-04 UTC

Frozen application source `01ff9a1a91114101c4edc580d7d9c0f87f09e6fa`
was accepted under `GIFT-IN02-RETAINED-WHOLE-MEDIA-20261104T2140Z` at
Unix `1793834414.954557`. All 2,097 selected build inputs and six helpers
were byte-verified before acceptance. The private retained40 control had SHA256
`5765376cafe001b08da4561032078e194fb28aaaa8aa862a8d98e207806ec854`.
It preserved the 720-second compile, 780-second total work, 1,020-second full
release and 240-second cleanup bounds, with both watchdogs live before Cargo.

Admission counted the **full** shared target allocation of 19,550,818,304 bytes
and outside retained bucket maxima of 3,705,727,152 bytes. Carry was therefore
23,256,545,456 bytes; adding the entire 3 GiB new allowance fit the fresh 40 GiB
whole limit. Actual free space was 17,623,195,648 bytes; reserving 3 GiB for
others and 3 GiB for Gift left 11,180,744,704 bytes, above the 9 GiB floor.
Earlier growth-only whole accounting remains unqualified historical evidence.

The first Cargo command was interrupted when the independent guardian's
`du -sk` measurement returned exit 1. Its stderr was not retained, so concurrent
Cargo file removal is a possible cause, **not a confirmed diagnosis**. The
controller then hit `PermissionError` during process-group cleanup and did not
write its normal terminal report. Controller exit was 1; the Cargo exit is
unknown. Fresh process and kernel-handle observations found the controller,
guardian, Cargo, rustc and rustdoc absent and zero target handles.

Target release was `1793834479.5983932`, SHA256
`742f6c392dfb0f02dc11943f435c28884a311915c656cb1960896f6960ba18da`.
Full release was `1793834494.972245`, SHA256
`2c63f8f6e8c1f0bd1236de3053e458f9fe5e5ba9b36fba76426be1bd71ce9f59`.
Both were sent directly to the responsible owners before documentation. The
private immutable receipts and supplemental failure report are in
`/private/tmp/mcr-gift-retained40-build-18m_lu3q/`; the failure report SHA256 is
`25ac9d5f82a9ca80204348edb18da9131a74c9bc40ed1c0cf7b2a4d21be47dc3`.
This round is **FAILED_UNQUALIFIED**. There were no subsequent Cargo commands,
copies, pure tests, native/PG tests, services or HTTP flows. No target reentry
or reuse of this closed allocation is authorized.

The repair replaces shell `du` sampling with allocated-inode traversal through
directory file descriptors, without following symlinks. Only an enumerated
descendant disappearing with ENOENT is tolerated. Missing/replaced roots,
permission errors, other I/O failures and a five-second sample timeout fail
closed. Both watchdogs retain nonshrinking peaks. The runner now requires the
full target plus source roots in carry and admits the offered upper bound of
40 GiB; it still grants no lease. Compilation ends at the original acceptance
plus 720 seconds and copying retains the original work deadline.

Cleanup errors remain explicit failures while direct-child reaping, guardian
shutdown and terminal reporting continue. Failed guardian receipts retain
their original diagnostic fields. Failed terminal accounting is explicitly
unqualified rather than silently substituted as a passing sample.

All ten tiny filesystem/process regression fixtures in
`backend/scripts/mcr/test_gift_public_controls.py` **passed**, exit 0, under the
separate `GIFT-CONTROLS-PYTHON-FIXTURES-20261104T2327Z` allocation. It accepted
clean source `55981275200f83dd42d6fc8c7d82e7b296b6cfaf` at
`1793835093.895883`; full release was `1793835114.951116`, within its original
120-second work and 180-second full bounds. Both live guards passed; measured
peaks were 5 processes, 189,382,656 RSS bytes, 1,200,128 new bytes and
28,182,554,800 whole bytes including saved target, outside history, full Python
and Rust tools, and the conservative prior 2 GiB peer target-growth maximum.
No target was inspected during this round. AST/whitespace and seven serial
Rust syntax parses of the separately committed Evidence SDK dependency packet
also passed; none constitutes type checking or native execution.

Private report `/private/tmp/mcr-gift-controls2327-2tbu6mvz/report.json` has SHA256
`b9b69f7d0aad25bcbd3bc2db2d3866bfee0b1665102cdbe6d121e9c1a9b42335`.
Its `FULL.json` has SHA256
`040a78263570ded94dc1a79fd6641da13ed81189ad4a9e604e3c616596ca3455`.
The literal release went directly to the allocator and next developer before
documentation. There were no Cargo, PostgreSQL, service, browser or product
runtime workloads. A fresh Gift build and all original product gates remain open.

The assigned `REPORT-EVIDENCE-CHILD-01` and `POST-MEDIA-CHILD-01` followups begin
after this Gift freeze, in separate commits with their own distinct child kinds,
exact owner ABIs and relevant validation/fixes/handoffs. Their reserved migrations
are 0080 and 0081. They do not authorize edits to Report/Feed owning leaves,
change this frozen Gift round, or complete unfinished Gift work.

## Shared Media and lease-bound runner followups

Source followup `44d966c59` moves the bounded sanitizer and Media fingerprint
keyring into a single shared startup composition independent of Gift mode. Gift
business receipt, cursor and pickup keys remain separate. Actual Gift upload and
link adapters receive the common Media-key Arc and sanitizer; Feed receives the
same handles through its agreed explicit owner port. See
`shared-media-dependencies.md` for source custody, exact API and open join gates.

Both Gift runners now take explicit accepted start/continuous free-space floor
arguments through `gift_public_guard.py`. The adapter preserves the existing
qualification helper's inode/private-output, cumulative byte, process, original
work/phase deadline and 240-second cleanup accounting. It additionally enforces
at least 12 GiB at start, 8 GiB continuously, and 9 GiB remaining under the full
new-byte allowance. These are lower bounds, not an allocation: supplied floors
must equal the fresh accepted offer, which may require higher values.

The native runner now also requires allocation identity and finite absolute
work/release deadlines, checks every migration/test phase, and catches direct
producer SIGTERM/SIGINT so its actual child/owned cluster cleanup runs. Closing
the watchdog before cleanup preserves its existing bounded shutdown reserve.
No peer helper is edited and no resource was entered by authoring these changes.
Syntax/AST/diff checks are source evidence only; guard behavior and all actual
build/native/HTTP commands remain **NOT_RUN** pending a fresh allocation.

## Next finite developer validation round (not allocated)

This plan is **NOT READY** and grants no resource entry. An immutable source pin
must first include Feed's actual four-argument composition/POST GET handoff and
the Media owner's unchanged READ COMMITTED fence correction `01d06d623`. The
Report evidence constructor is still absent; its enum/SQL must not be mistaken
for activated Report byte delivery. No whole-wave barrier is imposed on Gift
validation, and missing physical-place facts remain a separate open gate.

The proposed finite work caps are 720 seconds for build/copy, 60 for focused
pure cases, 180 for owning-store/native restart checks, and 420 for actual
seven-service HTTP startup/provision/Gift requests: 1380 seconds of work plus
240 seconds reserved for cleanup. The accepted allocator offer may grant a
smaller usable round; expired or insufficient bounds require release/requeue.
It cannot be extended by this document or a successful intermediate check.

Only the retained `/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`
may be entered under a fresh target lease. Use existing Rust 1.88.0, offline and
locked resolution, two jobs, dev/test debug info zero and incremental off, with
no RUSTFLAGS or wrapper overrides. Do not rebuild the five unchanged foundation
services or add a target/check-out/archive. Relevant new builds are:

- Concert library test executable and integration targets
  `gift_public_admission`, `gift_deletion_native`, `gift_publicmedia_native`,
  `bootstrap_features`, `report_evidence_child_contract`,
  `post_media_child_contract`, and `shared_media_dependencies`.
- Gateway library tests limited to the actual Gift admission module, plus the
  actual Concert and Gateway main executables for HTTP composition.

Each output must carry its actual compiled-source revision and SHA256. Keep
the three required native executables (Concert library, deletion and Media)
and two actual mains in one owned private output root. Process the other pure
executables sequentially and remove only those exact newly copied temporary
files after retaining their hashes/results; enforce the accepted aggregate copy
cap (at most 1 GiB) throughout, rather than assuming all eleven outputs fit at
once. Root's five foundation records at source
`18076c951a7283354c869caaf64d40491ca52d75` remain unchanged read-only inputs.
Their actual binary and provenance hashes were checked; no runtime was inferred
from those checks. The Gateway legacy signup/old Auth mixed-source boundary is
explicitly versioned and does not claim the later enrollment contract.

A concrete READY request must include the then-current source pin, exact command
list, actual target lease, every cumulative retained path/byte charge, fresh
free-space reading, new-byte and process caps, per-phase and absolute work/release
deadlines, cleanup ownership, and the five foundation source/hash/provenance
records. Native and HTTP enter only their accepted slots after target release;
no implicit overlap or unallocated workload is permitted. SQLx migration
receipts, actual before-byte/EOF races, producer recovery, Auth deletion transport
and physical stock/pickup results must keep their separate truthful dispositions.

## Selective Feed compile custody

Feed owner checkpoint `fbbae1771fc1476e9b7455b54a939b2fdc881676` is explicitly
SOURCE_ONLY, uncompiled and not source-READY. Minimal dependency packet
`f179f7c853eaf95c9f267e0ee6d7e7fb7bef9024` was prepared on Gift source
`a482e53d36e46b4d5ff0a6280ae6b5feefec4b99`, without changing this checkout.
Its 23-path readset contains the platform Feed authority module/export and exact
method policy additions, additive Feed proto, Concert 0069, changed `feed/**`,
Feed composition and four Product RPC files. Twenty blobs are byte-identical to
Feed's source. `product_rpc/{mod,query,owner_media}.rs` are mechanical unions:
retain both Gift and Feed closed operation/parser/refusal arms and both request
helpers. Source syntax and diff whitespace checks passed.

Both service mains, Gateway, all Post-service leaves, Feed native resolver and
Feed tests/docs are excluded. The publisher routes named selective intake and
central main/callback union; this packet does not take peer implementation
ownership or establish Feed runtime correctness. All build/SQL/native/HTTP
checks remain **NOT_RUN**. The pending generic local-read recovery correction
must also be taken from its actual owner before Post restart qualification.

The Gift launcher now explicitly sets and validates
`CONCERT_FEED_COMMAND_MODE=off` alongside Feed read mode. Its retained Post
foundation at source `18076c951a7283354c869caaf64d40491ca52d75` cannot implement
the new six-RPC/0034 Post participant contract. Gift's bounded flow does not
exercise or qualify positive POST Media, Feed mutations, Feed projections or
new attachment linkage. Those require the actual owner's new binary and native
witness/recovery evidence; an ambient feature override must not silently change
this versioned consumer boundary. This does not close the wider product scope.

## Applied dependency custody and local recovery correction

Publisher-authorized Feed packet `f179f7c8` is applied as `4be40bb09`: exact
23 paths, twenty unchanged Feed source blobs and three mechanical Gift/Feed
Product RPC unions. The owning Feed implementation reports no additional source
compile dependency in this readset. This remains static interface evidence;
compile, SQL, native and HTTP checks are **NOT_RUN**. Feed runtime mutation
resolver and central callback wiring remain excluded and unqualified.

The authorized Media `01d06d623` intake was empty because all four actual
postimages already exactly matched that source. The empty cherry-pick was
skipped, preserving existing files; no new Media runtime result follows. The
earlier assumption that this correction was absent was incorrect. Exact SHA256:

- `migrations/0067_media_account_deletion.sql`: `238bcfd3b0ef782a4c344c62a750580c497b80356cb1429379b4e7419f49ca2f`.
- `src/adapters/out/media/account_private_data.rs`: `1d7a95067d10681fd1552627f1467a67a61d6cc079942e70f4bd8830e7e0fbfb`.
- `tests/media_deletion.rs`: `7abaae06e40db3db0ed41d3d34f986e3f14a390376d29a498d7e5986be0f5549`.
- `docs/mcr/backend/media-account-deletion.md`: `10b2307f28297afb35afc673ee71a948368ed2cb25b9a816ea206f5d827605b2`.

The first three paths are relative to `backend/services/concert-service/`.

Root transferred exactly `public_profile/local.rs::recover_one` and one focused
append in existing `public_profile/native.rs`. Recovery now queries the saved
full-XID state after cluster/database/GID/low32 matching and requires actual
`in progress` before attempting ROLLBACK PREPARED. The existing actual full-XID
`aborted` postcondition remains unchanged. No target, role, scope or child kind
is widened.

The new ignored native regression creates an actual prepared Concert transaction,
keeps its GID and low32, changes the saved full XID by +2^32, and requires refusal
without removing that actual transaction. It then uses the actual reference to
verify abortion and replay. Synthetic correlation records grant no canonical
authority. `gift_public_native.py` runs this case with the existing owned helper
fixture against one additional fresh `mcr_canonical_concert` database in its
same marker/inode-owned PG cluster. That helper uses the real SQLx Migrator and
receipts; no extra service, target or PG process is started by this case.

Rust syntax and Python AST checks pass. The regression has not been built or
executed; helper/native restart/Post media acceptance remain **NOT_RUN**.

## Runtime migration package dependency

Root explicitly transferred selective source from `9a3c008b6` and `34ca081d7`
(published launcher source `521a8075`). This checkout takes the literal
`migration_directory`/`migration_directory_from` API and its serving-pool use,
only the standalone migration command use in main, and the exact cancellation
fixture compatibility update. It preserves existing Gift/Feed/shared-key/core
composition. No other main/runtime ownership is acquired. The owning API's
unrelated documentation and appended config test module are not imported.

The config API literal SHA256 is
`739b24bb7e49328bfaaa3bb1704c61868daf8dab621fbff05bd5a2cc74cbc906`.
The exact `seed_concert` optional `runtime_migration_directory` function postimage
from `world_in01_prepare.py` at `34ca081d7` is retained (SHA256
`74ccf4621ff9948380881b8259b53647df38f0aa3d0e442690a17f6a40f944ad`).
The helper's other functions remain unchanged.

Gift's derivative copies `verify_migration_package` and
`package_concert_migrations` literally from that launcher source, combined SHA256
`e763a7ee36a1aa73af3b3c2929e7cc4dd713a2560d128dddc013c17962c08589`.
It packages only the actual Concert binary source's SQL in the existing owned
cluster, mode 0700 directory/0600 files, with exact SQLx SHA384 inventory. The
same explicit directory is passed through migrate, verify, catalog seed and
serving. Initial checks, source/binary rechecks and the hold loop reverify that
package; the retained executable's build-time checkout is not used as runtime
SQL truth. No second source checkout, archive or target is created.

The publisher reported another owner's retained `cfc49f4ee` main qualification
with the override. That is peer evidence for its own source, not this composed
main's build, launch or migration result. Gift's config/main/launcher union still
requires its own actual compile, SQLx/serving checks and runtime evidence. Only
source literal comparison, Rust syntax, Python AST and diff checks have passed
here; all actual build/native/HTTP operations remain **NOT_RUN**.


## Finite source-build executor

`backend/scripts/mcr/gift_public_build.py` is an unexecuted, source-only executor
for a fresh accepted allocation. It requires a clean exact source revision,
explicit allocation identity and work/release deadlines, a 240-second cleanup
reserve, accepted free-space/process/byte bounds, and an existing private output
directory. Its only target is the retained
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`; it grants no lease.
No target inspection, Cargo command or executable launch has been performed by
this source change.

Under an accepted lease it runs three offline/locked Cargo commands from the
actual `backend/` workspace with two jobs, debug information and incremental
compilation disabled. It builds the Concert library plus the seven named Gift,
bootstrap, Report/Post child-contract and shared-Media integration binaries,
the Gateway library, and the two actual service mains. It does not rebuild the
five versioned Auth/Friend/Profile/Post/Notification foundation inputs.

The required `build` stage copies all eleven executables before target release:
the Concert library, deletion/media native tests, both mains and six other
pure-test executables. The separate `pure` stage requires SHA-pinned successful
build and actual zero-process/handle target-release receipts. It runs eight
selections from these copies, requires nonzero passing cases and removes only
the six exact temporary copies. The pure branch does not stat, resolve, measure
or enumerate the target. Actual target release goes directly to the allocator
and publisher before pure work, cleanup or documentation; it needs no ACK.

Both stages retain the same accepted start epoch, absolute work/release
deadlines and byte/process bounds. Work ends within 780 seconds and full release
within 1020 seconds, preserving 240 seconds for cleanup. The build phase is at
most 720 seconds. Cumulative copy and peak source/target/output counters never
decrease across phases or when a copy is removed. The inclusive new-growth cap
is 3 GiB: target growth at most 2 GiB, and all outside growth at most 1 GiB.
Source, shared Git, private controls, proof and logs are inside that outside cap;
source/Git/control growth is additionally limited to 128 MiB. Retained carry is
at least 3,669,547,184 bytes or the higher fresh/prior maximum, with whole usage
at most 7 GiB. Process metadata sampling enforces at most 32 owned processes
and 8 GiB RSS. Cargo explicitly retains the contracts-build optimization level
of two. Actual target process/handle inspection remains the developer's duty;
the build report leaves that handle check **NOT_RUN**. A conditional offer,
script or receipt template does not establish allocation or release.

Before a workload child starts, both the producer watchdog and a separate
supervising guardian process enforce the accepted bounds. Both count toward the
32-process/8-GiB limits. A SHA-pinned private capacity snapshot records actual
named outstanding/eligible growth claims, workflow counts and the literal
predecessor target-release receipt. At build admission its observation must be
at most 60 seconds old: fresh free space is at least 12 GiB and remains at least
9 GiB after the full inclusive 3 GiB allowance and all other growth reservations.
The separate guardian performs its own fresh disk/process/byte checks before
writing its ready receipt. Both controls retain the 8 GiB live floor. Pure is a
continuation of the same original allocation; it preserves the original pinned
capacity record and deadlines, rather than restarting the admission clock or
adding a fresh allowance. Actual current B/C/D claims are obtained once at the
predecessor handoff; the allocator's queue metadata is not a running broker.
The independent guardian remains active while owned children terminate during
cleanup, using the original full deadline. Final accounting includes partial
failed copies and cleanup writes, and reserves 64 KiB for final receipt metadata.

Build provenance for the two new mains is labelled `GIFT_PUBLIC_SOURCE_BUILD`.
The derivative launcher's admission uses that label for those mains and retains
`WORLD_AUTH_FOUNDATION`/`IN03_ADVANCED_COMBINED` only for the unchanged versioned
foundation cohort. A compiled source receipt does not certify original IN03,
canonical multiuser behavior, SQL, native recovery, restart, HTTP EOF, physical
pickup or the wider Feed/Report product. Python AST and diff whitespace checks
are the only checks performed for this executor; all build, pure executable,
native and HTTP checks remain **NOT_RUN** pending a fresh accepted allocation.


## First bounded build and repair (2026-11-04 20:50 UTC)

The actual Gift2000 allocation started at1793825409.129157 on clean
c14c8c1bad676715ad6b89e4d1c78f6375f6de92. Its first Cargo command exited101;
no second/third build ran, no executables were copied, and pure/native/PG/HTTP
checks are NOT_RUN. The target was actually released at1793825487.435307,
FULL at1793825504.443176, with compiler/owned children/guardian/target handles
zero and no reentry. The failed build is preserved separately from qualification.

Private original evidence: `/private/tmp/mcr-gift-public-build-7rl5_680/`.
Build report SHA256 `0f44fad53b05fca78a202e035dfa37ccc23b17cf5c61d00f9ed6e23977c6836f`;
target-release SHA256 `2197dfa89447a160c8cba2b1d4214001e6e2ac9fdd5553558112b82847310851`;
FULL SHA256 `7c8e27532c5907e48e029786f4f7366e13febecdabd27542831e4a4ff3bd539d`.
Peak target growth3,592,192 bytes, inclusive outside708,608, whole3,689,236,656,
owned processes7 and sampled RSS2,339,438,592. Independent guardian PASS means
its resource checks passed; the build result is FAIL. The target-release receipt
has a null optional RSS field; the actual RSS remains in the original build
report and is stated here without altering a sent receipt or its SHA.

Two Gift-owned test-helper conversions incorrectly used `?` with DeletionFailure,
which does not implement std::error::Error. Both now map failure into a static
anyhow context. The helper still requires actual source verification and common
fence establishment; no success behavior or deletion authority is weakened.

The compiler also reported missing platform Feed command and application/domain
Mural Tools B exports despite their presence in the exact frozen checkout.
Stale shared warm artifacts are a suspected cause, not a verified conclusion.
For the next accepted build the runner verifies selected tracked backend build
inputs against their frozen Git blob, refreshes only their mtimes in its own
checkout, then verifies identical SHA256 bytes and retains a private manifest.
Both resource controls are active before that work and before Cargo. It neither
cleans nor changes the shared target and does not edit source or contract bytes.
This follows the allocator's existing byte-identical freshness policy; another
build still needs a fresh finite allocation, current capacity and source pins.
The repaired helper and new freshness path have syntax/whitespace evidence only.
Compilation and the remaining original product validation are still required.
