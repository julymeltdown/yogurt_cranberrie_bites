# Ordered World recipient-read candidate

Status: IN_PROGRESS; bounded component compilation, three concurrency tests and four actual-main startup refusals PASS after two retained failed attempts. Actual owning-service/database/privacy qualification and a current capacity run remain NOT_RUN. It does not establish the cause of RW9, RW10 or RW11.

Root owns only `src/world/production_owners.rs` and this document for this change. Sessions retains its assigned admission-bridge/socket deletion hooks; no PrivateData, Auth, protocol, manifest, timeout, arrival rate or authority policy is changed.

The prior visibility implementation awaited every candidate's qualified Auth/Friend/local observation serially inside one sealed parent. The candidate buffers at most four such futures and collects in the original subject order. Each still constructs its own typed presence projection and uses the existing reader with its original clipped deadline and target attempt gates. Existing local SQL semaphore limits remain in force. Ordered collection preserves native publication-plan/SQL lock order; all observations are checked again before the plan is installed. Any error or cancellation drops the in-flight futures and installs no partial plan. No detached task, new deadline or unverified observation cache is introduced.

Three source tests cover actual bounded overlap plus preserved ordering, failure cancellation without a partial result, and cancellation at one original deadline with all read guards released. They do not qualify a real owner, privacy race, initial-snapshot handoff or 200-user capacity result. Relevant compilation and copied tests require a fresh named allocation; native World recipient checks and a current same-candidate multiuser flow remain separate obligations. The former Green/runtime51 migration inventory is superseded by current Concert 0052 and Notification 0007.

The first allocated build, `ROOT-WORLD-VISIBILITY-MEDIA-202611041446Z`, compiled clean source `d534149b200136986468366c867bf9256a792cbf` using Rust 1.88.0, locked/offline jobs 2 and both Rust flag variables unset. Concert library test compilation exited 101 after 41.900 seconds: the generic mapped borrowed future did not satisfy the `Send` requirement at existing admission/maintenance call sites. No executable was qualified or run. The follow-up prepares each typed projection before buffering and boxes each individual reader future as `Send`; source ordering, cancellation and the original authority deadline remain unchanged. This repair requires fresh compilation.

The failed allocation is retained at `/private/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-root-world-visibility-qt1nnat4`. Actual ACCEPT was 14:50:20.662566 UTC; FULL release was 14:51:06.217935 UTC, 45.555 seconds later. The target was released with zero handles before any copied execution; positive target growth was 1,830,912 bytes. Whole upper bound was 6,269,689,856 bytes under 8 GiB; final free space was 25,637,224,448 bytes. All owned groups, temporary files and evidence handles were absent; no database or listener was started. Target access after its release is prohibited and was not used for the repair. The original failure receipts remain unchanged.

The second fresh allocation, `ROOT-WORLD-VISIBILITY-REPAIR-MEDIA-202611041459Z`, accepted clean source `395aebb3330ae33cf303a48597fd2e8c0d1ff3ea` at 15:00:25.891721 UTC. Cargo exited 101 in 1.268 seconds before evaluating the repaired reader: Auth's invoked build-script executable requested `authority/v1/deletion_source.proto`, absent from this older source candidate. The retained Root build.rs did not list that proto. This observed shared-target build-script/input mismatch is distinct from the first Rust diagnostic; the precise cache reuse mechanism was not independently established. No cache, fingerprint or target file was removed, and no placeholder input was invented. Root subsequently integrated the actual qualified Auth source-bridge proto and both build-script changes as one source handoff; the combined candidate needs fresh qualification.

Second failure evidence remains at `/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-root-world-visibility-followup-sz27bwqs`. FULL release was 15:00:30.769989 UTC, 4.878 seconds after ACCEPT and 0.425 seconds after target release. Positive target growth was zero; whole upper bound was 6,317,535,232 bytes, final free space 24,923,631,616 bytes. No copied executable, database or listener ran; all owned groups, temporary files and handles were absent. The repaired reader's compilation and tests therefore remain NOT_RUN at this checkpoint.

## Qualified bounded follow-up

Fresh allocation `ROOT-WORLD-COHERENT-BACKEND-202611041510Z` accepted exact clean source `ca4dd26fc13bbd8719315f621e03d94a4e4cde32` at 15:12:04.613474 UTC. This includes the qualified Auth source bridge and Sharing owner; Concert migration files number 42 and Notification seven. No database was created or migrated in this round. All 3,754 selected backend/contract/public-resource inputs were SHA-256 pinned before and after; all backend/contracts and the character source-pack readset was frozen outside the target. Public image inputs were hash-pinned but unused by these checks.

Before ACCEPT, tracked backend Rust/proto/SQL source mtimes were refreshed in the exclusive integration checkout, with SHA-256 verification that every byte stayed identical. This makes Cargo reevaluate current inputs after the observed shared-target build-script mismatch. No fingerprint, target, dependency cache, lockfile or compiler profile was removed or changed. The retained refresh receipt enumerates every input. The private Party draft in the evidence directory was charged to the allocation but never entered the application checkout or compilation.

Every Cargo command used `cargo +1.88.0`, `--manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 --message-format=json`, dev/test debug zero, incremental zero and both Rust flag variables unset.

| Command selection | Exit | Seconds |
| --- | --- | --- |
| `test -p concert-service --lib --no-run` | 0 | 65.875 |
| `build -p concert-service --bin concert-service` | 0 | 30.387 |
| `build -p notification-service --bin notification-service` | 0 | 6.677 |

Cargo JSON proves the first artifact is the Concert library test executable and the other two are actual non-test service mains. All three executables were copied and hash-verified outside the target before its unconditional release with zero handles. Only then did the copied executable stage run, exiting zero in 2.895 seconds:

- The `visibility_pipeline_tests --test-threads=1 --nocapture` filter executed exactly three tests, all passing: bounded overlap/order, failure cancellation, and original-deadline cancellation.
- Concert USER Report without shared composition, invalid Report mode, invalid Social mode, and Notification invalid runtime mode each exited one with its exact expected refusal. No database connection, listener, valid startup or successful user flow is inferred from these negative checks.

| Copied artifact | Bytes | SHA-256 |
| --- | --- | --- |
| concert-service | 65614464 | `9f0d340ceb99d2803f723014535a1cee46ff48456d8877d9d08a04312d4e595d` |
| notification-service | 34597664 | `b4067372419bb93bbce6824720bbd26bb8829bee8d73f0ae7ab14b2413aa422d` |
| visibility-lib | 100299392 | `f0e491ab9808edc5b11d45dbd2eca863c31ea19db53b03bb8d571e4f6552fb6a` |

Target release was 15:13:52.814552 UTC; FULL release was 15:13:56.571931 UTC, 111.958 seconds after ACCEPT and 3.757 seconds after target release. Positive target growth was 66,449,408 bytes. The whole upper bound was 6,640,861,184 bytes under 8 GiB, including all prior retained artifacts, both failed rounds, all 14 historical target records, full common Git storage, current checkout and the private draft. Final free space was 23,067,250,688 bytes. Owned groups, temporary files and handles were absent; target access after release was NONE. No database or service listener was started.

Evidence directory: `/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-root-world-visibility-coherent-4zzhbid3`.

| Receipt | SHA-256 |
| --- | --- |
| `checkpoint.json` | `8df7a85e8218eba149bc7613f2ac806d2861e46da0f62e23733dbffeac79829e` |
| `target-release.json` | `ab65a73d1f83eba7370207dd8c422f86e3a477bbdd7bf7cfc05d24181a4fff5a` |
| `test-results.json` | `053691f20da10f0c6c20cfa1c26afb340a086ecaf6f7910d730dd133135826d6` |
| `copied-results.json` | `d7a2c2ba699ae5e6d650d043c014234fcfe9844ac3fdc677a32d8663da956306` |
| `full-release.json` | `1a56f4ef0fad8e7df0eef46ef2675e01832c151d11818c64d2c17a17a857e26f` |
| `source-mtime-refresh.json` | `800f9363b389c38a504bd24f34ed4c59d64691e4a71a5b8824ef769ea35141d6` |

These results qualify the bounded source change and actual-main build/refusal behavior. They do not qualify real recipient-owner revocation races, native SQL publication, deployed startup, initial-snapshot delivery, 200-user capacity or QA08 acceptance. The old Green/runtime51 executable and SQL readsets remain superseded. A new current-candidate owning-service/SQL/runtime packet and fresh resource offer are required before another real-200 run. Neither the code review nor the three helper tests establish the RW11 failure cause or a successful capacity fix.
