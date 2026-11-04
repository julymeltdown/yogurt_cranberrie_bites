# CR08 canonical producer cancellation

Status: IN_PROGRESS. Source and nine owned-PG regression cases are authored. Compilation, native qualification, Participant integration, serving, and master delivery are NOT_RUN until their exact receipts are recorded. This document does not upgrade the earlier e692 live incident or any old test receipt.

## Observed trigger and source ownership

The expired e692 runtime recorded a `getConcertParticipant` GET with an ACTIVE root, a published delivery hash, no serialization marker, no registered transactions, and no child intents. Auth login was blocked by the real prepared owner exclusion. Existing native participants being terminal was not proof that this root could issue no more work. Existing unpublished NoWork cleanup could not truthfully describe the already published hash.

Base: `a0730a4e` in the isolated `codex/mcr-be-CR08` worktree. Root assigned guarded `scope.rs`, `registry.rs`, `mod.rs`; product `delivery.rs`, `scope_ledger.rs`, optional new `scope_ledger/cancellation.rs`; migration 0048; the new cancellation tests/support; this document. The exact later transfer adds only the two cancellation methods in the existing `ScopeLifecycle for PgScopeLedger` adapter. Participant service composition is owned separately by Participant209. No main/config, contract/proto, Notification temporary arms, or existing child implementation is transferred.

## Producer API and proof

`OwnerGuardScope::run_owned(&Arc<Self>, future)` accepts a `Send + 'static` future returning `Result<T, tonic::Status>` with `T: Send + 'static`. Call once immediately after acquisition, without an intervening await. Move all subsequent context validation, authority construction, delivery publication, actual local/remote producer work, final validation, and response construction inside that future. Only a returned successful value enters the existing response serialization path.

The spawned supervisor owns the actual pinned future. It captures the earlier of the binding deadline and current propagated call deadline, installs that original deadline task-local around work polling, prioritizes cancellation/deadline over ready work, and checks the deadline again before successful handoff. It never extends content authority. The independent two-second terminal budget applies only to sealing and actual settlement after content work has stopped.

Caller Drop closes a channel; it is not terminal proof. On caller cancellation, original deadline, or an actual producer error, the supervisor explicitly destroys its owned work future before constructing the private `CancelledRootWork` capability. A successful value that cannot be sent is destroyed before proof. A successful send transfers ownership to the old response-body lifecycle: a later lost/reset/unpolled body remains quarantined. A panic or lost supervisor does not construct proof.

Producers must not spawn or otherwise let independent unregistered work escape the owned future. Remote operations must retain their existing exact durable pre-dispatch child intent and native transaction registration; destroying a Rust future does not prove remote/native completion. The supervisor stops further local dispatch, not remote business effects.

Under the existing settlement mutex and admission mutex, cancellation rejects any serialized response and seals local state plus actual registry admission. It releases the admission mutex before draining all admitted `RootReadPermit`s, so a late callback can observe the sealed state and return. It durably seals the root, waits for the actual callback count to reach zero, and only then records abandonment and ROOT_FINISHED. Missing registry, failed seal, timeout, panic, and unfinished callbacks retain quarantine. A timed-out supervisor cleanup is not later revived merely by dropping a permit.

After truthful root finish, the existing real Auth and Friend Complete RPCs independently resolve their exact native registrations. Actual retained-child completion and durable typed child recovery must also succeed. Unknown transactions, missing child proofs, adapter errors, or lost receipts do not certify settlement.

## Additive schema 0048

`response_abandoned_at` records the distinct no-response terminal proof. It is written atomically with ROOT_FINISHED after actual owned-work termination and callback drain. The delivery secret hash remains immutable; `serialized_at` and `delivery_completed_at` remain NULL. This is not HTTP EOF, successful delivery, or a business/native receipt.

`cancellation_settled_at` is a separate immutable receipt written only after actual owner and child settlement succeeds. The trigger requires prior abandonment and rejects unresolved child intents. Durable capacity excludes settled cancellations; abandoned but unsettled roots remain counted, including unpublished ones. The old unpublished NoWork exemption now explicitly excludes abandoned roots.

The migration preserves original successful serialization/delivery transitions, permits only the two additive cancellation transitions, and prevents changing immutable bindings, secrets, or existing timestamps. Existing rows are not backfilled, reset, or repaired. The old serving incident is forensic evidence only.

## Regression inventory and qualification

New entrypoint: `backend/services/concert-service/tests/owner_authority_cancellation.rs`. Shared existing fixture/capture/owner/environment source is read and reused without modification. The actual owner services and Concert ledger run over owned PG16 plus mTLS; no positive owner resolver fake is introduced.

| Exact case in `cases::` | Required evidence |
| --- | --- |
| `returned_error_records_abandonment_without_delivery` | Published real capability; actual error; abandonment and actual owner settlement; no serialization/delivery; cannot finish as ordinary response or start a second supervisor. |
| `caller_cancellation_destroys_actual_work_before_owner_release` | Actual caller abort; actual work destructor; original task-local deadline; actual owner settlement. |
| `unresolved_callback_and_expiry_never_prove_abandonment` | Actual admitted callback retained beyond two-second cleanup/original deadline; sealed quarantine; late admission denied; no abandonment or owner release. |
| `active_native_transaction_prevents_cancellation_settlement` | Real tracked in-progress PG transaction; truthful root abandonment but actual owner fences and missing cancellation settlement receipt. |
| `successful_handoff_retains_existing_unserialized_quarantine` | Successful response ownership transfer followed by token Drop does not create cancellation/delivery proof or release owners. |
| `drained_late_callback_allows_true_abandonment` | Existing callback sees seal and exits; root finish waits for its actual permit; then real owners settle. |
| `expired_original_deadline_never_polls_ready_producer` | Already expired propagated deadline prevents even ready protected work from being polled; independent settlement can still complete. |
| `failed_work_destruction_never_reaches_terminal_proof` | Actual producer destructor panic prevents even root abandonment; real owner fences remain. This detects terminal proof occurring before actual destruction completes. |
| `unpublished_unsettled_abandonment_remains_in_durable_capacity` | Actual Auth lease-table lock prevents Complete; unpublished abandoned root remains counted by a new ledger and empty registry, so the sixteenth additional durable admission is rejected. |

Build only under a separately named resource lease:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j2 -p concert-service --test owner_authority_cancellation --no-run --message-format=json
```

Freeze the executable outside target, record exact clean source commit/full `owner_authority_support/qualify.py::source_manifest()` hashes, successful exact build command/exit and executable hash, and release all target handles before PG. The new `owner_authority_cancellation_support/qualify.py` requires that checkpoint and runs exactly one selected case in a fresh owned three-database fixture; use a new report per case. Expected negative cases retain Auth/Friend prepared exclusions before exact owned-cluster teardown. Teardown is never recorded as settlement.

Participant209 separately owns interrupted actual public HTTP, fresh real Auth login after recovery, and callback/unknown-native negative flows. Core tests do not certify that service integration, production main activation, HTTP EOF, physical restart, or release readiness.

## Execution evidence

- Rustfmt and Python AST checks: PASS (exit 0), source authoring only.
- Source review: two concrete findings corrected before qualification: abandoned/unsettled capacity accounting and expired-ready-work dispatch/handoff races.
- Compile/native checks: NOT_RUN on this source revision.
- Committed handoff, integration ancestry, and master delivery: pending.
