# World transport for Social policy metadata

Status: IN_PROGRESS. Configured transport compilation, four policy unit cases
and 18 synthetic-authority native loopback cases PASS at source
`d36382c1aef212ea4339a17c5c7e56afcbbde07f`. Four existing PostgreSQL cases were
explicitly ignored. Canonical Social composition and public IN03 remain NOT_RUN.

This Root-owned transport implements `SOCIAL_POLICY_V1` from contract commit
`930380deed5c47b4c9eed920a8d9352278f0e90e` on the existing World socket. It adds
no World snapshot fields, listener, queue, Auth identity or Phaser instance.
Default composition supplies no policy baseline. World movement remains
independent; a missing policy baseline never enables private Social services.

The canonical Social owner supplies `PolicyBaselinePort` and its held generation
guard. The socket owner obtains that guard for its canonical binding, installs
the World binding locally, stages the revision, commits the guard, and then
enables baseline handoff. Local Install acknowledgement is not snapshot delivery.
The baseline is transmitted only after both successful guard commit and actual
matching World snapshot handoff, whichever happens last. No network wait occurs
under the database guard. An invalidation crossing commit and local arm cancels
the pending baseline. Failed commit, duplicate install and late arm cannot
restore it. A new stream needs a fresh held baseline.

The publisher shares only the existing socket registry, avoiding an owner-to-
publisher reference cycle. Its crate-private metadata method is intended solely
for the canonical sealed Social-source adapter. Actor/event matching selects
current bindings; frames omit actor, resource, receipt, handle and capability
data. Older/duplicate revisions do not roll back the floor. Native handoff
failure marks the actual subscription terminal, preventing further publication
and invoking the incumbent close path. There is no intermediate application
queue to mistake for delivery. No-active-stream acknowledgement is safe only
with the held-baseline reconnect path.

Four passing unit cases cover commit/snapshot ordering, invalidation crossing
arm, old/duplicate signals, failed guards and unsafe revisions. Two passing new
loopback socket cases cover baseline ordering on the same socket and independent
World movement when guard commit fails. Their authority is explicitly synthetic;
these passing cases do not certify durable Social source or public IN03.

Pending required integration: adapt the original Social owner's sealed signal
and `BaselineGuard` (candidate source `cbc1bbce`, documentation `0372e00b`) after
its developer-owned qualification; mount that owner in actual World startup;
route feature-specific outbox legs and server deadline jobs; qualify actual
durable-source HTTP/WS withdrawal and original frontend IN03. None is implied
by the transport source. Explicit public service availability is also pending;
a World flag or policy baseline is never its replacement.

Formatting and scoped `git diff --check` passed. The accepted
`ROOT-WORLD-POLICY-BACKEND-202611041128Z` round used Rust 1.88, locked/offline,
`-j 2`, debug zero and incremental disabled. Both `concert-service --lib` and
`--test world_ws` no-run selections passed. All 3,686 source/runtime files were
copied and hash-verified; exactly two ordinary Cargo-JSON executables were copied.
The target was released with zero handles before any copied tests. Copied test
commands passed in 0.933 and 9.215 seconds; the loopback suite reports 18 passed
and four PostgreSQL cases ignored. All resources were released 182.124 seconds
after acceptance, 67.056 seconds after target release. No target access followed
release. No PostgreSQL or explicit production main was run.

See `world-social-policy-transport-build.json` for exact commands, executable and
evidence hashes, sampled resource measurements and the final allocation bound.
Actual production World startup still supplies no canonical policy port; this
checkpoint does not claim that a deployed Social baseline is emitted.
