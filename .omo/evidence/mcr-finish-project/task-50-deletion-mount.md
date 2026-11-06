# Task 50 - mount the account-deletion runtime on the serving listener

Status: DONE (verified)
Delivered commit: `79d18315e` `feat(account): mount the account-deletion runtime on the serving listener`
Merged to master: `1234ae7c6` `merge(account): mount sealed deletion runtime on serving listener`
Verifier: independent verifier run `st_01a1af71`, verdict `confirmed`, confidence 0.85.

## Scope

`backend/services/concert-service/src/main.rs` builds `AccountDeletionRuntime::compose(Managed, grpc_pool.clone(), CONCERT_AUTH_GRPC_ENDPOINT, runtime.tls.as_ref(), CONCERT_SERVICE_CREDENTIAL, AuthDeletionCallers::configured(), LiveDeletionOwners { worlds: deletion_world.as_serving_world(), sharing_publisher })` inside the supervised gRPC task and mounts it with `.add_optional_service(deletion)` on the same tonic router that serves `serve_with_shutdown(runtime.address, global shutdown)`.

`config/world_runtime.rs` adds `serve_with_ready_and_deletion(SocketTransport::default(), None, Some(deletion))` delegating to the existing `serve_inner`; `main.rs` selects it only for the (no-social, deletion) tuple and keeps `serve_with_ready_social_and_deletion` for the social path.

`deletion_rpc` policy: service `concert.account_deletion.v1.AccountDeletionService`, workload `Concert`, `Apply` callers `[Auth]`, `guard_without_delegation`, so the mounted service is Auth-only on the shared listener.

## Verification evidence

- `cargo check -p concert-service -p api-gateway --all-targets` -> exit 0.
- `cargo test -p concert-service --test account_private_data` -> 3 passed, 0 failed.
- `cargo test -p concert-service --bin concert-service` -> 3 passed, 0 failed, including `serving_binary_composes_and_mounts_account_deletion`.
- `cargo test -p api-gateway --test unit` -> 216 passed, 0 failed.
- `backend/scripts/mcr/account_deletion_mount.py` against a fresh private PG16 cluster (PostgreSQL 16.14, socket-only, owned 0700 root): PASS, both phases exit 0, including a physical PostgreSQL restart (postmaster pid changed), sealed `PrivateData` ACK over mTLS, caller refusal before the deleter, zero unrevoked sessions for the deleted user, tampered retention refused by the Auth source, restart replay without duplicate effects, zero private rows, control user intact, migration ledgers auth=19 / concert=57 before and after.
- Cleanup: owned cluster root verified absent on disk; no residual process.

## Limitations

- The authenticated Apply path runs the production factory on a test-owned loopback listener; source composition of `main.rs` is proven by a composition assertion, not by launching the real process.
- Sharing terminal-row behaviour is a durable fixture for the prior saga step; the Social owner flow itself is not re-qualified here.
- No Gateway HTTP, browser or deployment acceptance is claimed.
- A World configured without Social uses the deletion-only handoff method.

## Notes

Earlier evidence for this row was produced outside the repository and was not committed; this file reconstructs the verified record from the delivery commit, the merged tree and the independent verifier's evidence so the row's evidence path resolves.
