# BE27 Product Intro runtime

Status: **IN_PROGRESS**. Source and owned native harness are prepared. Rust compilation, pure/native execution and public HTTP acceptance are **NOT_RUN**. Planning checks do not certify runtime behavior.

Base: `a7f3989d9c40c53a9c2982bced938f8b4853f627`; branch `codex/mcr-be-BE27-product`. The former consumer writer stopped before this isolated checkout began. Changes stay within BE27-PRODUCT ownership: `src/intro_live/**`, needed retained `repository/intro.rs` hooks, only `pub mod intro_live;` in `lib.rs`, exclusively reserved migration 0049, `tests/intro_product.rs`, `tests/intro_product_support/**` and this document. Root owns global Operation/HTTP/main and union resolver composition. No manifest, lockfile, contract, configuration, CI, old migration or `docs/source/` changes. Root explicitly transferred `backend/crates/concert-application/src/intro/mod.rs` for the single additive `IntroError::Expired` variant only, after the base lacked the historical adapter variant. This preserves the frozen 409 IDEMPOTENCY_EXPIRED reason; no broader application rewrite is included.

## Runtime and integration API

`IntroRuntime::new(PgPool, Arc<PublicProfileReader>, IntroCursor, Arc<FingerprintKeys>, Arc<AgeRuntime>)` receives actual typed dependencies. `dispatch_for_scope(Arc<OwnerGuardScope>, proto::ProductRequest) -> Result<ProductRepresentation, tonic::Status>` consumes Root's acquired scope. It implements `ScopedProductProducer`. `protocol::handles_operation(&str)` accepts exactly `listConcertIntros`, `getConcertIntro`, `getOwnConcertIntro`, `setConcertIntro`, `publishConcertIntro`, `closeConcertIntro`. Root must restore the original sealed parent **inside** the supervised future: Tokio spawning does not inherit task-local authority.

The facade compares the full raw request to the original sealed unary message and checks actor/event/family/request/operation/method/target/body/deadline before parsing. Closed command structs require nullable fields and reject duplicate/unknown fields. The retained PgIntro engine owns CAS, receipts, idempotency and BE15 outbox. Production policy registers the actual connection before effects and uses actual canonical Auth/Friend enrollment and current local admission. Public reads use actual held PublicProfile subjects and current authority; restricted reads borrow the same transaction for AgeRuntime's trusted PgClaimProjection and acknowledge pending values after known commit. Unknown, expired and untrusted claims cannot authorize restricted list/detail. Accepted persisted claims remain usable when new verification is unavailable. Public DTOs omit exact age, private bounds/expiry and private profile avatar URLs.

Migration 0049 registers the retained Intro schema and a private immutable same-transaction native witness. Both actual canonical registration ACKs and the confirmed full native identity must exist. Deferred commit requires the held deadline plus a witness from the current native transaction. Update/delete/truncate cannot rewrite witnesses. `IntroNativeResolver::new(pool, instance) -> Result<Self, Status>` uses actual native negative proof and requires the exact durable witness for COMMITTED; native commit alone remains UNKNOWN. The witness is settlement evidence, never new request authority. Feature code never marks serialization, releases scope or settles delivery.

The edit callback invokes the retained `revoke_intro_consent_in` inside the actual Intro command transaction, after event/Intro locking and mutation and before outbox commit. It creates no new ledger, receipt, consent grant, context or cross-service SQL. Cursor encryption/authentication binds actor/event/query and preserves the original fence and expiry across continuations.

## Validation and limits

The native harness has two pure protocol/cursor cases and seven ignored canonical cases. `tests/intro_product_support/qualify.py` requires a clean committed source checkpoint plus frozen binary SHA-256 identities. It creates a fresh private marker/inode-owned socket-only PG16 cluster with actual mTLS Auth/Friend/Profile services and four isolated owner databases, reset between cases. Central migration receipts must match source checksums. Fixture claims, Social records and selected source-bound quality-v6 release are explicit test metadata, not production grants or operator admission.

Cases cover all six operations, private-field omission, CAS/replay/precise expired-key reason, restricted list/detail across outage/trust/expiry, parent rebinding, block/departure/suspension, fatal Profile outage, original response disclosure ceiling, consent/outbox atomic rollback/replay, exact native witness, witness immutability, rollback and unknown committed effect quarantine. Other cases require zero prepared transactions. The final unknown-commit case deliberately retains owner exclusion, recorded separately; final owned-cluster destruction removes that quarantined fixture. Four retained Intro engine cases run separately on their unchanged explicit synthetic-authority prerequisite schema, followed by restart persistence. Those regressions are not canonical authority evidence.

Current checks:

- `rustfmt +1.88.0 --edition 2021` on owned Rust: exit 0, parsing/formatting only.
- Python compile of the owned qualifier: exit 0.
- `python3 scripts/mcr/check-plan.py`: exit 0, PASS; original source checksums remain intact.
- Rust build, pure/native tests and retained engine execution: **NOT_RUN** pending a fresh named allocation.

Build request: Rust 1.88, `--locked --offline`, jobs <=2, debug 0, incremental 0, exact `cargo test -p concert-service --test intro_product --test intro --no-run --message-format=json`. Freeze source inventory/archive, Cargo JSON and exact executable copies/hashes outside the target, then release the target before private PG. No target or PG is currently held.

Public HTTP/main composition, browser/two-client IN03, client cache invalidation, actual operator admission and commercial provider activation remain **NOT_RUN** here. The existing unbudgeted goal is unfinished. Its API metadata still reports the old BLOCKED integration gate and old checkout; it was neither falsely completed nor replaced. The tools expose no active-status setter. The current authorized Product assignment resumes source work and remains unfinished through validation, committed handoff/fixes and normal Root publication plus fresh ancestry verification.

## Provenance

Audited headless backend code only. Existing migrations and supplied source archives remain unchanged. Historical source revisions establish provenance, not current runtime proof.

| Source revision | Source path | SHA-256 | Destination/adaptation |
| --- | --- | --- | --- |
| `fbe54712c3cb0bd95de937b001ee6a00eaefe900` | `backend/services/concert-service/src/adapters/out/repository/intro.rs` | `623b574db5183bad6f7517dd48c87075a1defce8e74d7bd80d08e0673f7cbb83` | `retained engine and relocated intro_live adapters` |
| `fbe54712c3cb0bd95de937b001ee6a00eaefe900` | `backend/services/concert-service/migrations/0039_intro_runtime.sql` | `1e14cb9bfb36dda6160bcd43eb0f0614b2c0f3841353b82bc673ec923b47b45b` | `reserved migration 0049` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native/environment.rs` | `adde0dc16f21112e866bea96f11334d6033f6669fad408569de492f5c392c352` | `intro_product_support/environment.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native/owners.rs` | `0d9c52e710823bd936d36daf46da25dae08226efeb5db96dd4209dde2280a9ce` | `intro_product_support/owners.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native/capture.rs` | `438063134a950ef2d06a1f25a876f023552201ebab6304c8d77853ed70677806` | `intro_product_support/capture.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/src/adapters/out/public_profile/native.rs` | `3b276170b55f9e24d67841335b8ba3b8fe6d275831b8614f63bc8a0d0babbf3a` | `intro_product_support/fixture.rs` |
| `a7f3989d9c40c53a9c2982bced938f8b4853f627` | `backend/services/concert-service/tests/age_runtime_support/qualify.py` | `5d960e413cb54eff1ebaaafdacf0e64f231c70022098221b5fe2798226b23c68` | `intro_product_support/qualify.py` |

Pinned contract bytes at the base (no amendments):

- `contracts/domain/v1/operations.json`: `f801d703dfc5509bbe06a2f0fa07a170b6442a48b2204c7ead460fc9a3600112`.
- `contracts/generated/mcr-domain.openapi.json`: `905d6f5b6f66f95cb68499a5fbfb32802539fce515f421226d79cb375ecfb887`.

Actual build/native artifact IDs and exit codes will be appended after execution. Source readiness is not acceptance.
