# Participant post-response delivery retention (CR10)

Status: implementation and scoped validation complete; master integration pending. Full project goal remains ACTIVE. Base: `c9dd4c980f39b12eda5a79fff871c8de48f8df71`.

## Scope and provenance

Owned production files are the Participant Gateway inbound HTTP and outbound gRPC adapters. New tests/support and this document are the only other writable paths. No protocol, manifest, composition, generic Product, or source archive changes.

The controlled-peer regression fixture derives from `backend/services/api-gateway/tests/concert_participant.rs` at the base revision. Its identity and route helpers are reused unchanged by relative path. These peers issue synthetic capabilities and therefore establish Gateway behavior only, never native owner settlement.

## Defect and required behavior

After receiving a serialized authenticated Participant response, freshness checks can return before preserving its delivery capability. HTTP access rechecks and disclosure-body construction can also discard it. An expired response must disclose no protected content. Once the complete reply has passed authenticated metadata, bounded positive original expiry, actor/event/schema, closed failure and capability validation, a late refusal must retain that same capability through a newly authored closed public error. Only actual outer HTTP EOF can settle it. Drop, partial consumption, body error, invalid reply, and unknown delivery remain quarantined.

## Validation

`participant_delivery_retention` contains a post-gRPC shorter-content-ceiling expiry regression, error-data-versus-EOF assertions, partial/drop negatives, mismatched actor, malformed capability, missing no-store metadata and zero-expiry negatives. Pre-fix source `df8728b1bc6a490b69f6782e8a294d880908b200`: build PASS (exit 0, Rust 1.88 locked/offline, 30.39 seconds), copied component run FAIL as required (exit 101, 2 passed / 1 failed). The exact positive error EOF test observed completion 0 instead of 1; the drop/partial/invalid reply test passed. Frozen binary SHA256 `2256b6991f904660eb834ee007cb72566c064f3a9fe1a7d4c96f29830e385fab`; log SHA256 `48c0cfa0b9f359f0fb42088d803f7ad55491f4ac5f0ce6f6a7604c398f0025b7`. Actual zero-handle build-target release preceded the copied test; no PG was started. Expected functional failure is preserved independently of later fixes. Fixed execution at `05ae2e8ff5a9b0f33f922d19d97e5165b24b16e2`: PASS, 3 controlled tests plus the explicitly ignored native host. Original Participant regressions: PASS, 7 tests. Actual PG16.14/mTLS/HTTP/error-EOF/Auth-refresh qualification: PASS (exit 0, 21.28 seconds). Exact signed-JWT expiry scheduling and the Root production incident are not reproduced by these controlled peers.

## Implementation and native regression boundary

The outbound adapter validates no-store metadata, positive expiry bounded by the sent original authority, opaque delivery structure, and the closed response schema/actor/event before post-response freshness checks. A late refusal clears protected bytes and keeps the exact capability and original expiry in a closed failure. The HTTP adapter captures delivery before its late access check; locally authored errors remain readable without a protected-content gate, while successful content retains the unchanged original DisclosureBody checks and error/drop quarantine semantics.

The new native host uses production Gateway state/router/transport layers, actual signed JWT verification and owner identity reads. A test-owned mTLS proxy forwards the original signed Participant request to the real production Concert main, preserves the genuine delivery capability, shortens only the returned disclosure ceiling and delays beyond it. Separate body gates hold an unopened error or return only half a data frame before actual TCP reset; they never poll/synthesize EOF. The controller requires real signup/mail verification/login/Profile/bootstrap, genuine terminal-error EOF/durable owner settlement, and actual Auth refresh 200 afterward. Negative roots must remain serialized/undelivered with four prepared fences until owned-cluster teardown.

All original 2,151 retained d0f source input hashes and the frozen Concert binary hash match the CR10 base. Nevertheless its embedded migration directory points into the advancing integration checkout, which now has an additional migration. Native qualification must rebuild the exact Concert main in this stable worktree; the six frozen peer embedded migration sets still match the base. No released build target is accessed for reuse.

## Executed qualification and delivery

The compiled/native source is `05ae2e8ff5a9b0f33f922d19d97e5165b24b16e2`. Its 2,157 source/build/runtime input hashes were independently compared to that commit's Git blobs, with zero mismatches. The production fix is `dd34f46521f43ec23b7b3d9526df73e7e23c3cb1`; `05ae2e8f` only strengthens native input pinning and the negative expiry oracle. The later evidence successor does not claim a new runtime run.

| Check | Command or boundary | Exit/result |
| --- | --- | --- |
| Pre-fix red | copied `participant_delivery_retention --test-threads=1 --nocapture` at `df8728b1` | 101; required positive EOF failure reproduced, negatives passed |
| Fixed build | Rust 1.88 `cargo test --locked --offline -j 2 -p api-gateway --test participant_delivery_retention --no-run --message-format=json` | 0 |
| Stable production mains | same locked/offline toolchain, `cargo build -p concert-service --bin concert-service` and `-p api-gateway --bin api-gateway`, JSON artifacts | 0, 0 |
| Existing suite build | same toolchain, `cargo test -p api-gateway --test concert_participant --no-run`, JSON artifact | 0 |
| Fixed component | copied exact test binary, serial tests | 0; 3 passed, native host explicitly ignored |
| Existing regressions | copied exact original-suite binary, serial tests | 0; 7 passed |
| Native regression | new owned PG16.14, mTLS, actual production mains, signed Participant proxy, real HTTP | 0; terminal error EOF, durable settlement, real refresh 200, negative resets all passed |

All builds used `--manifest-path backend/Cargo.toml`, debug information disabled, incremental compilation disabled, and the declared narrow `profile.dev.package.contracts-build.opt-level=2` tooling override. All Cargo JSON and copied artifacts were frozen before unconditional target release. A conservative private compile subguard stopped the first fourth-target attempt after including frozen peer copies (768,925,696 bytes versus 751,619,276); it remained below the granted 2 GiB whole limit. That aborted receipt is retained. Aligning the guard with the already granted whole limit, without resetting source/baseline/window, yielded the exact fourth-target retry PASS in 4.42 seconds.

Native report SHA256: `a66db33a06ad18f99c4221eb5dc7d2ff04ad7ff9175134ea4d316bc5f908b48c`. Resource/cleanup guard SHA256: `1cfee2268cf7c967e378d33ce6e8d7e2a55544d3429001e0d67d0cb187746572`. Portable details: [native qualification](../../../backend/services/api-gateway/tests/participant_delivery_retention_support/native_qualification.json).

Actual native evidence: three principals completed real signup, mail verification, login, Profile creation and bootstrap; a separate login and refresh also returned 200 (17 HTTP 200 setup/Auth results). The positive GET produced a genuine serialized Participant response; the fixture shortened only its content ceiling. The safe 503 was consumed through actual terminal HTTP EOF, the exact root became ROOT_FINISHED with genuine DeliveryCompleted, Auth/Friend leases became COMPLETED with settlement, and prepared transactions reached zero. A genuine cookie-based Auth refresh then returned 200. No protected response content or capability was exposed.

Actual TCP reset of an unopened safe error retained two prepared fences. Reset after half a data frame retained four total. Both roots remained ACTIVE, serialized, undelivered, nonabandoned and without cancellation settlement after their original expiry. Negative-cluster teardown is not recovery evidence.

The allocation was `CR10-FIXED4-BACKEND-202611040633Z`: whole hard bound 2 GiB, observed peak 981,446,656 bytes; native additional peak 141,434,880 bytes. Actual zero-handle target release occurred at `2026-11-04T06:34:24.207679Z`, before every copied test and PG run. Full workflow release occurred at `2026-11-04T06:36:22.461047Z`. No target access occurred after release. The owned PG root is absent, all eight recorded service PIDs are absent with unforced exit 0, the native host was waited successfully, and the runtime process group is absent. Five retained owner peers' actual hashes, 1,108 required compile-input hashes and live baked migration files matched before and after runtime; fresh Gateway/Concert mains used this stable worktree's actual migration directories (17/36).

A targeted independent read-only review found no actionable issue in the two production changes; it is static evidence only. This native case covers GET, not a full browser incident replay or exact signed-JWT scheduling. First-DATA expiry after success-body construction still raises a body error and intentionally quarantines delivery. Dropped, partial and unknown bodies remain fenced. Full Product/Participant composition, sustained IN01 and project acceptance remain open under their owners.

Final source checks: `python3 scripts/mcr/check-plan.py` PASS (exit 0, planning provenance only); `git diff --check` PASS (exit 0). No application acceptance is inferred from the planning check.
