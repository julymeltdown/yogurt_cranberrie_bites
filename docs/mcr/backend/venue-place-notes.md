# VENU-BE-N01 developer assignment and implementation

Status: IN_PROGRESS. Developer213 accepted the explicit N01 priority transfer in
the existing exclusive `be-report-completion-03de` worktree, branch
`codex/mcr-be-BE36-report-completion-03de`, starting at
`ba99630a2d256c9b493348b3e5dfadae460cb636`. BE36 remains wholly unfinished behind
this priority; the frozen Report candidate and stopped Media checkout/evidence
remain unchanged. The persistent goal API was queried and still reports the
historical incompatible BE24 upload objective BLOCKED. No reset, false
completion, duplicate goal or pause was performed.

## Concrete objective and completion criteria

Deliver mandatory additional amendment VENU-BE-N01: real place-anchored notes,
replies, currently authorized marker/count/list/detail, Friends/Everyone,
server-clock1h/reviewed-venue-endday expiry, current authority/withdrawal,
durable idempotency and safe unknown-outcome reconciliation, privacy purge,
actual Meet Here handoff, actual retained-native read EOF/cancel/recovery.
Complete relevant developer-owned source/schema, SQL/native, public, restart,
privacy/negative/fault validation and fixes; record exact provenance and actual
limitations; commit scoped handoffs directly to publisher412e and coordinate
contracts directly with Root153/40fde. This is added mandatory scope, not a
replacement for or completion of an original task.

Continuity mappings: original BE08/W03 provides place concepts; BE15 command
receipts/outbox; BE37 privacy/block/relationship; BE41/BE42 event/account
cleanup. Original BE31/32 Feed does not become a Notes producer. Keep all112
original IDs and61 IA entries unchanged, and the eight VENU deliverables/ten
extension IA states separately accounted. No king/SL code/query/test review or
acknowledgment gate applies.

## Authority and provenance

Canonical amendment `686752254c805bbe8ec7a38d0be276513205dc06` (owner source
`e8f576d7702483f6f1bbfa14e0f5134c4c5051cb`) and corrected reference packet
`6ff3e481cfcf820c2e14a9a670ae2dc3bffc9afb` were delivered in master
`1907016b6aa41c17998acab691a83e99d0a80076`. The originals were inspected via
immutable Git blobs and original Photo10; private management ancestry was not
merged/rebased/materialized in this checkout. Notes Photo10 SHA256 is
`1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63`.
Actors, art anchors, times, positions and text are examples, not authority.

New domain/application/service code is authored here. Existing headless BE15
`concert_application::commands` and
`concert-service/src/adapters/out/repository/receipt.rs` are called through their
actual APIs; no old rendering or Feed implementation is copied. C03 current
PublicProfile and its exact transitive schemas are copied into the N01 OpenAPI
with source SHA and names in `x-mcr-public-profile-provenance`; actual source is
the unchanged `contracts/generated/mcr-domain.openapi.json` at this base. The
production PublicProfileReader validates the same source and supplies only real
current approved avatar/profile data. There are no fabricated author fallbacks.

## Source and central ownership

Owned new source: `concert-domain/src/place_notes/mod.rs`, its `tests/place_notes.rs`;
`concert-application/src/place_notes/mod.rs`, its `tests/place_notes.rs`;
`concert-service/src/place_notes/{mod,authority,repository,producer,wire}.rs`,
`tests/place_notes_native.rs`, `tests/place_notes_support/mod.rs`; this document;
the exact new N01 proto/OpenAPI/contract MD. Root separately delegated additive
Notes14 SDK and Location15 SDK hunks in the six explicitly named shared files;
SDK amendments and exact three module-export hunks were accepted by21d within the same ordinary booking. Root retains
config/native_resolver/canonical/main/operation routing, manifests/lockfiles,
build/codegen/migration-loader and Gateway. New SQL0103/0104/0105 are separately
assigned; all original SQL stays immutable. No Location producer or L0101
schema ownership is implied by the shared Location15 variant.

Initial additive Root export proposal (the three exact lines are now applied
under the subsequent explicit Root ownership transfer):

```rust
// concert-domain/src/lib.rs; concert-application/src/lib.rs;
// concert-service/src/lib.rs, one new line in each existing root module:
pub mod place_notes;
```

Required BE15 registry entries are the three exact mutation operation IDs and
templates in `place_notes::wire::OPERATIONS`. The existing path validator assumes
UUID parameters; an opaque L-issued landmark key needs an exact `landmarkId`
validation exception using the validated LandmarkId type. Never relax other
parameters or invent a Place UUID. Root also registers the closed six HTTP
operations in actual Product admission and mounts this scoped producer; generic
C04 command status dispatches only the three N01 mutation IDs to this owner.
No registration, export proposal or source trait proves a mounted route.

## Behavior and required actual dependencies

Domain and application prepare immutable notes/replies using database time and
the actual L grant tuple. Current L dependencies distinguish provider/source,
event/map/catalog and individual landmark revision; actual endday uses venue
policy/timezone. Current Friend dependency promises both directions of blocks,
current actor/subject Auth and event eligibility plus mutual friendship; missing
providers return503. Everyone is current eligible authenticated same-event
participants. Replies inherit audience/absolute expiry and use the intersection
of parent and reply author current permissions. One-level replies cannot widen,
outlive, cross event/map/provider or reinterpret a changed physical anchor.

The local SQL schema is the exact assigned0103 migration, included by `repository::SCHEMA_SQL`; it is not run at startup. It enforces identity/scope/time/revision/reply invariants. Commands call
actual BE15 keyed fingerprints, stable original-key resource identity, receipt
and safe-reference outbox atomically. Raw body/profile content is excluded from
receipt, outbox, diagnostic and error payloads. Owner withdrawal clears the
entire root thread atomically; expiry reads withdraw immediately, with bounded
resumable persisted-body purge and account-deletion hooks. Actual Root cleanup
and moderation composition are still required. A state flag or raw SQL fixture
does not prove current moderator authorization.

Protected reads must transfer the actual registered transaction to a real
durable typed prepared-native retention owner. Ordinary commit would release
the note-row locks before HTTP EOF and is not accepted. Until that actual
adapter is configured, reads refuse503 before disclosing protected content.
No old Evidence/Social/Profile/TargetRead variant is retagged as Notes. Actual
native identity, binding, ordered note/parent readset, original expiry,
predispatch intent/BOTH owner ACK+witness, EOF rollback and restart recovery are
required. Same actual runtime Arc goes to producer and optional Composite
settler. The injected interface alone is not native qualification.

Meet Here requires a current actual M composer-entry capability and performs no
invitation write/acceptance or physical arrival. Absent M leaves the capability
unavailable. Contextual MESSAGE remains the M owner's explicit new scope, with
optional invitationId and no implicit send on opening.

## Resource and qualification record

Ordinary `VENU-BE-N01-PREP128-0122` adds128MiB TOTAL through2026-11-05T02:10Z,
including all new source/Git/private controls/evidence. Before the first mutation,
free space was23,623,581,696 bytes; the conservative required8GiB+ordinary2.5GiB+
own128MiB+then-current accepted growth was11,408,506,880 bytes. Initial owned
booking is `/private/tmp/mcr-venue-notes-213-0122/booking.json`. Historical carry
is retained at least23,306,629,310 +94,478,526 bytes; no retained target was
accessed and historical bytes were not subtracted from free space twice.
No old booking, source cap or historical FULL was reused as new permission.

The exact three contract files were committed as
`a378329c242f950f36b868ff50c9ef19bf6ed6c4` (parent base above), independently of
untracked implementation leaves. A sparse-checkout staging refusal initially
left the Markdown out of a private provisional commit; exact `git add --sparse`
and private amend restored the intended three-file candidate before any handoff.
No published history was rewritten. Contract qualification N01-CONTRACT-SOURCE01-0142 passed actual protoc (exit0), Draft202012 validation of19schemas/80localrefs, exact7 C03 public-profile closure schemas and30 positive/negative fixtures (exit0), and exact-three-file whitespace check (exit0). Actual ACCEPT1793843079.415671, FULL1793843080.8395882, all owned resources zero. Canonical exact-three contract delivery is8d4561d9291274f867fc921ba8ed4c28790a85a7, delivered in master574b3e5a2a4533d70da3236b2c42c465a32e34a4. The immutable contract Markdown records its earlier authoring-time NOT_RUN status; the later factual packet records this narrow qualification. Syntax/schema checks are not Rust type checking.

Domain/application tests, SQL/native cases, actual current L/Friend/profile/M
providers, actual Notes SDK/native retention, HTTP/Gateway mounted paths,
unknown-status reconciliation, public multiuser/privacy/moderation, physical
restart and expiry/purge scheduling are NOT_RUN. Source/schema-only qualification
will be recorded separately and may be published independently; it cannot
complete N01 or BE36.

## One consolidated missing-policy packet

Current sources establish the supplied Paradise City/name/reference, selected
characters and the requested audience/expiry choices; do not ask for them again.
Actual reviewed landmark identity/current permission/closure validity and real
Friend/policy providers are unconfigured, as confirmed by Root/L. Endday needs
the actual reviewed venue/event timezone and absolute boundary; no arbitrary
operational cutoff is selected here. Privacy proposal is earliest feasible raw
body scrubbing at expiry/withdrawal/moderation/account removal, safe original
receipt metadata only. An actual purge cadence/maximum persisted-body retention
and event close policy remain operator inputs. Root/Main resolve any genuine
missing decision once; these gaps do not stop independent contract/model work
and do not authorize invented physical/provider/retention evidence.


## Current implementation source boundary

Shared additive SDK introduces only Notes14/Location15 without changing1..13.
Both private proofs commit to exact complete native metadata read sets; full
sets are persisted by their respective native owners, keeping Root's8192-byte
reference ceiling. Location has Snapshot/Landmarks/Landmark/CommandStatus, exact
four closed GET operations,51 maximum facts and explicit zero-count absence
semantics; L retains concrete native facts, immutable IDs, physical permissions,
witness and schema0101 ownership. SQL0105 only appends its closed kind.

Notes metadata selection binds the complete actual provider/map/catalog/
individual-landmark tuple and filters current author/parent permissions without
selecting text. Exact original native identity and sorted UUID/revision set are
registered in the durable child intent before protected text selection. The
opaque read ticket is accepted only with that same transaction identity.
SQL0104 checks BOTH owner transaction ACKs, original published Root, exact saved
intent, prepared full-XID, complete ordered read set and separately committed
witness. Exact serialized response hash is in that witness and the held guard,
not an impossible pre-ACK placeholder. The same actual native runtime Arc must
serve producer and Composite recovery. Full-XID ABORTED alone proves native
rollback after actual terminal Root; missing GID, timeout or Drop does not.
These are authored implementation properties, not an executed native proof.

The producer now includes a concrete C04 own-command status method for the
three exact N writes. It reauthorizes the current actor/event and existing own
landmark metadata, returns original safe receipt only after expiry, and never
runs apply, recovers text or substitutes a fresh key. Root operation registration,
opaque landmark path exception, actual status dispatch and Gateway mounts remain
Root-owned and unfinished. Closed provider ports remain required/unconfigured.

Root's exact three `pub mod place_notes;` additions were authored after its
literal STOP/RETURN and21d scope amendment; pre/post hashes and exact hunks are
in private module-export-hunks.json. Other root-module exports are preserved.
New Rust source, domain/application/SDK tests, SQL/native and public integration
remain NOT_RUN until each actual developer-owned finite admission completes.


## Rust source qualification and current remaining validation

N01-RUST-SOURCE01-0204 passed with actual ACCEPT1793844084.112409 and
FULL1793844085.540603, both complete LIVE guard initials/continuous guards,
all owned resources zero. Explicit installed rustfmt1.95 parsed/formatted exactly
ten new owned Rust files (exit0), checked their formatting (exit0), syntax-parsed
six shared SDK and three root-module files with skip_children=true/emitstdout
(exit0; no peer-byte rewrites), and checked scoped whitespace (exit0). Peak5PIDs,
101,842,944 RSS,262,144 newcontrol/log bytes, minimum free23,002,738,688 bytes.
Exact actual frozen pre/postimages and original controls/logs are retained at
`/private/tmp/mcr-venue-notes-213-0122/rust-source01`.

This establishes Rust parse/format status only. Domain/application/SDK tests were
NOT_RUN; no Cargo/type check, SQL execution, real prepared-native/restart/public
journey, actual physical provider, event cleanup/moderation or device acceptance
is implied. Native test/support files and actual required fault fixtures remain
to be completed, not silently waived. The finite source round is FULL and may
never restart. Future type/native checks need their own actual resource offer.


## Native settlement and isolated-case source follow-up

The actual product constructor now takes the concrete native runtime Arc and
derives its pool from that runtime, preventing an independently selected foreign
retention pool. Root installs the same Arc in Composite.with_place_notes.
The new Notes native resolver checks actual full-XID, exact same-transaction
witness, current producer identity, saved parent/binding/registration and terminal
Root. Native COMMITTED is transaction evidence only, never a success-shaped
replacement for the original BE15 command receipt. Prepared text reads remain
abort-only and insert no Root-row witness: retaining such a SHARE/FK lock through
EOF would block Root serialization/sealing. The separately committed read-witness
transaction and ordinary committed Notes transactions have their own0104 native
witnesses. Root still owns actual resolver-union and route composition.

Three explicitly ignored isolated PostgreSQL tests/support sources now cover
immutable scope/text/expiry, one-level inherited reply expiry, stale withdraw
CAS, whole-thread body scrubbing, restore rejection and account-deletion cascade
across authors. They require a dedicated empty database, exact private owner
marker/socket/data-directory/no-TCP checks and a real allocated native lease.
They do not run silently against a default database. Execute serially with
`--ignored --test-threads=1` only when that lease exists. They remain NOT_RUN and
are not a Root/Auth/Friend/L/native EOF/restart proof.

The proposed cold Cargo target was declined by21d under the standing retained-
warm-target policy; it was never created, statted or read. The actual next Cargo
intake is drafted for the retained shared target and remains UNACCEPTED pending
its actual future custody handoff and finite lease. Source preparation has a
separate additional32MiB ordinary book through2026-11-05T02:45Z, preserving all
prior positive maxima; it grants no compiler, database or target access.


## Remaining provider capacity interface

The inherited Root Guard contract exposes MAX_SUBJECTS=100. Notes can have1000
active rows under its independent persistence quota; many distinct current
visible authors therefore cannot be assumed to fit one existing Root enrollment.
The actual Friend/Auth provider integration and native cases must resolve that
capacity boundary without truncating authorized counts, treating cached metadata
as authority, silently dropping authors, minting extra Roots, or increasing a
shared cap from this lane. The current missing-provider/unavailable boundary
remains truthful. No scalable multiuser acceptance is claimed from this source.

C04 parsing has been extracted into the same closed transport parser with
negative source cases for foreign operation IDs, duplicated parameters, nil or
changed keys and write-shaped GET bodies. Reply cases reject independent
audience/expiry/author fields; create cases reject duplicate JSON keys and
inconsistent landmark selections. These added Rust cases are NOT_RUN until the
actual Cargo test admission includes them.


## Selective source02 and later actual fixes

N01-RUST-SOURCE02-0204 passed actual ACCEPT1793845079.4116201 /
FULL1793845080.640518 with complete LIVE initials/continuous guards and all0.
Exactly authority/producer/wire plus native test/support were parsed/formatted,
format-checked and scoped-whitespace-checked (all exit0). Peak5PIDs/102,301,696
RSS,434,176 cumulative conservative source-family bytes, minimum free
23,989,301,248 bytes. Source01 and source02 inputs/outputs/controls and original
clocks stay immutable; neither closed round restarts. Cargo/tests/SQL/native
execution remains NOT_RUN.

A later actual ordering fix moves ordinary committed native-witness writes
until after all current authority calls and immediately before commit. Mutation
completion is the final hook; C04 lookup witnesses only its second actual stored-
target authorization. This avoids holding a Root SHARE/FK lock while a later
child registration needs Root UPDATE. This post-source02 change is unqualified
pending the next real source/Cargo admission; the prior pass is not reused.

Root separately granted ONLY three static Notes registrations in
`concert-application/src/commands/registry.rs`, the narrowly scoped opaque
landmarkId path exception/adjacent doc in commands/mod.rs, and new
`tests/venue_notes_command_registry.rs`. Exact frozen contract8d4561d9 supplies
methods/templates; this is an additive static registry correction, not authority
or HTTP mounting. Every other parameter retains existing UUID/platform,
duplicate/missing/unknown checks. New source cases preserve all59 historical
(id,method,template) triples and test traversal/percent/control/nonASCII rejection,
legacy behavior and original-key HMAC changes across body/landmark. They remain
NOT_RUN. Central generator reconciliation remains Root-owned. The exact private
pre/post/inverse-byte proof is registry-hunks/preservation.json; publisher MUST
compose minimal hunks into current canonical rows, preserving newer peer/Mural
registrations missing from this historical private checkout.

The exact legacy count-line grant changes only `OPERATIONS.len()==59` to
`OPERATIONS.len()>=59` locally. Canonical41d90738 has62 and additional explicit
Mural assertions; publication must change only that count line and preserve those
assertions. The frozen59 registration triples are checked by the new Notes
registry regression test; that test is still NOT_RUN.

Source03-0229 actually accepted1793845845.3981981 and released FULL
1793845845.573758 before any formatter/parser command. The guardian refused its
initial storage check. Its conservative gross allocation included complete
recently modified shared-Git inodes:1,063,251,968 bytes plus prior434,176 bytes.
This is not a measurement of net Notes growth. The higher conservative bound
and immutable failed receipt remain retained; there is no source03 PASS or restart.
The prospective Cargo offer remains unaccepted until the actual retained-target
turn and fresh capacity. Domain/application cases precede service checks; the
ignored native cases may be compiled but not executed by that Cargo offer.

The current producer also hides `can_withdraw` in READ_ONLY/ARCHIVED phases,
matching the actual mutation's writable-phase check. This later source correction
is unqualified pending the next real formatting/type/runtime round.

Source04-0234 then actually accepted1793846053.2114182 and released FULL
1793846055.527227, all6 commands exit0. At frozen preHEAD245c3f5f68304b3b4715a5f24be39e2a74032220,
the current producer and new registry test were formatted and format-checked;
the three shared command files were syntax-only with exact preimages preserved.
Both independent guards completed LIVE initial/continuous samples. Peak5PIDs,
77,430,784 bytes RSS; conservative nonshrinking source-family upper1,138,683,904
bytes, including historical gross-Git carry and the full ordinary PREP32 bound.
Minimum free23,867,527,168 bytes. All resources were released before inspection.
This qualifies current source parsing/formatting only. Rust type checks, the
registry/domain/application cases, SQL and native/service execution remain NOT_RUN.


## Actual Cargo01 admission and preserved failure

`N01-CARGO01-0229` actually accepted1793849576.620119 at frozen source
`eb89c826296c2e1f8507f976563433208ffc77c7`, with original WORK1793850476.620119
and FULL1793850656.620119. World05 explicitly yielded before acceptance; the
last actual207 TARGET/FULL and input-only Rust1.88/profile/root handoff remained
the custody source. Resource21d's common publication order was separately
satisfied by observing exact `BE03-EMAIL-NATIVE02-0329` actual acceptance. No
chat acknowledgment or technical review was an admission requirement.

Both actual controller/guardian identities were published before heavy discovery.
Both complete initial LIVE censuses passed. The full unique-inode baseline was
29,967,241,216 bytes; the higher historical WHOLE58,943,049,728-byte floor remained
charged without refund. Recent shared-Git positive gross growth113,954,816 bytes
raised prior Notes carry from1,155,461,120 to1,269,415,936 bytes, leaving
5,173,035,008 bytes within the ONE cumulative6GiB cap. The separate runtime
target5.5GiB/outside0.5GiB limits, whole64GiB,12PIDs/3GiBRSS,9GiB residual and
8GiB live free floors remained binding.

A later continuous census at1793849681.978139 rejected ordinary owner153
PID35913 with `ordinary observation command/cwd mismatch`. The receiver had
already checked the exact script/executable/birth controller descriptor, but
then independently required a command digest in its ordinary observation.
The responsible owner reports using its full kernel descriptor instead of that
digest. This establishes a receiver-format incompatibility to address; it does
not establish peer noncompliance or a Notes application failure. Future controls
need narrow descriptor-or-digest handling with separate diagnostic stages. The
closed controls and failure are preserved; no repair was executed under the
closed clock.

No Cargo, installed-tool version, Rust test/check, native/SQL/HTTP command or
binary copy was reached. The only commands were target-handle proof, clean
source status and exact Git identity checks. Git checks returned0; lsof returned
its expected1 with no output/open handles. The round is FAILED, not a product
PASS or generic guard qualification. Peak5PIDs/559,841,280 RSS; target growth0;
cumulative conservative NEW1,314,226,176 bytes, including44,810,240 this-round
upper; nonshrinking WHOLE58,987,859,968 bytes; minimum free23,288,143,872 bytes.
All higher subsequent observations must carry forward.

Actual TARGET1793849683.282016 is
`/private/tmp/mcr-venue-notes-213-0122/cargo01/run-01/target-release.json`, SHA256
`cc598cc7144d0cc394729a9bc1e6282b4a53b8e766b214b3eb1131e31d07a28f`.
Actual FULL1793849683.2824051 is the adjacent `full-release.json`, SHA256
`88fc5afd704dde7f68d592cf05e5a8fdfc93cccf540f107b924ffe102820eb15`.
The guardian and owned child were reaped, target handles were zero, cleanup had
no failure and the tool session exited1. The canonical index preserves the
literal closed history and raw hashes. A completion poll and diagnostic read
were batched in one tool call, so traceback output preceded the direct release
message; this notification-order gap is retained rather than retrospectively
certified. The release was then sent directly to21d and downstream owners.

The actual control archive is immutable at
`cargo01/source-controls-actual-1793849576620119000` under the same private
evidence root, manifest SHA256
`60fa979b5cd1d84a7660e3914ef4c61ba4e73beb7fae50e70441f2245c8adb42`.
The immutable target/profile/root input handoff SHA256 is
`cfe452ce6da635a0ae6873c7e13425fbe8d93091544359944072afc27a5757b0`.
It records the required authored profile and actual zero target growth; no new
compiled-profile proof is invented.

N01 remains IN_PROGRESS. Domain/application/SDK/registry/wire cases, service
type check, ignored-native compilation, actual SQL/native retention/EOF/cancel/
restart/public routes, current Auth/Friend/L providers, scalable author capacity,
cleanup and Meet Here integration remain unaccepted. A distinct future Cargo
round must follow the allocator's ready queue and retain all history inside the
same cumulative6GiB; the closed Cargo01 offer cannot restart.


## Actual Cargo02 admission and preserved failure

`N01-CARGO02-0343` actually accepted1793851904.990769 at frozen source
`f25605070e84b4e6e2e89a662e36d7c7fd64cc2d`. Original WORK1793852804.990769
and FULL1793852984.990769 were not extended. The immediate predecessor was
QA03 inventory's actual TARGET/FULL, with inherited Notice/BE10 profile facts;
QA03 inventory and Notes Cargo02 provide no new compiled-profile qualification.
Both Notes controller identities were published before heavy discovery. The
initial primary capture completed by the actual gated control flow, and the
initial guardian LIVE snapshot is retained. A later primary sample overwrote
its initial full snapshot: that missing initial artifact is not reconstructed.

Later paired primary/guardian samples at1793851935.2454479 and
1793851935.248399 rejected publisher412e ordinary PID97140 with a present
command digest before attempting its cwd probe. Diagnostics did not distinguish
digest equality from missing purpose or descriptor-cwd disagreement, so they
cannot identify that narrower cause. Root153's exact descriptor observation
actually passed both samples, with verified live cwd; this is evidence for the
specific prior receiver repair, not general guard or application acceptance.
The receiver hashes process-command text. A later mutable publisher observation
explicitly declares a Python sys.argv JSON hash domain. Those domains differ;
the later record does not establish the failure-time producer fields. The
implementation owners are resolving that identity interface directly.

No Cargo, tool-version, Rust test/check, native/SQL/HTTP command or copy ran.
Initial target-idle lsof was not reached; only cleanup target-handle lsof ran,
returning its expected1 with no handles. Actual TARGET1793851936.59966 is
`/private/tmp/mcr-venue-notes-213-0122/cargo02/run-01/target-release.json`, SHA256
`bf5d3f75a0f1ad8a0e0845568bd88e734d8011bcada0e263de7373c7c412d2e3`.
Actual FULL1793851936.600086 is the adjacent `full-release.json`, SHA256
`08cee298883df2af2c8f1133586579ff4cee58570b74916f92de0c9fa06dff08`.
The run is FAILED, tool exit1, cleanup failure null, guardian and owned child
reaped, zero remaining resources and target growth0. Literal releases were sent
directly to the allocator and downstream owners before diagnostic inspection.

Peak Notes4PIDs/932,380,672 RSS; minimum free22,505,414,656 bytes. Conservative
cumulative Notes NEW1,413,828,608 bytes includes prior1,374,580,736 plus this
round39,247,872. Nonshrinking WHOLE64,513,339,392 bytes and every higher later
observation carry forward. The ONE6GiB cumulative cap and all original separate
physical, runtime-growth, WHOLE and live-resource limits remain binding.

Immutable actual controls are under the same evidence root at
`cargo02/source-controls-actual-1793851904990769000`, manifest SHA256
`e18c349449db464367b359a22ba3668fa559d3ba5387e570a6397a93375701c2`.
The actual outgoing target/profile/root handoff is
`cargo02/target-input-handoff-b2ab38bebd4e906c451b07bdf385d9ace2920058fc227037490da5e6a5626488.json`,
SHA256 `b2ab38bebd4e906c451b07bdf385d9ace2920058fc227037490da5e6a5626488`.
It was sent directly to L16e and World20f. L02 subsequently yielded its distinct
`L02-CARGO-CHECK01-0410` without acceptance, target access or Cargo; its immutable
UNREADY_YIELD SHA256 is
`3f113ed070accd121be593d4cfe6c4b01a1746acb4b1ec787f2605106e757f6c`.
An unused yield is not an actual L02 FULL or a replacement compiled profile.
World06 then Gift retain their allocator queue positions. Notes requires a
fresh distinct allocation; Cargo02 cannot restart. All outstanding product
validation and implementation boundaries listed above remain IN_PROGRESS.


The publisher subsequently supplied the actual failure-time PID97140 evidence:
`/private/tmp/mcr-publisher-412e-ordinary-history/42055-1793852385-144128/FACTUAL_PID97140_ENCODING.json`,
SHA256 `97cb6d1d12986b2a4a52c40831fc262e68da01f94b29c4d85653246d82106825`.
Its original immutable ACCEPTED.json SHA256 is
`53665f143672834e12eb6f755d31dffd3717311213ccde115ccdbe2e95291ff4`,
accepted1793851933.192179, observation1793851933.192189; its literal FULL is
1793851940.065022, SHA256
`75189923f6ce35e47378e3ca917cbc4f6e83643654199f64dcf484dcefea4871`.
The original acceptance already declares
`sha256:python-sys.argv-json-utf8-v1`. Hashes of all four referenced bootstrap,
intent, acceptance and FULL files were checked. The declared digest
`3f2d2eb7f8f700f5ab4d3bec31042231d1fa840f91e20bcaac42584a5e860f8d`
was reproduced from retained captured argv with its exact JSON canonicalization.
This confirms the producer/receiver format incompatibility. Historical raw ps
bytes and the receiver's original receipt readset were not captured, so no
claim is made about exact bytes the original reader consumed or any repaired
historical result. No reused-PID probe, target access or product execution was
performed. The future typed adapter must keep exact live birth, OS image,
program bytes, cwd, purpose, receipt binding and all positive resource limits;
unknown or contradictory observations still refuse. Future controls are source
preparation only and have no granted execution entrypoint yet.


## Cargo03 receiver preparation: source only

The direct default-host counterpart is developer
`01a1a1fe-1eed-7d12-a247-879cd521e0ec`. Its committed source
`f29984173c3e023040d1ea7b4225a7fa1404d509` supplies inspectable
`kernel_fact`, `actual_cwd`, `matches` and `typed_publisher_ordinary` helpers.
The responsible counterpart reports actual Default03 acceptance1793852609.315866,
FULL1793852616.207422, both initial LIVE guards passing and11 controlled receiver
regressions exit0. Its overall run still FAILED on another ordinary process
before browser execution. These are counterpart-reported observations, not a
Notes validation result or an application acceptance claim.

Notes adapted those functions in private `cargo03/typed_ordinary.py`, with exact
source revision/path/SHA and changes in `typed-helper-provenance.json`. Unlike
the counterpart helper, Notes refuses when a PID retained in the complete ps
snapshot has no live kernel/cwd evidence, including a FULL activity finisher.
Program reads are bounded to nonsymlink regular files of at most1MiB. The
registered publisher's actual program/birth/image/cwd, supported typed argv,
literal acceptance/FULL and unchanged ordinary booking must all agree; a new
filename alone neither grants permission nor requires a manager acknowledgment.

`cargo03/readset.py` projects fields actually consumed by Notes' reader,
including literal closed-offer clocks/SHA, controller identities, program bytes
and ordinary future reservations. Its snapshot protocol compares that projection
before and after a complete strict process census. A relevant change permits
at most three complete attempts, with original retry events retained. Stable
mismatches refuse. Exact kernel-birth-bound history may identify a still-live
metadata finisher; it does not release workflow reservations or excuse a missing
identity. The first successful consistent full census is separately preserved.

Twelve controlled Notes checks are authored in
`cargo03/typed-ordinary-tests.py`, adapting the counterpart fixtures and adding
Notes' historical-closure/booking projection check. They use retained original
PID97140 records with controlled OS probes; their source does not authorize a
live or reused-PID query. They are NOT_RUN. All current private control hashes
are in `cargo03/SOURCE_PREPARATION.json`. No parser, guard, census, target access,
Cargo, SQL/native/HTTP/browser run or new target custody has been performed by
this preparation. `cargo03/NOT_GRANTED.json` has no allocation or entrypoint.
The requested next finite scope starts with these controlled checks, then the
original six Cargo stages; it cannot restart Cargo02 or reset any carried cap.


Resource21d then granted distinct conditional `N01-CARGO03-0430`, after current
QA03 build, Notice02, L03 and exact `WORLD-BUILD07-0427`; existing accepted custody
wins. This is not an actual Notes acceptance or permission to start early. The
new03-only WHOLE ceiling is72GiB, explicitly replacing64GiB prospectively while
retaining incoming WHOLE at least64,683,737,088 bytes and every higher observation.
The ONE cumulative6GiB growth cap retains Notes at least1,413,828,608 bytes and
all higher carry; it is not fresh6GiB. This-round target5.5GiB/outside0.5GiB remain
subordinate to the tighter total remaining growth. Physical admission still
reserves the full6GiB future plus every other applicable future, requiring
9GiB residual,11GiB start and8GiB live free floors. Global4/3/2/1 workflow/source/
browser/target and96PIDs/18GiBRSS limits remain unchanged. Both actual initial
LIVE guards precede any target lock, stat or traversal in the new source controls.
Each guard independently computes current future reservations from its own census.

The new finite offer permits exactly the12 controlled Notes receiver cases first,
then the original six Cargo stages, Rust1.88 locked/offline/j1/debug0/incremental0;
no PostgreSQL, SQL execution, native execution, HTTP or browser. The fixed bounds
remain12PIDs/3GiBRSS,900 seconds WORK and1080 seconds FULL, with acceptance before
07:00UTC. Existing source-preparation16MiB remains the same unspent book through
06:00UTC only. The actual outgoing World07/current last-writer TARGET/FULL and
coherent profile/tool/root/higher-history packet are prerequisites, not a review
or acknowledgment gate. World confirms it has not accepted07 yet and will send
that packet directly when its turn actually closes. Its64GiB ceiling does not
inherit Notes' distinct72GiB grant. Cargo01/Cargo02 failures and all frozen source
and receipts remain unchanged.


The allocator subsequently published an exact prospective host PostgreSQL
infrastructure disposition for PID3029, parent1, kernel birth1793657831.120962,
loaded image `/opt/homebrew/Cellar/postgresql@16/16.14/bin/postgres` and the exact
recorded raw-ps argv digest. The original immutable observation SHA256 is
`7e457ad7ae57bd5ea690d66207d2b627d02c816e69b2e3fc395ad62ea53febc6`;
the compatible envelope SHA256 is
`39a98adaadaad684e3e681f671ac1625ae07dfea21dd10c74c1ccb06e140b9e9`.
The current resource-receipt interface binds that envelope and original provenance.
Notes' new source adapter checks those exact bindings and requires fresh matching
kernel birth, OS image, parent/group and raw argv when the PID is observed. It
counts that physical PID/RSS without granting a workflow or database usage. The
wrapper's `loadedImageAndCwd` is an n-prefix image encoding; cwd is NOT_OBSERVED.
No lsof cwd result, launchd ownership, connection, query, data-directory access,
signal, restart or broad PostgreSQL exception is inferred or authorized.

Current future accounting prospectively uses one maximum target bucket including
Notes' full6GiB, ordinary combined future2.5GiB once, and every other applicable
future. The12GiB planning floor and every higher actual input remain binding;
the9GiB residual,11GiB source-start and8GiB live floors are unchanged. New source
assertions cover exact infrastructure mismatches and target/ordinary bucket
deduplication inside the12 controlled unittest cases. These assertions and both
new live guards remain NOT_RUN. A cheap metadata free-space lower bound is not
actual process census, cap fit, source qualification or acceptance proof.

Prospective outgoing order is now N03, Root1fc's
`ROOT-BE42-COMPILE-SIX01-0453` under joined0453, then12cc's
`BE03-CARGO-CHECK01-0435`, then QA03's fresh Build02. Actual N03 TARGET/FULL and
coherent root/profile/tool/source/higher-history packet go directly to Root1fc,
12cc and allocator21d before diagnostics;12cc cannot acquire ahead of Root's
actual joined disposition. Existing accepted custody/clocks still win, and no
review or acknowledgment is required. Notes remains unaccepted while its exact
Notice02/L03/World07 predecessor handoff and fresh admission are pending.

Future ordinary metadata publication also records factual `endedAt` equal to its
literal FULL activity epoch on observation/controller, with `kernelExitAt:null`.
A routine metadata publication demonstrated exact agreement; this is no kernel
exit, terminal-identity waiver, guard or product qualification. Historical
receipts, Cargo01/Cargo02 failures and their frozen writer bytes remain unchanged.


## Cargo03 actual closure: failed before target access

The historical conditional/preparation descriptions above are superseded by this
actual disposition. Source HEAD was `9193dbd0e5832631002ed087b0a7172ebef0d990`;
Rust implementation and frozen contracts remain unchanged. World07 actually
closed after a pretarget infrastructure-schema failure; its literal outer FULL
1793855278.774934 and coherent packet SHA256
`9aca2451b67b13c988944e01b9861d7674647c7b3d54554499dcc2fa1a8bb4ed`
were consumed with the inherited last actual Notice02 packet. No World compiler
qualification or new complete baseline was inferred.

A metadata-only refresh first refused because the Notes reader applied the
publisher's stored-program binding to another owner's stdin `-`. That refusal
is preserved as `cargo03/metadata-cap-refresh-1793855246203.json`. Source changes
restricted publisher-specific literal/program binding to its protocol and added
separate typed observation handling for exact stdin and hash-bound stored
scripts. Current observations are distinct from historical controllers; JSON
sys.argv digests are never compared as raw ps digests. Live birth/image/cwd and
physical resource accounting remain mandatory. The subsequent metadata-only
refresh1793855586.877595 passed with prospective G1/T1,12PIDs/3GiBRSS and the
12GiB future floor. This did not qualify a live guard or the authored tests.

`N01-CARGO03-0430` actually accepted1793855610.963755, with original WORK stop
1793856510.963755 and FULL stop1793856690.963755. Both initial strict censuses
refused the exact host PostgreSQL identity at1793855611.134979 and
1793855611.141271. They consumed the prospective wrapper SHA256
`f2a799ae2beb44eb0b3f3507a2a2af08370b4311c45c7d1aaa5dada7dcade269`.
The retained diagnostic is `exact host infrastructure birth/image/argv mismatch`
for PID3029. Complete raw process rows and kernel predicate operands were not
retained on this stable refusal. The exact failed subpredicate and historical
operand bytes are therefore unproven; no retrospective probe or replay is used
to claim them. Ordinary observation diagnostics had no live candidates in those
samples, so the new typed adapters are also not runtime-qualified.

The target lock was never attempted or held. Target stat/traversal, a new complete
inventory or baseline, copies, tool-version commands, all12 controlled cases,
all six Cargo stages, SQL/native execution, HTTP and browser remain NOT_RUN.
Literal never-held TARGET1793855611.621166 is
`cargo03/run-01/target-release.json`, SHA256
`b482b258bbb181c14b1ce61a2a088122c11bde52043cb3f9a8f379847bf6ce00`.
Literal FULL1793855611.621989 is `cargo03/run-01/full-release.json`, SHA256
`5f92865bce3908c05cc1fed7e466e9252b9bdf57bd1bce4a2a615819ec059a92`.
The process exited1, guardian and owned child were reaped, target was released,
and cleanup_failure is null. No target lsof scan was needed for never-held
custody. Exact literal paths, hashes and clocks were sent to Root1fc,12cc and21d
before diagnosis. No review, acknowledgment or restart hold was introduced.

Actual ten-file control source was hash-checked against admission and frozen at
`cargo03/source-controls-actual-1793855610963755000`, manifest SHA256
`b1fd42e2dbe8c69f8ca8fba9e36f54cc8eedf4cbf1706f3df7334c6a35fb6ca5`.
The immutable outgoing packet is
`cargo03/target-input-handoff-f8b5cc9522c118fecf49132be81c75984dcd82320bda796b041221baeccf7e99.json`,
SHA256 `f8b5cc9522c118fecf49132be81c75984dcd82320bda796b041221baeccf7e99`.
Retained WHOLE is at least67,177,394,176 bytes and cumulative Notes NEW at least
1,464,160,256 bytes, plus every higher future observation. These conservatively
add startup32MiB and the entire unchanged ordinary16MiB allowance to incoming
floors; they are not new inventory measurements or a fresh growth budget.
Target growth is zero because it was never acquired. Root0453 is the next owner,
then12cc0435, then QA03, each under its own grant and fresh admission.

Cargo03 is closed and cannot restart. Notes remains IN_PROGRESS with developer-
owned guard diagnosis, controlled checks, Cargo validation and all original
product/runtime obligations outstanding. Any further finite execution needs a
distinct allocation; ordinary source/documentation preparation alone does not
authorize a guard, compiler, test or live diagnostic run.


## Separate identity diagnostic Source01: closed with a captured cause

Resource21d granted `N03-IDENTITY-CONTROL-SOURCE01-0517`, originally6PIDs/1GiBRSS,
4MiB growth inside the same cumulative budget,120seconds WORK/180seconds FULL.
Actual acceptance1793855941.4833229 preceded receipt of an amendment explicitly
addressed to an UNUSED offer. That unused amendment was not used to expand or
reset this actual round. Original WORK6061.4833229 and FULL6121.4833229 remained
binding. No target custody, stat/traversal, Cargo, PG connection/query/data/signal,
product tests, HTTP or browser was authorized or performed.

Two independent initial samples retained exact per-predicate operands at
1793855941.669081 and1793855941.66939. PostgreSQL PID3029 had ps `comm` equal to
`/opt/homebrew/op` (16characters), while the kernel reported the exact full image
`/opt/homebrew/Cellar/postgresql@16/16.14/bin/postgres`. Parent/group, ps birth,
raw argv/digest, kernel seconds/microseconds, full kernel image and kernel
stability all passed. Only the comparison treating that intermediate ps `comm`
column as a complete path failed. This is fresh Source01 evidence; Cargo03's
missing historical operands remain missing and its original failure unchanged.

The private raw predicate files are
`/private/tmp/mcr-venue-notes-213-0122/diagnostic04/run-01/global-census-primary-round0.json`
and the corresponding guardian file. Compressed consumed-input snapshots retain
the exact metadata values/process rows. Apple's published `ps/keyword.c` defines
comm's default width16; `ps/print.c` `p_command_and_or_args` truncates nonfinal
fields to their declared width. Sources:
https://raw.githubusercontent.com/apple-oss-distributions/adv_cmds/main/ps/keyword.c
and https://raw.githubusercontent.com/apple-oss-distributions/adv_cmds/main/ps/print.c.
These source facts explain the new observation; they are not repaired execution
proof. No process-name prefix alone may substitute for exact full kernel image,
birth, parent/group, raw argv/digest or current positive resource accounting.

Attempted fixes did not qualify: the colon-width keyword was unsupported, and
an incorrect15-character comparison still refused the observed16-character
value. The original work-minus15 cleanup cutoff was honored. All12 receiver
methods and the proposed controlled infrastructure predicates remained NOT_RUN.
A subsequent16-character representation correction is source only, unqualified.
The first preparation hashes and raw runtime operands are retained; source files
changed during the bounded diagnostic, and complete byte snapshots of every
intermediate source version were not retained. No exact historical source-byte
archive or runtime qualification is claimed for those missing versions.

This diagnostic also exposed a separate producer defect: the guardian was launched
with cwd `diagnostic04`, while a helper copied its caller's worktree cwd into the
child descriptor. Actual live lsof cwd was captured and the current descriptor
corrected in `diagnostic04/run-01/actual-guard-cwd-correction.json`. Root153 was
informed directly; its SOURCE07 failure remains unchanged. Future source guards
must publish their own actual cwd before the controller declares their identity.

Literal FULL1793856046.54126 is
`diagnostic04/run-01/full-release.json`, SHA256
`0d39caaf5fe88586757c6d6d50eb77d8ee8acbdb77517509614af05b59d3e0a5`.
Exit1/FAIL, guardian and owned child reaped, target never held, retained allocated
bytes1,306,624. The literal closure was sent immediately to21d before the detailed
post-closure report. The entire original4MiB is conservatively charged: Notes
NEW at least1,468,354,560 and WHOLE at least67,181,588,480 bytes, plus all higher
observations. No ongoing reservation, restart or fresh family budget exists.

Fresh `diagnostic05` source preparation restricts execution to one independent
LIVE per observer, obtains the guardian's cwd from its own process, retains exact
raw predicate context, and would run the same12 controlled methods only after
both admissions pass. Its typed adapters obtain the executable from the kernel,
not a truncated intermediate ps column. It has no new execution authority yet.
A distinct Source02 allocation was requested; conditional `N01-CARGO04-0517`
requires actual changed-control qualification, frozen new pins and its later fair
queue custody. Notes remains IN_PROGRESS; no implementation acceptance follows
from these source/control records.


## Identity diagnostic Source02: live guards passed; three fixture errors remain

Distinct `N03-IDENTITY-CONTROL-SOURCE02-0528` accepted1793856705.866855 under
4PIDs/512MiBRSS,16MiB inside the same cumulative budget,120seconds WORK and
180seconds FULL. Both newly frozen independent initial LIVE censuses passed.
The exact16-character ps representation and mandatory full kernel image, raw
argv/digest, birth and parent/group checks passed without target or PG usage.
Guardian identity now came from its own kernel/image/cwd rather than its caller.
Typed observations likewise use the actual kernel executable; an intermediate
truncated ps column is not treated as the OS executable path.

All12 existing controlled receiver methods ran once: nine passed, three errored.
The errors were `KeyError: metadata` in the diagnostic recorder because the three
consistency tests supplied minimal mocked snapshots without that newly consumed
field. The affected tests cover registration-between-reads retries, same-PID
exec transition, and stable mismatch/retry exhaustion. Their fixture correction
adds the consumed metadata shape and a separate retained scratch directory per
invocation. It preserves the strict immutable recorder and all original
assertions. Nine unaffected passes remain valid; they are not a twelve-test pass.

Literal FULL1793856706.268505 is
`diagnostic05/run-01/full-release.json`, SHA256
`75f83d5bebf6f1d3e031eeb2a5e87cdeedf2730da03b9f7629943e17915f856e`.
The round exited1/FAIL with guardian and child reaped, target never held, and
835,584 retained allocated bytes. Its literal release was routed immediately
to21d before the test log was read. The entire16MiB is charged conservatively:
Notes NEW at least1,485,131,776 and WHOLE at least67,198,365,696 bytes, plus every
higher observation. No target, Cargo, SQL/native execution, HTTP or browser ran.

Exact unchanged actual control source was verified against the preaccept pins
and copied to `diagnostic05/source-controls-actual-1793856705866855000`.
The original frozen-source-pins SHA256 is `415568e7c6c4cd0336d7435f79ba8a0a3193510e08fa0d56a711795a48b96a50`.
`diagnostic05/ACTUAL_SOURCE02.json` binds the literal acceptance, both live
results, both raw census records, receiver test log and FULL. Cargo03 and prior
Source01 failures remain unchanged; no historical missing operands are inferred.

The three-fixture-only rerun is prepared under `diagnostic06`; it has no new
grant yet. A distinct bounded Source03 allocation was requested after Root153's
reserved admission window. Conditional Cargo04 still requires completion of
control qualification and its actual fair-queue target handoff. No nine-test
replay, closed-round restart, reviewer acknowledgment or implementation
acceptance is inferred. Notes remains IN_PROGRESS.


## Identity diagnostic Source03: three fixture methods passed

Resource21d prospectively replaced the stale12GiB planning projection with the
current named future reconciliation:9,586,068,447 exact bytes,9,596,567,552 rounded
upper bytes. Ordinary2.5GiB and mutually exclusive warm target6GiB each occur once;
named source/browser phases, including Notes'4MiB phase, also occur once. Fresh
same-owner actual phase growth raises its named bound; genuinely nonoverlapping
actual reservations are added. Historical cumulative NEW/WHOLE charges remain
unchanged. Residual9GiB, live8GiB, source11GiB/browser12GiB start floors and all
global class/PID/RSS/identity limits remain binding. No prior capacity failure
is retrospectively changed. Older controlled12GiB planning assertions are
historical policy evidence, superseded prospectively by this explicit direction.

Distinct `N03-IDENTITY-CONTROL-SOURCE03-0548` accepted1793857969.4152908 under
4PIDs/512MiBRSS,4MiB inside the same6GiB family,60seconds WORK/90seconds FULL.
Two new independent complete LIVE censuses passed at1793857969.592435 and
1793857969.613185. Each computed exact9,586,068,447/rounded9,596,567,552 once,
with identity_errors and unregistered_MCR_processes empty. Actual free-space
assertions passed; their numerical free-space operands were not separately
retained, so the preaccept cheap free bound is not relabeled as a live sample.
The allocation snapshot SHA256 is
`03b7f64febadf82eaa84a6876baf452fad44d259fe14f47c5be5d17d848b9082`;
the reconciliation snapshot SHA256 is
`79071580e9f60dfce629edc77f3ee0e10196d22c6d90a59db10ea9fe7bbed5d6`.

Exactly the three previously errored consistency-fixture methods then passed:
registration-between-reads retries, same-PID exec transition, and stable mismatch/
retry exhaustion. Their corrected synthetic snapshots include the consumed
metadata field and use distinct retained scratch directories. The immutable raw
recorder and all original assertions remain active. The raw log reports
`Ran 3 tests` and `OK`, SHA256
`edb4e2159c923ea45a32656078835cdf531c3d7b414c670dd786978875cb0678`.
Nine unaffected Source02 passes are retained without replay. This establishes
the12-method aggregate across the named sources, not a new twelve-test run or
renewed old-capacity-policy assertion. Source02 remains overall FAIL.

Literal FULL1793857969.771434 is
`diagnostic06/run-01/full-release.json`, SHA256
`f2c0e52c0079f6b6f0ddab6c7fed0ebfe25452acdf1818551ba179faacea0561`.
Exit0/PASS for allocated Source03 scope, guardian and owned child reaped, target
never held,917,504 retained allocated bytes. Entire4MiB is conservatively charged:
Notes NEW at least1,489,326,080 and WHOLE at least67,202,560,000 bytes, plus all
higher observations. Literal acceptance/FULL was routed directly to21d,12cc,
Notice205 and Root1fc before diagnostic reads. No target/compiler, PG connection,
SQL/native execution, HTTP, browser or product acceptance is claimed.

Unchanged actual source was hash-checked and frozen in
`diagnostic06/source-controls-actual-1793857969415290800`; preaccept pins SHA256
`50fa985b5031595f13d102e268b5d9d489cb5fbe073ff114c6f7510f7446080a`. `diagnostic06/ACTUAL_SOURCE03.json` binds exact acceptance,
both live results/censuses, the test log, FULL and retained Source02 evidence.

There is an operational publication-order gap: admission explicitly checked
PI08's actual source publication and207's exact source03 FULL, and RootAuth09 was
observed resolved, but the preceding RootBE42 source02 tuple was not checked.
Current Root metadata later still showed its older input-preparation FULL. The
gap was reported directly; no Root yield, earlier acceptance or corrected clock
was invented. Notes' actual publication does not waive any unresolved predecessor
constraint for another owner. There is no reviewer or acknowledgment gate.

Conditional `N01-CARGO04-0517` source preparation imports the qualified current
capacity/identity reader, binds the12 retained receiver results and removes their
replay. Its original six Cargo stages remain unchanged. Actual execution still
requires the exact207/World08 fair-custody closure, current profile/tool/root/
higher-history packet, named next-owner routing and fresh admission. No target
access or Cargo has begun. All product/runtime obligations remain IN_PROGRESS.


## Cargo04: guardian publication failed before target access

World08 genuinely yielded its never-accepted `WORLD-BUILD08-0512` at
1793858746.14131, without target access or resources. Its immutable disposition
SHA256 is `f0a3d3dc1348099db8cb160629c301559ab8325414ecb53402f1252a87c855c4`.
The actual preceding custody remained207 COMPILE03: coherent packet SHA256
`1f5033112928ce0c1cc86f85668e39a5e5a71d0fbd23507c05a8d98a5dc0d8c3`,
TARGET `b2d5237a198ac658915d6f9444ab71fb77561dd24c081c2e7b3e674855f490a9`,
FULL `75882902118c2833f68801ba9fabc14e2e7472833ed670c2d66828f4f2ea9871`.
Its175 roots and WHOLE floor67,282,251,776 bytes were retained. No World08
TARGET/FULL, new target baseline or predecessor compiler pass was invented.

The allocator repaired a stale reconciliation pointer by publishing coherent
current phase projections. Cargo04 consumed the fresh current allocation and
linked proof during preaccept; earlier9,596,567,552 and9,797,894,144 scalars
are historical observations, not fixed current requirements. Phase amounts,
exact owner/offer bindings and proof asOf must agree. Ordinary2.5GiB and T1max6GiB
are counted once; actual higher nonoverlapping reservations remain binding.
All historical NEW/WHOLE charges and start/residual/live floors are preserved.
This metadata repair did not replay the twelve retained Source02/03 cases.

A preaccept refusal matched the literal authority text too narrowly: the stored
grant said "direct conditional dispatch" and the source expected "direct dispatch".
Both exact authorized strings are now accepted; this refusal had no actual
ACCEPT or target access and did not start or reset a finite round.

Actual `N01-CARGO04-0517` accepted1793858957.9035661 with its original
12PID/3GiBRSS, SAME6GiB family/72GiB WHOLE and900/1080second WORK/FULL limits.
It failed at the parent's `identity(guardian.pid, guardian.py)` assertion:
`actual birth/image/cwd unstable`. The exact kernel/cwd operands were not retained;
an exec/startup race is not established as historical fact. The guardian exited
without its own failure. No discovery-ready marker, complete initial LIVE pair,
target lock/stat/traversal/baseline, Cargo/version command, source copy, test,
native execution, SQL, HTTP or browser workload occurred. Commands and stages
are empty; cleanup_failure is null; guardian and owned child are reaped.

Literal TARGET1793858958.0467908 is `cargo04/run-01/target-release.json`, SHA256
`bfcc2770132c40d601036fcc22ba8c4a667af392aa8b185be51e93780b270158`;
never_held is true. FULL1793858958.047186 is
`cargo04/run-01/full-release.json`, SHA256
`2b4c4a53deb51b361568357d7e7140498ea1b9fb5c37bafa8d75d400f58a9b0a`.
The controller exited1/FAILED before its original deadline. Both literals were
routed immediately to21d and BE03 before diagnostic reads. The allocator's
exact next-owner routing to L05 arrived after closure and was forwarded directly
to16e immediately. RootSIX02 and BE03CHECK02 are genuinely unready unused offers;
neither creates a source-completion or acknowledgment barrier. L05's exact
successor offer is `L05-CARGO-CHECK01-0548`.

The entire startup32MiB is conservatively charged: Notes NEW at least1,522,880,512
and WHOLE at least67,315,806,208 bytes, plus every higher observation. Immutable
actual source archive `cargo04/source-controls-actual-1793858957903566080` has
manifest SHA256 `e771e3892f478953dd87be42e3fe6a2986d73c309f34bd569f02cb7c13fecee6`.
Every copied actual source/input matched its acceptance pin before freezing.
The181-root outgoing packet is
`cargo04/actual-target-input-handoff-897de2eaf76970280cc1556d6b1751a3b9da8e04cb1907f0db62bd20a81392f4.json`,
SHA256 `897de2eaf76970280cc1556d6b1751a3b9da8e04cb1907f0db62bd20a81392f4`.
It was sent directly to16e,21d and Notice205, without a review/ACK gate.
Its target allocation upper remains inherited Notice03 metadata; no new completed
baseline or deletion credit is claimed. Cargo04 is closed and cannot restart.

Default Venue Host separately reported Native14 refusing ordinary PID11461.
The original retained Notes ordinary BEGIN1793858942.431221 and activity
FULL1793858942.759685 were supplied directly to1eed and21d, together with exact
birth1793858942.225836, parent49590/group11461, executable, cwd, entry script SHA
and rawPS digest. The original writer omitted a digest encoding field and did
not separately retain rawPS text or the action's historical input SHA. These
are disclosed limitations; activity FULL is not kernel-death proof and the
packet does not retrospectively qualify Native14. The ordinary authorization
remains SAME16MiB once, time-only through10:00UTC, without an execution lease.

Notes remains IN_PROGRESS. A prospective guardian self-publication fix needs
distinct bounded source validation and a new actual target offer if needed.
No closed offer, old green test, missing operand or incompatible persistent-goal
metadata is reset or falsely completed. Product, PG/native execution and real
vertical-flow acceptance remain open with the previously documented limitations.


## Prospective ordinary identity and guardian publication repair

The original ordinary entry, v2 library, PID11461 BEGIN/FULL and frozen actual
Cargo04 controls remain unchanged. New private entry `ordinary-preparation-v3.py`,
SHA256 `ca1c146d7ec55c3dcb6434298dcf1cea67086a21294e05bf506183797a86668c`,
uses profile `N213-ORDINARY-SELF-V3-20261105`. Before its action it captures
precise actual self kernel birth/image, PPID/group, cwd and entry hash, explicit
JSON argv/digest domains, the original interpreter argv and its separate digest,
and an exact argv-tail relationship. Action hash, immutable input contract and
SAME16MiB book snapshot are bound in BEGIN. The canonical owner record retains
actual finite Cargo04 FULL alongside the current ordinary activity/observation.
Activity FULL remains distinct from kernel death; no PID or owner waiver applies.

The new producer naturally ran necessary source preparation at actual PID54603,
kernel birth1793859287.986037: BEGIN1793859288.15765 SHA256
`420852bdb7a868361cddb97963d3d3bace160475ee817d5e1375b4444089bfb3`,
activity FULL1793859288.163696 SHA256
`7fa97407e720c21003957fb77a910d25db725ed2410514b73c5d1fc5d0c3e451`,
under `ordinary-v3-history/54603-1793859287-986037/`. Those exact facts were
sent directly to1eed and21d for prospective reader binding. This is a naturally
executed ordinary metadata producer, not a new allocated test/browser/app round
or retrospective Native14 validation. No old rawPS receipt is relabeled.

`cargo-next-guardian-selfpub` contains a source-only prospective repair copied
from the verified frozen actual Cargo04 controls. Only receipts.py, run.py and
guardian.py change: the guardian first publishes its own exact entry/kernel/cwd/
argv facts; the parent waits within a bounded startup deadline, binds that exact
owned child and records its actual before/cwd/after operands. Stable mismatches
refuse; at most three unstable binding samples are allowed. Original interpreter
entry is distinguished from transient runpy argv. The unchanged six other
control files are exact frozen copies. No new target offer exists and no changed
control test, initial LIVE pair, compiler or application workload has run.
Distinct bounded source validation was requested; Cargo04 remains closed.
The SAME ordinary16MiB book, original family charges and all higher floors remain.
