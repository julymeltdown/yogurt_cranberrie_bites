# BE34/BE35 mural implementation and native qualification

The assigned implementation and native PostgreSQL qualification are delivered.
Overall BE34/BE35 product acceptance remains IN_PROGRESS: authenticated public
routing/current production authority and real IN04 are NOT_RUN. This is not a
release or live multiuser acceptance claim.

## Ownership, ancestry and source revisions

Worker: `01a1a21e-ab80-7012-b9fa-cdb69eed097b`. Current isolated checkout is
`/Users/lhs/Desktop/mcr-worktrees/be-mural-resume`, branch
`codex/mcr-be-mural-resume`, published base
`7fc3e38a85a91a99567e62f661869092cb03dc3b`. The former `be-mural` checkout at
`3641622` is read-only. Its unfinished aligned unbudgeted goal was reused.

All mural logic is newly authored. Existing BE15 transaction and isolated
PostgreSQL harness patterns are reused; no legacy rendering or artwork is copied.
`docs/source/`, manifests, locks, contracts, HTTP/proto, frontend, published
migrations and existing world composition are unchanged.

Original author commits, carried with `cherry-pick -x` provenance:

| Original | Resume commit | Content |
|---|---|---|
| `4f412f6` | `1ffa4b0` | Exact mural exports, SafeFailure cases, new receipt migration |
| `d08ceae` | `a71421c3` | Domain/application/SQL/repository/subscription/tests |
| `89e174f` | `f48b4c3f` | Immutable checkpoint test |
| `1ab501c` | `f917990a` | Exact retained own-result envelope |
| `9ddf7cd` | `76a384ce` | Native suite/source evidence guards |
| `3641622` | `801363c3` | Checkpoint ID reuse protection |

`9b307be` completes the migration reservation adaptation. Unpublished original
0013/0014 are now **0019_mural.sql / 0020_mural_receipt_failures.sql**. Existing
0016 and 0018 retain their exact bytes. Reserved future 0015/0017 are absent from
this pinned base and are not edited or fabricated. The full package is versions
1–12,16,18,19,20. The current world/mod.rs is preserved with only the mural
subscription declaration appended; service lib.rs already exports world and
has no change. The exact shared scope is mural declarations in domain/application
lib.rs and repository.rs, the mural_subscription declaration, and four
SafeFailure enum/as_str/parse cases. Implementation/tests/scripts/documentation
remain within the named mural paths, including tests/mural_support/.

Pinned owning inputs:

- Specification §§8.4/BE34/BE35/BE40, SHA256
  `e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36`.
- Initial C04 candidate `b615e35`; own-result amendment `a25b855` corrected by
  `bff7e53`, C04 v0.1.2. Those shared dependencies are already in the published
  base; old local imports `71effa2`/`05e70a5` were deliberately skipped.
- BE15 seam originally `c2a2c30`; safe domain failure/migration0009 pattern
  `9472785`; BE37 relationship-policy foundation `620f8ee`.
- Current owning source, root declarations, all migration bytes, contracts and
  locked dependency inputs are hash-pinned in the committed build/native
  receipts. Existing dependencies only; no installation or lockfile change.

## Atomic write and private result recovery

`PgMuralRepository::place` accepts one explicit confirmed PlacePixel and canonical
actor/event/key. It rejects invalid 128×128 coordinates, 16-color indices, nil
scope/key and unsafe revisions. Preview/dragging has no backend write path.

The trusted MuralAuthority port must resolve current canonical Auth/session,
event membership, feature eligibility, suspension and BE37 policy, retaining
local locks or equivalent external fences through commit. Its unavailable default
denies every operation; no production authority/settings/roles are invented.

The producer locks event and mural before BE15 admission. It enforces OPEN/LIVE,
EDITABLE state, region locks, exact expected revision, and per-user cooldown from
PostgreSQL clock_timestamp after acquiring the aggregate lock. Pixel, monotonically
ordered operation/revision, private author/cooldown, receipt and outbox are one
transaction. Storage/outbox failure rolls everything back, including budget and
cooldown. Same key/body recovers the same operation; changed body conflicts.

Paused/frozen/cooldown/region-lock failures persist exact safe FAILED receipts
without effects/outbox. A replayed cooldown reason has no historical retry timestamp
in the BE15 safe receipt, so it returns Cooldown(None), never invented Retry-After.
Public wire error metadata remains an integration obligation.

`recover(actor,key)` is read-only and freshly authorizes one transaction. It joins
an unexpired retained SUCCEEDED placeConcertPixel receipt for that actor/event/key
and status200 with the original durable operation by event/revision/author.
Receipt resource_id/target_id remain the event mural aggregate UUID; the operation
UUID is separate. The closed OwnPixelResult envelope carries only that verified
receipt_id/key and original PixelOperation, preserving next_allowed_at. Missing,
foreign, expired, retired, pending, failed or inconsistent evidence uniformly
returns ResourceUnavailable; policy/storage outages fail closed. No command is
replayed and no cooldown is spent. Public pixel equality is never acknowledgment.

## Snapshot, delta and bounded subscription

Snapshots hold the aggregate SHARE lock across grid/revision/palette and caller-only
cooldown reads. Public immutable recovery checkpoints contain no author/moderation
or caller cooldown data. Reusing an old checkpoint ID is rejected, and reads verify
that a retained checkpoint exactly matches the selected aggregate revision.
Operator provisioning supplies palette/cooldown; no product defaults are added.

Public deltas explicitly select seven public operation fields in strict order,
bounded to512. Equal revision returns an empty batch. Future/obsolete revision,
retention floor, reset barrier, a gap larger than512, or a missing operation returns
reset_required=true and no operations. Original operation records remain available
for retained receipt recovery independently of public delta retention.

BE40 control/lock/restore must lock this same row, increment revision, advance
reset_revision, allocate a new snapshot ID/time, and never rewind revision or
cooldown. This patch provides those storage boundaries and simulates them in native
tests; BE40 operator commands/audit remain a separate assignment.

MuralSubscription is a pull adapter on the incumbent world connection. Capacity is
explicit (1–16 frames); overflow/gap/reset/epoch change discards obsolete queues and
requires a snapshot. Every dequeue reauthorizes and checks the reset boundary.
A high-water revision prevents rewind across epochs. Revocation/read failure clears
the queue. A failed transport send must invalidate/reconnect; dequeue is not client
acknowledgment. This creates no second socket/server or unallocated C05 message.

HTTP snapshot/delta plus the existing world epoch observer remain the interface.
C04 defines GET /api/concert/events/{eventId}/mural/commands/{key}/result with the
OwnPixelResult envelope. FE33 correlation is event aggregate + receiptID/key +
original event/revision/body; live producer/consumer qualification is NOT_RUN.

## Reproducible evidence

Committed under backend/services/concert-service/tests/mural_support/:

- build-receipt.json: exact Cargo command, exit0, source hashes and copied binary hashes.
- source-provenance.json: equal source hashes before/after, pinned base/settings,
  Cargo output hashes and release-before-PostgreSQL ordering.
- native-evidence.json: result PASS, all16 ledger checksums in three isolated DBs,
  8PG+2subscription before and after restart, direct guards, persistence and cleanup.
- native-path-refusal.json: preserved first-run refusal of a /var versus /private/var
  artifact path alias. No database was started; canonicalizing the same copied paths
  corrected the receipt without changing binary/source content.

Focused compilation (exit0,14.75s), with Rust1.88.0, jobs2, debug0, incremental0,
locked/offline, sole existing target `/Users/lhs/Desktop/mcr-build/backend`:

```sh
rustup run 1.88.0 cargo test --manifest-path backend/Cargo.toml -p concert-service \
  --test mural --test mural_subscription --no-run --locked --offline --message-format=json
```

The exact two Cargo JSON artifacts were copied and hashed before releasing the
exclusive target. PostgreSQL ran afterward only against these private copies:

- mural-fcc6b2e81d8d9d09:
  `8449d35be52ce0ba04b2a9248be2f5403dfd9a24ea8014039449bba63f6dda00`.
- mural_subscription-3a0b152de08f35c2:
  `8c4cdeb25b46d2bc3131990110d8108415a0a0a16ebbc3a36b9ee046c65d1984`.

Native command exit0 (absolute artifact paths and build receipt are recorded in
those committed receipts; `/tmp/mcr-mural-resume-native-02.json` is the original):

```sh
python3 backend/scripts/mcr/mural_smoke.py \
  --binary /private/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-mural-build-wtn0iwdl/mural-fcc6b2e81d8d9d09 \
  --binary /private/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-mural-build-wtn0iwdl/mural_subscription-3a0b152de08f35c2 \
  --build-receipt /private/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-mural-build-wtn0iwdl/build-receipt.json \
  --report /tmp/mcr-mural-resume-native-02.json
```

The harness uses a new marker/inode-owned PostgreSQL16.14 Unix-socket cluster,
no TCP, no inherited database/provider settings, and verified cleanup. Native
Rust SQLx Migrator applies the whole current package, verifies actual SHA384 ledger
receipts, qualifies fresh installation and forward0018→0020 while preserving an
event and preexisting failed receipt, and proves identical receipts on reapply.
After a real PostgreSQL process restart, all three ledgers and mural state hashes
match; a new native process recovers the original operation and cooldown. Each
suite is required complete with zero ignored tests: 20 total passing invocations.

Coverage includes same-actor concurrent cooldown, concurrent distinct actors and
strict revisions, invalid coordinates/colors, key/body conflict, uncertain receipt
states and original envelope binding, forced outbox failure rollback, all four
safe failure replays, privacy/revocation, new/reconnecting convergence, reset/restore,
retention, bounded backpressure, epoch changes and immutable checkpoints.

Other checks, all exit0:

- python3 scripts/mcr/check-plan.py: planning provenance only PASS.
- python3 -m unittest discover -s backend/scripts/mcr -p test_mural_smoke.py: 2 PASS.
- python3 -m unittest discover -s scripts/mcr/tests -p test_domain_recovery_reads.py: 4 PASS.
- Owned Rust rustfmt and git diff --check; source archive/published0016/0018 unchanged.

The abandoned first compile on the old branch failed with ambiguous projection
Result inference; that code was corrected before the qualified resume build.
Old direct-SQL receipts are not used as migration-ledger or native acceptance.
Public routing/current production authority, real browser multiuser IN04, BE40
operator implementation and deployment/load/release qualification remain NOT_RUN.
