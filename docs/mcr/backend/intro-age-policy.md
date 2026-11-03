# BE25 introduction and age policy

## Delivery boundary

Task: **BE25**, pure rules and deterministic port fixtures. Base:
`5123972aac36c46466ecf1dcbc945e23a7c4d719`, branch `codex/mcr-be-BE25`.
Implementation and tests are in the isolated `be-intro-age-policy` worktree.
Status: **IN_PROGRESS** pending shared-target compilation and test evidence.
This document does not accept BE26, BE27, IN03, a provider flow, or release readiness.

Owned destinations:

- `backend/crates/concert-domain/src/age_policy/{mod,arithmetic}.rs`
- `backend/crates/concert-domain/src/intro_policy/{mod,contact}.rs`
- `backend/crates/concert-application/src/age_policy/mod.rs`
- `backend/crates/concert-application/src/intro_policy/mod.rs`
- Each crate's dedicated `tests/intro_age_policy.rs`
- This document

The integrator separately delegated exactly the `pub mod age_policy;` and
`pub mod intro_policy;` exports in both crate roots. Those exports are committed
separately. Other root composition, contracts, manifests, lockfiles, service
adapters, frontend files and source archives are outside this patch.

## Source and contract provenance

Read-only source: `julymeltdown/bigyeting` at
`5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`, inspected at
`/Users/lhs/Desktop/mcr-sources/bigyeting`. C01 audit inventory:
`docs/mcr/backend/source-inventory.json`; decision context:
`docs/mcr/backend/source-audit.md` and `docs/mcr/source-map.md`.
The source checkout is not a delivery destination. No rendering, images, source
identity integration, provider client, SQL query, or source configuration is imported.
No additional upstream licensing conclusion is made by this internal policy port.

All source paths below are relative to `backend/services/bigyeting-api/`.

| Source path | SHA-256 | Destination / use |
| --- | --- | --- |
| `src/domain/privacy.rs` | `f2aad647d8509b6a1e74c0b1c54ad783783dca0533df42ede668c68ad78cb16d` | Adapted pure parser, birthday and age arithmetic in domain `age_policy/arithmetic.rs` |
| `src/adapters/out/repository/current_age.rs` | `74df0f4f60a9ce262aeec2af6a33deb897279889634cb14ed6cfe74579e11a50` | Canonical ordering and current-age semantics; SQL replaced by domain evaluation and claim port |
| `src/adapters/out/repository/naver_verification_state.rs` | `ddea01ac7868e47fdd3557e7ce2f1f9910dce5d238c2374050243448a1ec82c2` | Verification/lifetime provenance audit; no callback implementation copied |
| `src/application/use_cases/social_intro_policy.rs` | `9a28d746c7b646a6039c2a645a76caa163775d6a811f6926d73c0fc1cfb47770` | Reviewed publication/contact validation semantics; no source UI fallback copied |
| `src/application/use_cases/social_intro_contact_policy.rs` | `e9cdc345e1da02c6eca667cfbc6d56d9977267332c654cdb754852bfa692b766` | Adapted contact normalization/detection in domain `intro_policy/contact.rs` |
| `src/application/dto/api/exact_age_values.rs` | `5e91aa55061461451daeeff12de5c10a4a2b7183284dc9075eb300d079c81b74` | Reviewed paired-bound validation and private/public separation |

C04 contract inputs at the task base:

| Input | SHA-256 |
| --- | --- |
| `contracts/domain/v1/common.schema.json` | `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |
| `contracts/domain/v1/social.schema.json` | `b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |

The patch uses existing `chrono`, `serde`, `uuid`, `thiserror`, and
`async-trait` dependencies; dedicated application tests use existing `tokio`.
Domain serialization tests use existing `serde_json`. No manifest or retained
lockfile resolution changes. No database, browser, provider, framework or transport
imports enter these domain/application modules.

## Deliberate adaptations

Canonical `AuthUserId` and `EventId` replace Bigyeting member/provider/X identity
coupling. Typed values and request bindings alone are never authority. Real adapters
must attest current actor/session, event, resource revision, membership, restrictions
and both block directions; unavailable adapters return an error. Default
`UnavailableClaims` and `UnavailableIntroAuthority` implement that behavior.
Test-only authorities do not certify real sessions or multiuser operation.

C04 is authoritative where older source SQL allowed one nullable bound or required
NAVER/X integration even for unbounded introduction discovery. Bounds require both
keys, both null or both numeric, `14 <= min <= 100`, `15 <= max <= 101`, and
`min < max`. Discovery uses `min <= age < max`; unknown age alone does not block
unbounded discovery or basic world eligibility. Pure sharing predicates require
verified age even with unbounded bounds. This patch provides no world-admission or
sharing command. Delivery was informed of this contract adaptation.

Select a canonical claim by descending `(verified_at, expires_at, claim_id)`
**before** validity checking; revoked, replaced, unsupported or expired newest rows
never fall back to older valid rows. Duplicate claim IDs and cross-owner inputs
fail closed. Valid verified input has a nonfuture verification time, positive
lifetime of at most 90 days, supported stored age, strict next birthday, and
nonempty bounded verifier/source provenance. These fields must come from the
owning verifier adapter, never client assertions.

Age arithmetic retains strict ASCII birth components, calendar validation,
Asia/Seoul date rollover, next birthday strictly after verification, and February
29 becoming February 28 in non-leap years. Supported current age is 14..100;
a 100-to-101 transition becomes unsupported. Exact expiry is invalid. Current
observations end at the earliest owner deadline, request deadline, claim expiry,
or next Seoul birthday. The arithmetic helper itself is not proof of verification
and its one-transition calculation is only applicable to the bounded lifetime.

The contact detector retains source Unicode/whitespace/bracket/zero-width folding,
handle, known contact domain, phone and contact-keyword detection. Original
non-English detection literals are retained as source data. No source title,
media, tag, request-mode or generated fallback UI requirement is introduced.
This heuristic is not a comprehensive contact classifier.

## Read ports and projection safety

`ClaimReader` returns an actor/event/session/subject/request-bound observation and
`ClaimLease`. `IntroAuthority` returns a `ReadLease` bound to the exact event and
collection or intro ID/owner/revision/read-kind query. Their real implementations
must preserve authority stability through projection, including claim replacement
and relationship/restriction changes. A timestamp or old authorization receipt is
not a lease implementation. No production authority adapter is supplied here.
No method is an authorization fence for a later remote mutation.

Direct and list filtering use the same introduction checks. List reads authorize
the collection even when empty, retain every row/claim lease through the final
page check, and omit rows denied at that check. Authority unavailability aborts
the page. Lists are bounded to 100 input records and duplicate candidate IDs fail closed. The caller's owning repository
must provide a consistent event-bound candidate page and preserve pagination;
this pure filter is not a database list endpoint or cursor implementation.

Own reads require canonical actor ownership and fresh authority. Own age status
exposes only C04 user/event IDs, opaque status, timestamps and verifier availability.
A temporarily unavailable initiation provider does not erase an existing qualified,
unexpired claim; no claim plus unavailable initiation yields `PROVIDER_UNAVAILABLE`.
Missing authority is an error rather than fabricated provider capability.

`OwnerClaim`, `ExactAgeClaim`, `AgeAssessment` and internal introduction records do
not implement `Debug` or public serialization. Error text never echoes age or raw
birth/provider input. Owner introduction projection includes private bounds and
expiry; the public content fragment has an explicit serialized allowlist without
bounds, expiry, claim IDs, verifier data or exact age.

`PublicIntroContent` is a **fragment**, not a complete C04 `IntroPublic` DTO.
BE27's transport composer must resolve the canonical author's real C03 public
profile (including qualified avatar) and attach `author`; no profile/art is
fabricated here. Projection mappers alone do not authorize reads. Application age
status likewise exposes safe getters for a future transport mapper rather than
adding a transport dependency.

Publication input validation checks IDs, text bounds, draft emptiness, contact
bypass and optional future expiry within event cutoff. BE27 must combine it with
fresh owner/feature/eligibility/claim policy and transactional expected-revision
checks. This task does not implement mutation persistence, receipts, policy-edit
request/grant invalidation, callback state, provider verification or RPC routes.

## Validation and limitations

- Planning/provenance: `python3 scripts/mcr/check-plan.py` — exit 0, PASS,
  including all 13 original archive checksums. This is not application acceptance.
- Standalone formatting: pinned `rustfmt +1.88.0 --edition 2021` on the four owned
  module roots (including their children) and two dedicated tests — exit 0.
  Ambient `stable` rustfmt fails loading its toolchain dylib; pinned 1.88 is used.
- Cargo test/check: **NOT_RUN**, waiting for the shared-target producer lease.
  No duplicate target or producer is started while waiting.

Dedicated domain fixtures cover paired/missing/null bounds; 24/25/29/30 half-open
limits; unknown discovery/sharing; before/on/after birthdays; leap/non-leap years;
strict parser failures; 14/100 support; Seoul midnight/New Year; exact expiry;
canonical ordering before validity; malformed/cross-owner/duplicate inputs;
100-to-101 transition; publication state/expiry/event; private/public field
allowlists; and source contact obfuscation.

Dedicated application fixtures cover missing ports including empty pages;
matching direct/list age decisions; both block directions, restrictions,
membership, session and revision denial; owner/event binding; stale/mismatched/
future/expired observations and deadlines; unbounded discovery without provider
access; safe own status; earlier list-row block, claim revocation and Seoul
birthday changes while reading later rows; nil identities; exact read-query binding; deadlines crossed during later reads;
and duplicate/oversized candidate pages.

Real verifier/callback/persistence/PostgreSQL, transport producer/consumer,
real multiuser races, browser/device/load and release gates are **NOT_RUN** and
outside BE25. Source upstream tests were inspected for semantics, not executed.
The task's deterministic fixtures cannot substitute for those later gates.

## Checkpoint ancestry

- `6bb5053`: audited pure modules, deterministic fixtures and initial provenance;
  Cargo evidence explicitly pending.
- `dbd170b`: the two exact delegated exports in each crate root, isolated from
  implementation changes. Apply after the source checkpoint, never by itself.
- Subsequent validation/fix commits will record the executed commands and results
  before the final integration handoff. No push to master is performed by this worker.
