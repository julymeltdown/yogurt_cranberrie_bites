# BE13/BE14 functional recovery continuation

Status: INTEGRATED — developer validation passed; committed publication pending. Current base is `6d351502bcee839e0836955edbf5eff61ab9b0b7`.
This continuation preserves published World motion `1e20c294` and the existing
recipient, admission, replay and recovery implementations. QA08 performance and
200-user qualification remain DEFERRED_BY_USER.

## Corrected behavior

The production owner calls replication refresh on maintenance ticks even when a
socket is not writable. Previously, a terminal refresh replaced the selected
`resyncRequired` disposition with generic `POLICY_UNAVAILABLE`/4500. This lost
`GAP` and `BACKPRESSURE`/4409 precisely on the normal maintenance/slow-reader path.
Replication now retains its original terminal reason/code independently of its
one-shot control payload. Maintenance attempts only a ready bounded handoff,
closes with that exact disposition, and removes the native subscription/motion.
Failed persistent cleanup retains the existing exact-generation retry obligation.
A copied maintenance roster skips peers already removed during that cleanup;
a terminal peer cannot stop the whole controller through a stale binding.

No wire schema, ingress/admission policy, credential deadline, public owner API,
Session/deletion handle, dependencies, manifests, assets or frontend changes.
Only the two World source leaves and their runtime/socket tests change. Source
is newly authored MCR code; no external rendering or logic was copied.

## Required behavior and evidence boundaries

| Requirement | Evidence retained or added |
| --- | --- |
| Differently blocked viewers, per-delivered sequence and avatar revisions | Existing 12 replication cases; new owner-level three-principal coalescing and distinct-filter fresh-baseline case. |
| Bounded slow receiver and gap recovery | Existing single coalesced queue/frame/byte/age limits; new exact maintenance GAP control, blocked snapshot BACKPRESSURE close, persistent cleanup retry and fresh-admission cases. |
| Foreground without motion replay | Existing 15 recovery cases and correlated actual socket FOREGROUND resync; new owner-level held/pending movement reset and stale-input rejection. Retain genuine trusted Chrome background/foreground evidence at three widths from frontend publication `980fc4d8`, `docs/mcr/evidence/realtime/world-client/publication.json`: one fresh connection, first input1, neutral inputs and no old interaction replay. Its peer is controlled, not Rust/Auth. |
| Restart, ticket replay, switch and ghosts | Retain runtime51 public real-Auth pair attempt3, documented in `world-capacity-qualification.md` and `qa05-runtime51-fault-probe-intake.json`: replay refusal, application gap, read pause, non-full switch, actual restart/changed epochs and reconnect; zero final memberships/prepared transactions. Client read pause is not physical network backpressure or browser backgrounding. |
| Expired/removed/suspended subscriptions | Retain recovery tests and qualified remote owner/native revocation evidence in `world-remote-invalidation.md`; Sessions deletion production composition remains its named owner's BE42 boundary. New code does not alter authority or extend expiry. |
| Motion/legacy compatibility | Reuse recorded 195 selected passes and two real-Auth motion/legacy pairs at source9c12, published1e20; changed runtime regression checks are separately required below. |

Historical evidence keeps its original source and fidelity. It is not relabeled
as fresh execution of this candidate, full QA05/browser/network/device acceptance,
combined-main qualification, or a capacity claim. Frozen `docs/source/` is unchanged.

## Candidate validation and handoff

Compiled and executed source: `b966f0ff4368d8ce4b4f60bfd4098282657deb75`.
The later documentation commit changes no compiled source. Rust 1.88 parser
checks for all four changed Rust files and `git diff --check` passed.

Build command (exit0, 50.376 seconds):

```sh
CARGO_TARGET_DIR="$MCR_SHARED_WARM_TARGET" CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j2 \
  --config profile.dev.package.contracts-build.opt-level=2 -p concert-service \
  --test world_runtime --test world_replication --test world_recovery \
  --test world_ws --test world_motion_owner --test world_motion_live \
  --no-run --message-format=json
```

Rust flag/wrapper variables were unset. The six exact own-checkout build.rs/mural
metadata refreshes used Root's explicit exception: bytes, inode, mode, owner and
size stayed unchanged. No target clean or peer checkout change occurred. Cargo
also built its ordinary implicit binary prerequisite; it was not copied or run.
Six private copies total68,331,296 bytes. Target handles were zero on release at
2026-11-04T19:47:34.739042Z; build FULL was19:47:35.282231Z.

A separate allocated copied-native round ran each executable with
`--test-threads=1`, the existing contracts-capable Python on PATH, and
`PYTHONDONTWRITEBYTECODE=1`. All2589 source inputs and six executable hashes
matched before and after execution. No compile, target read, PostgreSQL,
application stack, browser, new executable copy or install occurred in this round.

| Executable | Result / exit |
| --- | --- |
| world_runtime | 35 passed /0 |
| world_replication | 12 passed /0 |
| world_recovery | 15 passed /0 |
| world_ws | 20 passed,4 pre-existing PG cases ignored /0 |
| world_motion_owner | 5 passed /0 |
| world_motion_live | 2 passed /0 |

**89 passed;0 failed;4 ignored/NOT_RUN**, 20.273 seconds overall. Native ACCEPT
was2026-11-04T20:01:08.447889Z; actual FULL was20:01:30.458294Z. Both controls
passed; all owned groups/guardian were released, copied binary handles were zero,
and no owned PG existed. Native peaks:65,536 new bytes,5 processes,197,394,432 RSS
bytes; measured free space stayed at least12,790,706,176 bytes. The next named
resource owner received FULL before this evidence edit.

The five-path handoff contains only `world/replication.rs`, `world/runtime.rs`,
`tests/world_runtime.rs`, `tests/world_ws.rs` under Concert service, and this lane
document. The two source leaves preserve public APIs and existing authority
fences; no cross-lane import was added. Root1fc is the normal publisher. There is
no king/SL review or acknowledgment gate. The developer owns any actual failures.

This closes the identified backend maintenance/recovery correction and its
focused functional cases. Whole QA05 additionally requires the new frontend
owner's actual App/Rust/Auth fault-convergence journey through the shared
producer/adapter. Its currently read-only observer supplies no restart or traffic
fault action; the actual owner must provide an authorized control seam rather
than starting extra probe clients beside a capacity-two App pair. Source/driver
preparation proceeds independently. BE42 full deletion composition, full combined
main/IN acceptance, physical device/network and performance claims are outside
this bounded handoff. QA08 stays DEFERRED_BY_USER.

Private immutable build evidence: `/private/tmp/mcr-world-recovery-completion-20261105/`.
Private copied-native evidence: `/private/tmp/mcr-world-recovery-native-20261105/`.
All old World/QA05/motion receipts and their failures remain intact.

- Build `plan.json`: `03a22f71785c9920fc7a0a264658df13ac34e02c7f279b2d3798ca1c0a093376`.
- Build `source-pins.json`: `00fc9edb1be7af3720d1f2461d5a24e215394921eae2cc38a668736cfc89d0be`.
- Build `checkpoint.json`: `2711d7f251327d1e6dba33aea9816e8cb71420e786252736e3d993eab17aec99`.
- Build `compile-0-attempt-0-receipt.json`: `d43605f0b393986defd661a9d75c19ecff207b95636358c75c69dd5b1eb0647c`.
- Build `full-release.json`: `f0243054e327a7178e5f118629e282edfcc93b5ff2c0f7d380f6f5d9cf70ae6c`.
- Native `plan.json`: `21f62b7a5d1e42f48b13ae4c54dff68c38c28a068895c15c08029c38a573553b`.
- Native `copied-results.json`: `adcd00fff433eff331fa582daac1ad0c12c7cc75a53d3feb6899866ce5a8dedc`.
- Native `guard-receipt.json`: `3bbd930ee3833ffa41c31c2d01afe79334a075badeb201d45b47dc9e896410c8`.
- Native `full-release.json`: `88350067889a301206c847ab1348bf42546cc6c841dd5eb7f34c52d040244026`.
