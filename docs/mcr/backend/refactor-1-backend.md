# Refactoring track 1 — backend

Status: **INTEGRATED**. Branch `codex/yc-refactor-1`, base `62a31f104`.

Eleven work packages landed in parallel over one session. Each was correct in
isolation; the merged tree carried duplication and drift that no single package
owned. This pass removes it **without changing behavior**: every test count is
identical before and after, and the two verification scripts produce
byte-identical schema hashes.

## 1. What was consolidated

| # | Duplication | Where it lived | Now |
| --- | --- | --- | --- |
| 1 | The private-cluster harness | 5 scripts, ~700 lines of the same four steps | `backend/scripts/mcr/wp_cluster_harness.py` (239 lines) |
| 2 | `profile_media` row decoding | 3 copies with 3 different defaulting rules | `services/concert-service/src/profile_media/rows.rs` (66 lines) |
| 3 | The `MediaPurpose` enum | A second copy of `media::Purpose` | Re-export of `media::Purpose` |
| 4 | `names_event_path` / `names_intro_path` | 3 byte-identical copies | `services/concert-service/src/profile_http.rs` (58 lines) |
| 5 | Error-reason drift | 2 reasons outside the contract's closed set | Both project to `INVALID_INPUT` |

Net: **−595 / +211** across 17 files, plus 363 lines of new shared modules.

## 2. The harness consolidation

`wp02_migration_ledger.py`, `wp04b_identity_cluster.py`,
`wp05_intro_cluster.py`, `wp06_profile_posts_cluster.py`,
`wp07_room_cluster.py` and `wp08_avatar_cluster.py` each reimplemented: resolve
the PostgreSQL tools, create the marker-owned cluster, create a database, apply
the forward migrations, write a report, run commands, clean up.

The copies had **drifted**, which is the real cost:

* `wp06` and `wp07` applied a duplicate migration number in order; `wp08` and
  `wp02` refused it. The same repository state was accepted by two scripts and
  rejected by two others.
* `wp06` wrote `run_cmd`/`run_cmd_exit_code`; `wp07` wrote `runs`; `wp05` wrote
  `tests`. Three report shapes for one concept.
* `wp06` printed a traceback on an unexpected exception; `wp07` and `wp08` did
  not.
* `wp06` and `wp05` defaulted `--pg-bin-dir` to PostgreSQL 16; `wp07`, `wp08` and
  `wp04b` defaulted to `/opt/homebrew/bin` (PostgreSQL 17).

`wp_cluster_harness.py` now owns the four steps and the report keys. Each script
keeps **its own CLI and its own report scope**, so every evidence document that
cites a command line stays valid. The duplicate-number policy is a parameter
because the two packages that met the `0086` collision made different, recorded
choices; the defaults are preserved per script.

`wp02_migration_ledger.py` and `wp04b_identity_cluster.py` have genuinely
different shapes (a two-scenario ledger check; a start/stop state file), so only
their shared mechanics moved — the tool resolution, the migration manifest, the
ledger table, the apply-with-receipt step and the report/cleanup discipline.

## 3. The attachment-rule consolidation

Three call sites read the same four `profile_media` columns and decoded them
three different ways:

| Call site | Unparseable purpose | Missing state |
| --- | --- | --- |
| `intro_posts/attachments.rs` | defaulted to `ProfilePost` | `READY` if equal |
| `intro_posts/commands.rs` | defaulted to `ProfilePost` | defaulted to `DELETED` |
| `profile_posts/commands.rs` | error | error |

A **defaulted purpose is the dangerous one**: the purpose is what decides whether
an asset may cross from the intro surface to the profile surface (V-06-d). A
decoder that guesses `ProfilePost` for an unreadable row would let a
mis-associated asset be attached where it does not belong.

`profile_media::rows::candidates_in` is now the one decoder, and it **defaults
nothing that matters**: a row whose identity or purpose cannot be read is not a
candidate, which is exactly how an absent asset behaves. The pure rule in
`concert_application::intro_posts::attachments` still makes every decision; only
the row→facts step is shared.

`MediaPurpose` was a second copy of `concert_application::media::Purpose` with
its own `parse`. It is now a re-export, so a purpose added to the media domain
appears in the attachment rules automatically.

## 4. The parser question

The task asked me to extract the closed-parser shape "if it genuinely reduces
risk" and not to force an abstraction that makes each parser harder to read.

**I did not extract the parsers.** `profile_posts/http.rs` (764 lines),
`profile_room/http.rs` (272), `intro_posts/http.rs` (343) and
`profile_product/targets.rs` (430) share a *shape* — parse `(method, target)` into
a closed enum — but their bodies differ substantively:

* they return different error types (`RouteRefusal`, `RoomRefusal`, `tonic::Status`);
* their path grammars differ (flat table, prefix/suffix, nested segments,
  query-bearing);
* their query rules differ (none, page query, revision query, catalog query).

A generic `parse_closed<T>(method, target, table) -> Result<T, E>` would need a
trait for each of those axes and would move each parser's actual logic further
from its route table. That is the abstraction the task warns against.

What *did* reduce risk, and what I extracted, is the part that was genuinely
identical and security-relevant: the **target-level guards**
(`names_event_path`, `names_intro_path`, the 4 KiB/control-character shape check).
A guard that differs by one spelling between two surfaces is a hole in one of
them, and three byte-identical copies is three chances to drift. They now live in
`profile_http.rs` with one definition each.

## 5. Error mapping

Two reasons were outside the operation register's closed error set:

| Reason | Where | Contract |
| --- | --- | --- |
| `INVALID_PURPOSE` | `ProfileMediaError::PurposeMismatch`, `parse_init_upload` | not declared |
| `BYTES_MISMATCH` | `ProfileMediaError::BytesMismatch` | not declared |

The gateway refuses to build a response for an undeclared status, and the client's
error decoder only accepts declared reasons, so a reason outside the set turns a
precise refusal into an opaque one. Both are *client input* errors and now project
to `INVALID_INPUT`, which every profile media operation declares.

The status codes were already consistent across the profile surfaces (400 invalid,
403 denied, 404 unavailable, 409 conflict, 413 too large, 415 unsupported, 503
unavailable); only the reason strings drifted.

## 6. Dead code

* Removed a dead `current` variable and an unused `serde_json::Deserializer` in
  `profile_posts/http.rs::reject_duplicate_keys` — the scanner never used either.
* Removed imports left behind by the consolidation (`AttachmentCandidate`,
  `intro_posts::attachments`).
* **Deliberately kept** `ProfileMediaError::status()`/`reason()`: they look
  uncalled from `profile_media/` but have a real production caller in
  `profile_product/targets.rs::media_status`. This is exactly the case the task
  warned about — "do not remove something merely because you cannot see its
  caller from one crate".
* The many `dead_code` warnings in `tests/**` are pre-existing test-support
  helpers shared across targets; removing them is a separate decision and would
  change no product behavior.

## 7. Verification

| # | Command | Exit |
| --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** |
| 2 | `cd backend && cargo +1.88.0 test --locked -p profile-service -p concert-service -p api-gateway` | **101** — see below |
| 3 | `python3 backend/scripts/mcr/wp02_migration_ledger.py --report /tmp/refactor-ledger.json` | **0** |
| 4 | `bash scripts/mcr/check.sh guards` | **0** |

### Test counts, before and after

| Crate | Before | After |
| --- | --- | --- |
| profile-service | 342 | **342** |
| concert-service | 1449 | **1449** |
| api-gateway | 737 | **737** |
| **total** | **2528** | **2528** |

Identical. (Measured with `cargo test -- --list`, which is independent of fixture
wiring.)

### Why command 2 exits 101

The bare command exits 101 **before and after** this refactor, for the same
reason: most of these suites need a private PostgreSQL cluster and a per-target
`*_TEST_DATABASE_URL`, which the bare command does not provide. On the untouched
base the same command fails 696 tests across 691 distinct names; after the
refactor it fails 697 across 692, and the one difference is
`bootstrap_dispatch::http_ingress::public_http_route_dispatches_fresh_signed_chain_and_never_caches`,
which **also fails on the untouched base** when run in isolation (a timing
assertion: it expects `200`, gets `504`). It passed once in the baseline sweep and
failed once in the after sweep; it is flaky on both trees, not a regression.

The suites this refactor touches all pass with their clusters:

| Suite | Result |
| --- | --- |
| `profile_media_upload` | 16 passed |
| `profile_posts` | 16 passed |
| `profile_post_read` | 11 passed |
| `profile_post_routes` | 14 passed |
| `profile_post_lifecycle` | 4 passed |
| `intro_post_commands` | 11 passed |
| `intro_attachments` | 7 passed |
| `intro_snapshot` | 5 passed |
| `profile_room_routes` | 12 passed |
| `profile_room_visibility` | 8 passed |
| `profile_room_persistence` | 15 passed |
| `profile_avatar_routes` | 13 passed |
| `profile_avatar_persistence` | 13 passed |

The ledger script's four scenarios produce **identical schema hashes, ledger row
counts and applied orders** before and after.

## 8. Not done

* **The parsers themselves are not unified** — see §4 for why.
* The pre-existing `dead_code` warnings in `tests/**` and the pre-existing
  `venue_product.rs` unused imports are untouched: they are not this refactor's
  scope and removing them changes no product behavior.
* `contracts/` is untouched, as required.
