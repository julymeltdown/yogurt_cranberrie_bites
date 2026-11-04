# BE13/BE14 functional recovery continuation

Status: IN_PROGRESS. Current base is `6d351502bcee839e0836955edbf5eff61ab9b0b7`.
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
| Foreground without motion replay | Existing 15 recovery cases and correlated actual socket FOREGROUND resync; new owner-level held/pending movement reset and stale-input rejection. Actual browser lifecycle remains the frontend owner's boundary. |
| Restart, ticket replay, switch and ghosts | Retain runtime51 public real-Auth pair attempt3, documented in `world-capacity-qualification.md` and `qa05-runtime51-fault-probe-intake.json`: replay refusal, application gap, read pause, non-full switch, actual restart/changed epochs and reconnect; zero final memberships/prepared transactions. Client read pause is not physical network backpressure or browser backgrounding. |
| Expired/removed/suspended subscriptions | Retain recovery tests and qualified remote owner/native revocation evidence in `world-remote-invalidation.md`; Sessions deletion production composition remains its named owner's BE42 boundary. New code does not alter authority or extend expiry. |
| Motion/legacy compatibility | Reuse recorded 195 selected passes and two real-Auth motion/legacy pairs at source9c12, published1e20; changed runtime regression checks are separately required below. |

Historical evidence keeps its original source and fidelity. It is not relabeled
as fresh execution of this candidate, full QA05/browser/network/device acceptance,
combined-main qualification, or a capacity claim. Frozen `docs/source/` is unchanged.

## Candidate validation

Rust 1.88 parser checks for all four changed Rust files and `git diff --check`
passed before the implementation checkpoint. Native build and focused regression
execution are NOT_RUN pending a finite existing-target allocation. All validation
and resulting fixes belong to this implementation owner; Root publishes the
validated committed handoff without a king/SL review or acknowledgment gate.
