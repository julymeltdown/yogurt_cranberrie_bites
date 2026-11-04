# Enrollment policy operator: bounded compile qualification

Status: COMPILE_QUALIFIED. Actual execution in Root's existing seven-service
stack, canonical three-actor bootstrap, World continuation and IN04 remain NOT_RUN.
This is an operator artifact handoff, not product-flow or legal approval.

## Source and artifact

Producing source: `dee64fd1ac868f4dffbea49c0e75ca8e98727f3a`.

- Helper: `backend/scripts/mcr/session_deletion_runtime/enrollment.py`, SHA256
  `c4a328cdbb220202ef90502324f9873da5012e045b4f1e8b22d6ba9843760dc9`.
- Test source: `backend/services/auth-service/tests/enrollment_policy_browser.rs`,
  SHA256 `6a1a462a3969222dbfa0b523802edbb1dc870cf4d351311c391d1d48de598ce1`.
- Private retained executable:
  `/private/tmp/mcr-policy207-operator-o25tayv6/enrollment_policy_browser-frozen`,
  57,144,912 bytes, SHA256
  `1c3b16f00dd69ee0305191ed8a310cee3b9f6d5a94f555ec41d7da6713ec113a`.
- Exact source/SQL pins: `source-sql-pins.json` in the same evidence directory,
  SHA256 `67230a34bd42481fcd8513603f8e9b455a91b5f8d84d9afb452c5820716a7554`;
  1,953 frozen inputs, including Auth19 and Gateway17 SQL directories.
- Complete source/API/environment/migration/artifact packet:
  `root-existing-stack-qualified-artifact.json` in the same directory, SHA256
  `1aa99eebf40c82038a132f0a76e416ea8f1ba83ed8f8d06c98c116f2b59bf8d0`.

The only selected Cargo target was:

```text
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p auth-service --test enrollment_policy_browser --no-run --message-format=json
```

Debug and incremental output were disabled; both Rust flags and wrapper variants
were unset. Final Cargo exit0; exactly one selected test executable was copied.
No executable/test case was launched, no PG/database/native/browser stack was
started, and no additional test matrix was selected.

## Failures and corrective validation

The first guardian launch stopped before Cargo because an unused optional PG path
was null. The original failed logs/receipt remain preserved. Removing only that
unused key allowed the unchanged pinned build-only guardian to use its absent
PG default; no PG directory was created, and no limits or deadlines changed.

The first Cargo invocation exited101 after reusing stale shared-target
`auth-application` and `gateway-application` artifacts: the current frozen source
contained the reported missing consent variants, signup method and enrollment
export. The developer refreshed only the byte-identical crate-root timestamps in
its own checkout and retried the exact same command/target within the accepted
lease. Audit records preserve before/after hashes and timestamps. Cargo then
compiled both application crates and the selected test successfully. No cache
was deleted, source modified, target replaced, dependency updated or clock reset.
The independent guardian ran before every actual compiler child. Failed attempt
logs, final output and original accepted metadata remain in the private evidence.

## Resource release and ancestry

`POLICY-PUBLICATION-OPERATOR-MEDIA-20261104T2000Z` accepted at
2026-11-04T20:35:19.210304Z after the literal IntroFix1902 target and guardian-reader
release. Accepted source stayed exactly the producing revision above.

Actual target release: 2026-11-04T20:37:01.829677Z, `target-release.json` SHA256
`b1a4615a01285acfbe66e1b0f63abe575f5bb4aa39cc4a65fe1eaf6fe58d08ee`.
All compiler, target reader/handle and target-reading guardian counts were zero.
It was sent directly to Root1fc, allocator21d and Ops205 before cleanup/docs.
Main's updated critical-path order is RootONEConcert2040, then Ops205Fixture2000,
Party and Gift; the old direct207-to-Ops successor was superseded before release.
The developer never reentered the target after release.

Actual FULL: 2026-11-04T20:37:45.035042Z, `full-release.json` SHA256
`5fb348e76245631fa08b57d7a29256f0a6bab7bc3422dab9c2918a39dd035647`;
`FULL.json` is the same inode. All owned workflows/compiler/guardian/controllers,
children, PG/native/HTTP and copied-executable readers were zero. Inclusive final
NEW was129,019,904 bytes, WHOLE highwater5,281,484,800 bytes; the final accounting
includes the copied executable, which flushed after the guardian's last sample.
Peak process count7, aggregate RSS2,063,663,104 bytes. Original compile/freeze/work/
FULL limits and target-release-to-FULL240-second bound were preserved.

This documentation-only child commit is based on the producing source; it does
not change any implementation or make the failed initial attempts disappear.
The named publishing developer receives this handoff directly; no king/SL review
or acknowledgment is required.

## Existing-stack use and open acceptance

Root uses its SAME actual in03 seven-service stack and actual allocated execution
round. Call `owned_auth_adapters(prep,world,dev,EXE,SHA,frontend_origin)` to obtain
`EnrollmentMailSink`, `principal`, `provenance`. Install the returned mail sink
before Auth; preserve the actual owned verification endpoint/token and add
`AUTH_ENROLLMENT_VERIFICATION_URL=frontend_origin + '/verify-email'`.

The finite ignored case `publish_isolated_policy_into_owned_auth` requires the
actual private caller-owned root/token/inode, PG16 Unix socket/data directory,
existing `mcr_auth` with exactly19 successful migrations, and zero users before
actor1. It publishes only an explicit10-minute synthetic nonproduction policy via
`EnrollmentStore.publish/current_policy`; it creates no DB or identity. No stale
Sessions operator or old857a Auth fixture substitutes for this finite case.

The principal helper then performs actual public policy/attempt/signup/private
HTTP mail/bound verification/login/admission, including explicit current-revision
consent and a fresh admission GET when required. The credential file remains
`in01-principal-{ordinal}.json` with `user_id,email,password,login_path,access_token`;
`access_token` is the literal `MEMORY_ONLY`, and the real token stays in RAM.
No `login_user_id` field was added. Actual Auth verification/login and Concert
ACTIVE admission for managed roles remain the responsible same-stack publisher's
checks; a persisted helper file cannot prove them alone.

Operator execution, real same-cohort requester/recipient/restricted bootstrap,
World/Enrollment continuation, external-mail delivery and product acceptance are
NOT_RUN here. Future Gateway28 own-worktree compilation needs its separate new
finite offer/source freeze and cannot reuse this released turn.
