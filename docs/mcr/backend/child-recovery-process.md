# Physical Root-process recovery qualification

## Scope and status

CR01 is implemented as an ignored, explicitly invoked integration test and an owned four-database PostgreSQL 16 controller. Initial compilation and physical runtime are `NOT_RUN`. The already qualified child-worker component and Gateway HTTP tests are separate evidence; neither certifies this process boundary. CR02–CR06 remain open. Production main activation belongs to the delivery integrator.

The controller starts the exact frozen `process_child_recovery` test executable twice. The old process uses actual Product/Delivery services, canonical Auth/Friend stores, the source-bound PublicProfileReader, strict fixture mTLS, real Local/Profile children, and native prepared transactions. A negative-only Auth Complete fault leaves a genuinely finished Root with two durable pending children, no producer transaction, and no delivery receipt. The controller SIGKILLs and reaps its exact old child, then starts a distinct replacement PID with the same instance and keyring and an empty ScopeRegistry.

The replacement uses the production child worker to discover and settle both children. Real rollback-only Local/Profile writers must fail with SQLSTATE `55P03` before death and before recovery, then succeed after actual native release and durable ACKs. Original native GID/XID identities must survive process death. The old RootReadView must remain absent and content-denied; CompleteResponse must remain unavailable throughout. After the unchanged original expiry selects a sweep, actual Auth/Friend recovery queries the replacement Root's authenticated settlement service. Genuine RootFinished proof authorizes release. All four databases must finish with zero prepared transactions, and immutable owner completion proof must match the original scope.

Fixture Auth/Friend/Profile RPC servers share each Root child PID and are restarted with it. This does not claim independently surviving owner-service OS processes, production main startup, or external Gateway network HTTP. The outer terminal-body fixture calls the actual guarded DeliveryService on EOF. It does not substitute an ACK or update lifecycle rows directly.

## Reproduce from a clean committed source

1. Obtain a fresh named target allocation. Use Rust 1.88.0, existing locked offline dependencies, at most two jobs, debug information disabled, incremental compilation disabled, and at least 8 GiB free.
2. Run exactly `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-service --test process_child_recovery --no-run --message-format=json`. Record command, exit code, complete `checkpoint.source_manifest(repo)`, source commit, copied executable path and SHA-256. The source manifest includes every new test/controller plus selected runtime release and art resources. Copy the executable outside the allocated target and release the target before PostgreSQL execution.
3. Run `python3 backend/services/concert-service/tests/process_child_recovery_support/qualify.py --checkpoint /absolute/checkpoint.json --pg-bin-dir /absolute/pg16/bin --report /absolute/new-report.json`. The controller requires a complete clean source match and successful exact no-run build receipt, checks the owned marker/socket/system identity, applies actual migrations, keeps capabilities in private files, kills only its own children, verifies zero prepared work, and removes only its owned stopped fixture root.

Source-copy provenance is recorded in `backend/services/concert-service/tests/process_child_recovery_support/provenance.json`. No application code, manifests, lockfiles, protocol files, source artwork, or historical source documents are modified by this test patch.
