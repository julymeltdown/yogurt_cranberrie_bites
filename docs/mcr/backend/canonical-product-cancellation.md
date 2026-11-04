# CR09 canonical Product cancellation

Status: INTEGRATED for the committed source and exact component/HTTP qualification recorded below. Earlier authoring NOT_RUN statements are historical checkpoints. This patch does not activate Product modes or certify production main, browser behavior or master delivery.

## Trigger and exact ownership

At base `c9dd4c980f39b12eda5a79fff871c8de48f8df71`, `ScopedProductOwner::dispatch` acquired actual owner fences, awaited durable publication outside supervision, and ran a producer future borrowed from the request. Dropping that caller or expiring its original deadline could destroy local work without entering CR08's actual-work cancellation proof. A publication failure had the same gap. Generic Product modes remain disabled by the production constructor, so this is preparation for canonical cross-producer activation, not a claim of an observed Product production incident.

Root assigned the isolated `codex/mcr-be-CR09` branch these exact writable paths:

- `backend/services/concert-service/src/product_rpc/scoped.rs`.
- `backend/services/concert-service/src/product_rpc/mod.rs`: only a private validated owned-context copy helper inside `VerifiedProduct`.
- New `backend/services/concert-service/tests/product_owner_cancellation.rs` and `tests/product_owner_cancellation_support/**`.
- This new document.

No main, configuration, owner policy, contracts/proto, central manifest/lock, query DTO, Participant, CR08 supervisor, ledger, lifecycle, native resolver or migration is changed. Previous CR07/CR08 writers are stopped except for concrete handoff fixes.

## Behavior and authority

The owner retains the already authenticated context, its exact sealed parent and producer/publisher handles before acquisition. There is no intervening await between successful acquisition and `run_owned`. All post-acquisition context checks, actual durable publication, construction and polling of the producer future, produced-error projection, disclosure check and response construction belong to the supervised future.

The private helper copies identity, event, session, request, operation, trusted network bucket, original monotonic and wall deadline, feed/comments query fields and command query. It clones the existing sealed parent and owned cursor strings. It does not parse new observations, construct public authority or renew a deadline. Feed/comments DTOs remain unchanged.

Tokio task-local values do not cross a spawn. The future explicitly scopes the original sealed parent around all work; the existing CR08 supervisor separately retains the earlier original call/binding stop. This preserves nested child delegation and exact original authority in the owned task. Producers still must register actual native transactions and durable child intents and must not let independent work escape the owned future.

Ordinary producer refusals remain safe error representations inside supervision. They retain the existing response token, gRPC serialization and actual Gateway terminal HTTP EOF path. Returning an error envelope is not rollback proof. A caller cancellation, original deadline or publication failure can enter CR08's truthful abandonment path only after actual owned-work destruction and callback drain; unknown native/child work still prevents settlement. Successful handoff transfers ownership to the existing body path. Dropping that unconsumed body remains quarantined and is never retroactively classified as cancellation or successful delivery.

## Regression inventory

Every native case requires an exact clean committed-source checkpoint, an executable frozen outside target and target release before owned PostgreSQL startup. The test entrypoint imports the existing actual Auth/Friend/Concert fixture and production workload/actor guards without changing them. No positive owner scope, verified context, EOF, native completion, authority renewal or successful resolver is fabricated.

| Exact case | Required evidence |
| --- | --- |
| `cases::caller_drop_destroys_actual_product_before_owner_release` | Signed guarded Product dispatch; publication before producer; actual producer destructor after caller abort; truthful abandonment, actual owner settlement and zero native exclusions. |
| `cases::original_deadline_stops_actual_product_and_settles_without_delivery` | Original task-local deadline retained; pending actual producer destroyed at expiry; abandonment/settlement without serialization or delivery. |
| `cases::admitted_callback_past_expiry_retains_actual_product_fences` | Actual admitted callback held beyond original expiry and terminal drain budget; SEALED without abandonment, no late admission and owners still excluded even after callback cleanup. |
| `cases::committed_native_without_receipt_keeps_actual_product_quarantined` | Actual transaction registered and COMMITted without a business witness; truthful abandonment but no cancellation settlement and real owner exclusions retained. |
| `cases::successful_handoff_lost_before_eof_never_becomes_cancellation` | Actual successful guarded response dropped before body polling; no serialization, abandonment, delivery or owner release. |
| `cases::original_context_queries_parent_and_actual_success_eof_are_preserved` | Feed/comments/command queries, identities, sealed parent, original deadline and network context survive ownership; actual retained gRPC body EOF only marks serialization; authenticated terminal capability completes real owners. This component case alone does not certify outer HTTP EOF. |
| `cases::actual_publication_failure_is_supervised_before_producer_dispatch` | Actual wrong-registry publication denial after acquisition; producer never constructed/dispatched; truthful abandonment and actual owner settlement. |
| `cases::actual_product_destructor_fault_prevents_terminal_proof` | An actual producer destructor panic prevents abandonment/root-finish and owner release, detecting proof before destruction completes. |
| `cases::caller_drop_during_actual_awaited_publication_never_dispatches_product` | Actual publication UPDATE blocked by an owned-fixture-only SQL gate; caller cancellation while awaiting it cannot dispatch producer work, even after the gate is released; actual owners settle without delivery. |
| `product::produced_errors_keep_real_guards_until_terminal_delivery` | Existing 17-scope production-guard/retained-gRPC-body regression; safe refusal envelope and actual terminal RPC settlement exceed the actual capacity ceiling without leaks; duplicate completion remains idempotent. |
| `http::actual_public_router_retains_real_owners_and_checks_before_bytes` | Existing real-signup/login/Profile, production public router and actual ungated TCP error response EOF; genuine terminal settlement; separately gated reset/expired bodies retain exclusions. The gated cases change the size hint and do not certify unchanged HTTP framing. |

The last two cases reuse the reviewed source at the exact base rather than copying or weakening their assertions. The HTTP test is driven through the new owned `qualify_http.py`, adapted from `owner_authority_support/qualify_http.py` at the base: only its test target/checkpoint name and explicit existing core-helper import change. The new component qualifier is adapted from CR08's one-case owned PG16/mTLS controller with the exact new case inventory, test target and limitations. All reused Rust fixture/controller paths and revisions are covered by the source manifests and build checkpoint.

## Planned qualification and evidence limits

Exact requested artifact: `concert-service --test product_owner_cancellation`, using Rust 1.88.0, locked/offline dependencies, at most two jobs, debug information and incremental compilation disabled. Cargo/shared-target and PostgreSQL work require a fresh named offer from Backend staffing and resources. Freeze the executable and complete input hashes, release target handles, then run the ten component cases individually through `product_owner_cancellation_support/qualify.py` and the actual HTTP case through `qualify_http.py` with an explicitly qualified six-peer cohort.

Five new component cases (caller Drop, deadline, context/success EOF, publication failure and cancellation during awaited publication) plus the existing produced-error case require Auth/Friend/Concert prepared counts 0/0/0. Four component negative cases require 1/1/0 before owned teardown. The HTTP fixture requires its two expected reset/expired owner exclusions (2/2/0). Teardown is never settlement. Current receipt status for every case remains NOT_RUN.

Full canonical acceptance additionally requires the actual typed cross-producer child composition, native resolver union, production worker supervision, combined qualification and scoped validated non-force master delivery. This patch does not substitute constructor, source, component or fixture evidence for those obligations.

Targeted independent read-only production review found no concrete regression. Its two coverage limitations were addressed by adding the actual destructor-fault and awaited-publication cancellation cases above. Rustfmt, exact helper-only diff and Python AST/source-manifest parity checks passed; runtime checks remain NOT_RUN until a fresh allocation.


## First exact execution checkpoint

Source `32f95676be921729c8bfa4916c2ed7490d60bed1` compiled successfully: Rust 1.88.0 locked/offline, jobs 2, debug 0, incremental 0, declared `profile.dev.package.contracts-build.opt-level=2`, exit 0 in 41.43 seconds. Its 61,694,544-byte frozen executable SHA-256 is `782f64f49885cf7e243f1c119a713e3e9d0df8f28651a6f206567c0336bc41f4`, with 2,070 complete component/HTTP input pins. A private monitor's first attempt failed on its missing budget field, stopped its own process group and received no build credit; the corrected same-allocation retry passed without source changes or resource abort. Shared target release with zero handles and absent Cargo occurred before PostgreSQL at `2026-11-04T05:57:57.088045+00:00`.

The first actual caller-Drop case FAILED before its cancellation action because its test required the guard's monotonic stop to equal the incoming context's stop. Actual values differed by 417 ns: `GuardBinding::from_parent` legitimately calls `AuthorityRpcDeadline::within_original`, which may only shorten the original stop. This was an assertion defect, not evidence of production authority renewal. Exact owned fixture cleanup passed with root absent, and the copied runtime reservation was released; remaining cases were NOT_RUN. This failure is retained without upgrading its result.

The corrected test observes the already authenticated original deadline through a delegating ProductOwner wrapper before entering the actual ScopedProductOwner. It requires the producer's owned context to preserve that exact original wall and monotonic deadline, permits only a shorter/equal actual guard stop, and requires the supervisor task-local stop to equal that guard stop. The observer cannot mint a verified context or replace actual scope acquisition, producer supervision or owner settlement. No production code changes for this correction. A new committed-source executable and fresh resource allocation are required before rerunning qualification.


## Corrected-source native qualification

The test-only successor `c1fd3ec8a867a4edce40abe68b1ff8a06a4a79ed` compiled successfully in a fresh named `CR09-RETRY-BACKEND-202611040600Z` allocation: exact command above with declared contracts-build opt-level 2, exit 0 in 5.11 seconds, jobs 2, debug 0 and incremental 0. Production source is unchanged from `32f95676`. The frozen executable is 61,714,240 bytes, SHA-256 `d3fdfc5264396a0a8660978257916758d4a9ae2284d154d72f2362f28914de6a`. Independent readback compared every one of the 2,070 complete component/HTTP input hashes to that exact Git tree and rehashed the copied executable. Target release at `2026-11-04T06:02:02.198822+00:00` confirmed zero handles and absent Cargo before PostgreSQL; no target access occurred afterward.

All ten component cases passed individually with exit 0 and one actual test passed in fresh owned PG16/mTLS fixtures. Migration receipts verified Auth 18, Friend 18 and Concert 36. The six positive component cases ended with prepared counts Auth 0 / Friend 0 / Concert 0. The four expected negatives retained Auth 1 / Friend 1 / Concert 0: callback drain, committed unknown native work, lost successful handoff and actual destructor failure. Every fixture cleanup passed with root absent; teardown is not settlement.

The actual public HTTP fixture also passed, exit 0 / one test passed. It performed real signup, verification, login and Profile creation (all HTTP 200), then exercised the production public router and actual socket framing. The ungated error response reached genuine terminal EOF, recorded DeliveryCompleted and released real owner fences. The separately gated reset and expired responses retained exactly Auth 2 / Friend 2 / Concert 0 before owned teardown. This is the actual outer-HTTP proof in addition to the component retained-gRPC-EOF cases.

Its first attempt stopped before HTTP behavior because the private runner selected Apple LibreSSL, which could not load the actual Ed25519 key. That FAIL and successful cleanup are retained. Changing only the private runner argument to the installed Homebrew OpenSSL 3 executable allowed the unchanged source and frozen test executable to pass; no product, policy, fixture or authority change was made.

The HTTP controller reused Root's six frozen peer mains compiled at `d0f063d18a0549935f80a536077c30962d89f9e9`. Actual bytes matched the original coherent cohort. Git input comparison verified all six owning service directories, shared crates, proto, scripts, manifests and lockfile unchanged; intervening backend changes were only Concert wrapper/helper or Concert test/evidence paths. Gateway's Concert dependency is test-only. The new Concert host was separately compiled at `c1fd3ec8`; peer revisions are not relabeled. The whole fixture used a new private cluster and its own processes, never Root's protected IN01 runtime.

Every capacity guard exited 0 without a resource abort. Maximum sampled whole additional allocation was 225,640,448 bytes against the 2 GiB hard bound; sampled runtime additional allocation was 163,123,200 bytes against 0.8 GiB. The eight owned HTTP service resources stopped without force and with exit 0; actual recorded PIDs and every owned cluster root were absent on readback. Both the target and copied runtime reservation are fully RELEASED.

Portable [native qualification](../../../backend/services/concert-service/tests/product_owner_cancellation_support/native_qualification.json) records all ten component receipt/guard digests, exact compile/source/checkpoint/release hashes, the actual HTTP result and peer provenance, original failures, cleanup and limitations. It refers to the actual compiled source above; this evidence/documentation successor is not relabeled as a fresh build. Root intake, scoped combined validation and non-force master ancestry remain pending, as do the full canonical production composition and worker activation gates.
