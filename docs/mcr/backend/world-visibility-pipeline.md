# Ordered World recipient-read candidate

Status: IN_PROGRESS; Cargo, native owner qualification and a new capacity run are NOT_RUN. This candidate is unpublished. It does not establish the cause of RW9, RW10 or RW11.

Root owns only `src/world/production_owners.rs` and this document for this change. Sessions retains its assigned admission-bridge/socket deletion hooks; no PrivateData, Auth, protocol, manifest, timeout, arrival rate or authority policy is changed.

The prior visibility implementation awaited every candidate's qualified Auth/Friend/local observation serially inside one sealed parent. The candidate buffers at most four such futures and collects in the original subject order. Each still constructs its own typed presence projection and uses the existing reader with its original clipped deadline and target attempt gates. Existing local SQL semaphore limits remain in force. Ordered collection preserves native publication-plan/SQL lock order; all observations are checked again before the plan is installed. Any error or cancellation drops the in-flight futures and installs no partial plan. No detached task, new deadline or unverified observation cache is introduced.

Three source tests cover actual bounded overlap plus preserved ordering, failure cancellation without a partial result, and cancellation at one original deadline with all read guards released. They do not qualify a real owner, privacy race, initial-snapshot handoff or 200-user capacity result. Relevant compilation and copied tests require a fresh named allocation; native World recipient checks and a current same-candidate multiuser flow remain separate obligations. The former Green/runtime51 migration inventory is superseded by current Concert 0052 and Notification 0007.
