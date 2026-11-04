# BE-MEETING-PHOTO execution evidence

Original scope: optional authorized physical-beacon photo, BE09 / FE20 / §5.4.
Status: IN_PROGRESS; frozen contract and bounded photo adapter are native-qualified; final publication is being prepared. Historical checkpoints below retain their original evidence limits.
No full BE09/FE20/IN02 or giveaway-BE20 completion is asserted.

Checkout `/Users/lhs/Desktop/mcr-worktrees/be-meeting-photo`, branch
`codex/mcr-be-meeting-photo`; fetched origin/master base
`0c76e42790106501f3988fe7cb46113b8cd7784e`. Prior photo proposal preserved clean
at `1fa906236c3ad0104b01554fb007f77e36b82d87` in the old be-beacon-photo checkout;
no writes continue there. Earlier completed meeting scope/native18PASS remains
historical and does not qualify this photo implementation.

Actual get_goal returned ACTIVE for the aligned unfinished photo objective.
It retains historical old-checkout/BE20 wording; no goal was falsely completed or
replaced. Current authorized manual scope is this new checkout, contract plus
implementation/validation/documentation/direct non-force publication and fixes.
No token budget is assigned.

Owned paths: new `src/meeting_photo/**`, `tests/meeting_photo*.rs` and new
`tests/meeting_photo_support/**` under Concert service; new
`backend/scripts/mcr/meeting_photo*.py`; exact new shared contract document
`backend/docs/api/meeting-photo-contract.md`; and this evidence file. Initial
filesystem checks found all named new paths absent. Existing media, old beacon,
domain schemas, roots, migrations, manifests and locks remain separately owned.

The concrete contract and fixture handoff uses dedicated BEACON uploads, nullable
meeting.media_id, separate photo-permission acknowledgement, existing idempotency
operations and protected metadata/content reads. Direct owner agreement and exact
shared dependencies are tracked in the contract document, not a king/SL review
queue. Both current Cargo targets are occupied; no target, install, third build
cache or cleanup was used. Native and live HTTP evidence remains NOT_RUN.

Media213 confirmed exact wire names and owns BE24 target-prelock/read-in/erase
ports plus the Purpose enum (root-coordinated). FE1ab7 is now explicitly activated
and has received the exact contract/fixtures directly. The publication below is
a contract/fixture handoff, not the missing generated-schema/HTTP implementation.
Fixture JSON identifier/actor/event/media/expiry/READY binding checks, planning
check and whitespace check exit 0. No Cargo or native test has run for this slice.
New owned strict DTO and pure-test work is preserved uncommitted separately while
this documentation/fixture checkpoint publishes; it is not included in the claim.

## Frozen input contract

FE1ab7 and media213 explicitly agreed the exact fields, nullable/full-replacement
semantics, retained-photo acknowledgement and receipt/private-read lifecycle. The
frozen marker is `meeting-photo/v1`; it is an opt-in client input contract, not
live producer capability. BackendSL explicitly allocated only beacon.schema.json
and feed.schema.json additions plus reserved forward migration 0029 to this owner;
media213 owns Purpose enum. Root owns OpenAPI output and FE1ab7 owns the exact generated TypeScript output; the historical 230 assignment is superseded.

`python3 backend/scripts/mcr/meeting_photo_contract.py` exits 0: 17 offline schema
cases cover fixtures, dedicated purpose, attachment permission, omitted/null
removal, text-only compatibility and invalid remote-photo fields. Python syntax
and whitespace checks exit 0. No generated output, application build or native
execution is claimed. The reserved migration is being implemented separately and
is not included in the schema/fixture publication.

## Producer source checkpoint (native pending)

Root explicitly granted visibility-only `BeaconProducer`/four fields in
`meeting/commands.rs` and one `pub mod meeting_photo;` export in Concert lib.rs.
A subsequent exact grant covers only the owner Update authorization correction:
stale stored party Read no longer gates an explicit owner replacement; existing
actor/owner checks, stable old/new roots, new Update policy, destination checks,
CAS and ordinary stored-fence reads remain. Targeted photo regression exercises
old-read denial, explicit retained-image reconfirmation at revision two and later
place closure/removal/revoke. No other existing producer logic is assigned.

The new module reuses that actual BeaconProducer and BE24 transaction ports. Full
photo input participates in the BE15 fingerprint; old text-only serialization is
unchanged. Old/new media are locked in sorted order before global receipt budget.
Effect, exact revision/expiry/permission association and irreversible former-image
erasure share the same SQL transaction. Linked reads prelock current beacon/
audience/place authority before media locks, recheck stored linkage and final
expiry/proof, and return only existing private no-store sanitized image types.

`CanonicalPhotoGuard::from_request` consumes an actual SDK OwnerGuardScope and
seals the full verified frame, including key and typed intent, alongside the raw
HTTP binding. It registers the actual connection before effects and captures the
real held actor security generation with existing event/admission/participant
locks. No default success fence, cached observation grant or release-on-return is
provided. Root retains the actual scope through durable SQL outcome and qualified
Gateway HTTP terminal settlement. The closed MEETING_PHOTO reference includes its
full frame digest; root settlement dispatch for this reference and public product
routes remain explicit composition work, not enabled by the module export.

The forward migration 0029 extends purpose and adds a one-photo-per-beacon
association. Deferred SQL checks bind actual owner/event/READY target/media and
beacon revisions, expiry and any pinned policy deadline. Existing 0008 and 0012
are untouched. The owned socket-only SQL fixture passed migration application,
five negative binding/proof cases, a real restart and marker/inode cleanup
(`/tmp/mcr-meeting-photo-sql-b.json`, exit 0). Schema-only SQL data is explicitly
synthetic and not a decoded/authorized production image.

Authorized dependency copies: media owner's `d4ce03b1` -> local `ab9af08e` and
`65f90e08` -> local `b54abe4c`. A pure-doc cherry-pick conflict preserved master
and only the incoming meeting-photo port section; no dependency source behavior
was edited. That owner's unrelated unpublished canonical/v2 preparation was not
imported. No dependency build/live claim is inferred from these source copies.

Owned native sources cover real codec/upload plus attach/replay/current privacy,
replacement/removal/revoke, wrong owner/purpose/non-READY IDs, competing revision
updates, forced late erasure rollback, root/media/budget contention, restart,
expiry/cleanup, party generation/place closure/removal, and SQLx fresh/upgrade/
preserved old media/checksum/replay. Native tests are NOT_RUN until the actual
shared target is leased and exact binaries are executed. FixtureGuard is synthetic
remote owner proof only; production SDK/RPC/HTTP/device/rights and IN02 remain
unqualified. Earlier reports are not photo evidence.

Request-frame hardening additionally checks closed typed intent against the raw
body before sealing, validates JSON content type, and binds target/method/key/
content-type/body digest plus parsed intent in the request-specific guard digest.
Metadata/content methods compare the actual media ID to the sealed HTTP target;
argument substitution cannot fetch another current photo under an old URL.
The producer source checkpoint is `cd4b7362`; later hardening is tested separately
before publication. No source checkpoint alone is native qualification.


## Current composition dependencies

The checkout reconciles actual SDK/v2 and published owner-store source via merge
`562334f306b12e5d88aaa46987d9f95bcc692965`. Root OpenAPI projections are published
at `b9423cabc112da5605eb7f4393adfdee5c93d767`; FE20 optional-client source is
`7e41574a`, shared BEACON support `591040ff`, and FE-owned generated projection
`65f91166`, delivered through `6c908472ec3b6025bcdca9a90eae4db26c90a0b4`.
Those owner-reported client checks use deterministic HTTP persistence and do not
establish actual backend durability, authorization, atomic erasure or IN02.
No generated, frontend, source archive or root-composition files were authored by
this worker. Actual Cargo/native qualification is still pending the resource
queue, with three test targets ready. Additional security regressions bind the
actual raw request/type/key and prevent a retained old content URL from being
substituted with a newly attached image argument.


## Qualified native checkpoint

Implementation/hardening is committed through `b41cf49bd65f9d6c678c194d744a5322c9facd1e`,
after peer reconciliation `c1cf15a2c18b0a5dfe224597eb3a795241f3b814`. The explicit
MEDIA lease was accepted only after 65.25 GiB free-space and process preflight.
Build command (exit 0):

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml -p concert-service --test meeting_photo_contract --test meeting_photo --test meeting_photo_migrations --no-run --locked --offline --message-format=json
```

The first attempt exposed a stale contracts-build helper path from the prior
worktree, corrected by refreshing its mtime without source changes. The second
reported an owned JSON macro fixture syntax error, corrected in `b41cf49b`.
The third compile succeeded. Actual SDK/v2 and current imported Post IDL compiled;
no unrelated generation fix or full-wave barrier was required for this slice.
All three artifacts, Cargo JSON and 1,714 source pins were frozen/hash-checked in
`/tmp/mcr-meeting-photo-native-c1cf15a2`; MEDIA was explicitly released before PG.
No second target, install, target deletion or global cleanup was used.

Native command (exit 0), solely from immutable copied artifacts:

```sh
python3 backend/scripts/mcr/meeting_photo_smoke.py --binary /tmp/mcr-meeting-photo-native-c1cf15a2/meeting_photo_contract --binary /tmp/mcr-meeting-photo-native-c1cf15a2/meeting_photo --binary /tmp/mcr-meeting-photo-native-c1cf15a2/meeting_photo_migrations --report /tmp/mcr-meeting-photo-native-c1cf15a2/native-a.json
```

PostgreSQL 16.14: **6 contract + 8 producer + 1 SQLx migration test PASS before
and after a real process restart**, 30 executions, zero ignored/failed. All five
SQL binding/proof negative cases, migration application, persisted association
snapshot and marker/inode-owned cluster cleanup PASS. The committed durable
receipt is `backend/services/concert-service/tests/meeting_photo_support/native-evidence.json`.

The producer tests executed real bounded image decoding/upload/finalize,
owner/event/purpose/READY checks, private sanitized bytes and no-store headers,
current audience/block/outage/place authorization, retained-photo explicit
reconfirmation, same-key safe replay, replace/remove/revoke byte erasure,
competing CAS updates, forced late erase failure with complete transaction and
receipt rollback, actual blocked-reader root/media/budget ordering, expiry
cleanup, and restart receipt/content persistence. SQLx fresh installation and
pre-0029 upgrade preserved real existing POST media byte-for-byte at row level;
ledger replay/checksums and immutable published 0008/0012 hashes passed.

The imported BE24 helper and Purpose source blobs are byte-identical to media
owner's qualified `c32f4c74` for the exact two source paths, independently checked
with `git diff`. This worker's real photo composition now also qualifies those
ports; it does not claim the owner's separate canonical adapter or live RPC join.
Remote owner proof in these tests remains explicitly synthetic. Actual canonical
Auth/Friend exclusion, MEETING_PHOTO settlement dispatch, qualified HTTP delivery,
root routing/capability activation, device flow, production photo rights and IN02
remain NOT_RUN here. No full BE09/FE20/IN02 acceptance or scope cut is inferred.

The actual persistent-goal API still returned BLOCKED after explicit assignment
resumption; its available mutation API cannot set ACTIVE. This administrative
state was reported to BackendSL while the complete authorized work resumed; no
replacement goal or false completion was used to bypass it.
