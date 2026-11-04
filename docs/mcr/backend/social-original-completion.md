# Original Social completion: BE28, BE29, BE30 and BE37

Status: **IN_PROGRESS**. This is an original-requirement reconciliation and a
runtime dependency handoff, not acceptance of the four tasks or a new test run.
The specification rows in `MCR_IMPLEMENTATION_PLAN_V2_EN.md` remain authoritative.

## Source disposition

The resumed owner is developer16e, on isolated branch
`codex/mcr-social-original-completion`, retained base
`2ce48fa49046ab208705fbcf1ed837d368dd6390`. The examined published reference is
`5f9f2fc773905011447faedf22125610b007e19a`. This patch contains documentation
only; it must be applied as its exact two-file delta, without importing the
retained branch's unpublished BE39 ancestry.

BE28's implementation is retained in the domain's `relationship/social.rs`,
Concert `relationship/social.rs`, `relationship/crypto.rs`, and
`social_live/keys.rs`. The private handle type has no public serialization and
redacts Debug output. Normalization precedes AES-256-GCM encryption with fresh
nonces and event/owner/platform/version/key-bound associated data. Managed key
provenance is explicit. Replacement and removal revoke old request/grant
bindings. No new source gap was found in these examined paths. Actual serving
DTO, headers and logs still need the current real-stack privacy observations.

BE29's producer retains explicit requester consent, recipient approval/rejection,
per-platform bindings and exact reciprocal grants. The repository rechecks both
handle versions, request revision/state, expiry and current relationship policy;
the common command repository supplies idempotency. No new create/savepoint or
decision implementation is justified by this reconciliation. Current genuine
multiuser HTTP execution remains required.

BE30's reveal path checks actor/peer, current eligibility, approved request and
platform, both current handle versions, both exact reciprocal grants, expiry and
a final database-clock authority fence. `social_live/read_child.rs` retains the
original scope and actual locks through the owning HTTP delivery lifecycle.
`producer.rs`, `witness.rs` and `invalidation.rs` retain native transaction
correlation, durable invalidation generations and restartable expiry delivery.
Current mounted no-store, safe-error, WebSocket and private-cache behavior is
still **NOT_RUN** for this resumed assignment.

BE37's retained canonical scope combines both Friend block directions with
current Auth/session/restriction authority. Missing authority fails closed.
Unblock cannot resurrect grants invalidated by an earlier block. Component
coverage includes revocation/outage/expiry; current HTTP/WS/projection convergence
and actual moderation/suspension owner integration remain open. The old worktree
also contains unpublished `social_live/moderation_revocation.rs`, its module
export and a `publish_moderation_in` helper. They are not silently promoted as
qualified BE37 completion, and none is included in this handoff.

The accompanying `../evidence/be28-be30-be37/source-and-evidence.json` records
twelve examined source hashes: eleven match the published reference; the sole
`invalidation.rs` difference is the retained unpublished moderation helper.
This limited readset does not certify every application path. Historical
Sharing lives in `social_live/account_deletion.rs`; no `sharing_live/` directory
was created. Auth deletion/configuration, BE33 notification transport and shared
contracts remain with their respective owners.

## Reused validation, with its limits

The original commands, exit codes, source revisions, SQL prerequisites and
artifacts remain in the existing owner records; this patch does not replay them.

- `social-remaining.md`, “Qualified invalidation source”: source
  `beeb34941f3da4bed3826dd388d59ebc781b136c`, including create receipt correction
  `08602903`. Locked/offline Rust 1.88 compilation exited 0; copied execution
  exited 0 with 10 pure and 80 native executions across physical PostgreSQL
  restart. These were component tests with their recorded authority fixtures.
  Two original temporary JSON receipts are now absent; this handoff preserves
  their historical qualification record and does not claim to have rehashed them.
- `social-policy-runtime.md`, “Corrected full cohort”, and
  `sharing-deletion-runtime.md`: source
  `46defd2346e2e73641c552b408623ce357500c15`. The corrected full 47-migration
  cohort exited 0, including kernel, delivery, factory, restart and focused
  Sharing boundaries. The Social, Sharing and full-release JSON files were
  rehashed during this reconciliation; their digests are in the companion JSON.
  Parked downstream saga targets and absent real multiuser WS are not acceptance.
- `social-moderation-revocation.md`, copied-runtime section: compiled source
  `cfc49f4eec500d13ce62dd200ccf3efe6b8b1b67`, control source
  `30732c8d1db7d8e45ab47c44ff389e7e82adf017`. Recorded copied denials,
  protocol/migration checks and actual production-main migration verification
  passed; its native report was rehashed. This historical executable is not a
  qualified current Root serving bundle and does not qualify moderation effects.

No new Cargo, PostgreSQL, browser, target or test process ran for this document.
The documentation check is `git diff --check`; the evidence JSON is parsed and
its stated source/receipt hashes checked by the implementation owner. Those
checks establish this handoff's consistency, not product acceptance.

## Actual runtime prerequisites and next owners

Root1fc owns the current seven-service serving profile, canonical composition,
executable qualification and private principal window. At this reconciliation,
the reported earlier Root build failed with no executable; the repaired successor
is queued. No current READY descriptor was supplied. The existing historical
copied Concert binary must not substitute for it.

Auth207 supplies the current Enrollment helper
`backend/scripts/mcr/session_deletion_runtime/enrollment.py`, source digest
`c4a328cdbb220202ef90502324f9873da5012e045b4f1e8b22d6ba9843760dc9`.
`owned_auth_adapters(...)` supplies MailSink, principal and provenance;
`principal(stack, browser, ordinal, *, deadline=None, cancel_event=None,
signup_gate=None)` returns the canonical UUID, private credential path and
RAM-only token after actual signup/mail/verification/login/admission. It has no
negative-claim mode and creates no verified positive-age provider claim.

Root1fc and World20f route the real private descriptor directly once available:
the existing event, requester/recipient/restricted canonical principals, actual
helper/binary revisions, serving origin, expiry and world WebSocket URL. The
control window is not Social admission authority. The existing event must be
staged with its real admission, introduction/consent and claim facts, coordinated
with its exclusive runtime owner; standalone reseeding would create another
database and is not a substitute. No credentials or bearers belong in this record.

Developer16e then runs the applicable current owner-controlled Social flow within
an allocated finite validation round, fixes genuine failures in owned leaves,
and supplies actual HTTP safe-error/no-store, revocation/expiry and WS/cache
observations. Real external age claims constrain the affected positive scenarios
only. Claim absence must be confirmed by the serving authority; omitted helper
arguments do not establish it. Source work and independent negative/revocation/
expiry scenarios do not wait for a positive-age provider.

BE33 owner6ec9 owns actual notification delivery; its positive transport is not
invented by a Social receipt or outbox assertion. Shared Root/Gateway and current
moderation integration stay with their assigned implementation counterparts.
Technical interfaces and failures are resolved directly by these developers.

Direct publisher412e receives this exact committed documentation delta. No king,
SL, review, acknowledgment or ledger update gates publication or the next ready
assignment. Original tasks remain IN_PROGRESS until their real behavior and
required runtime evidence are delivered. The aligned persistent Social goal
remains unfinished: its API still reports `blocked` despite the human's resumption,
and the available status API cannot resume it; it was neither reset nor falsely
completed. Frozen BE38 `2acfba0b032be45272c895a2126c6bce28c64658`, its
worktree, readset, queue and qualified resource slot are unchanged.
