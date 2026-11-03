# BE25 combined introduction and age policy qualification

2026-11-04 KST: **scoped BE25 pure-policy qualification PASS**. The actual combined
candidate compiled and ran the planned domain/application library and dedicated
`intro_age_policy` suites:28 total passed,0 failed,0 ignored. This includes25
BE25 dedicated fixtures and three retained world-ticket library tests. The domain
library suite contains0 tests; it is not presented as additional policy coverage.
No provider, database, API, browser, BE26, BE27 or IN03 acceptance is inferred.

## Candidate, provenance and write boundary

- Exact build/test HEAD: `2e19fd9cff78f954f573fdf8e42a591c71ea29ef`.
- Published source baseline: `17adc599302109a3e9a44fb114c44663fed4c9d4`.
- Checkout: `/Users/lhs/Desktop/mcr-worktrees/be-policy-combined`.
- Branch: `codex/mcr-be-BE25-combined`.
- Sole qualification change: this new document,
  `docs/mcr/backend/intro-age-combined-qualification.md`.
  Its handoff commit directly follows the tested candidate; its final SHA is
  recorded to Main without relabeling the test execution HEAD.

Main serially applied the five authored BE25 commits6bb50539,dbd170bc,80972a8,
4a075a3 and7ca2f730. The11-path candidate contains nine nonshared producer paths
and the two additive crate exports. Their exact original root-export delegation
is recorded in [intro-age-policy.md](intro-age-policy.md). This assignment adds
execution evidence without taking over the original source writer. Backend,
contracts, original docs and original worktrees were read-only. Main alone
integrates and publishes; no source edit or worker push occurred.

The required specification, project assessment and execution plan are unchanged
from the published baseline already read for combined qualification. The original
BE25 document's Cargo NOT_RUN reflected a resource wait; it was not a prior pass.
This report supplies the first actual combined Cargo evidence and leaves that
historical original document intact.

The original producer remains at `7ca2f7305672a784eb1c06325de313b8d2503776`.
All six selected Bigyeting source files documented there were independently
rehashed against the read-only upstream checkout and matched their recorded
hashes at `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`. The exact source paths,
expected/actual hashes and revision are retained in `provenance.json` below.
No additional source code or external dependency was copied by this assignment.

## Contracts and semantic boundary

Current common/social contract bytes match the original BE25 pins. Current
policy is version0.1.2; comparing its JSON against the original task base's
version0.1.0 found only `/version` changed plus added rules
`/rules/mural_own_result` and `/rules/notification_unread_count`. Existing
introduction/age rule content is unchanged. This binds the current contract
without treating the unrelated additive rules as new BE25 implementation scope.

| Current contract | SHA256 |
| --- | --- |
| `contracts/domain/v1/common.schema.json` | `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |
| `contracts/domain/v1/social.schema.json` | `b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2` |
| `contracts/domain/v1/policy.json` | `01c6b9e9a44b81bcf1273ebd7ba8361d3cc035a969399b4dad1f9dedec05839a` |

## Resource refusal, serialized execution and environment

The released mutable target
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target` was explicitly
reassigned for this compile; it was not cloned again. Initial handle/process
checks were clear, but another Cargo/rustc started before strict runner admission.
The first runner correctly refused before launching any command, with
`failure_detail: unrelated Cargo active before build`, null command exit and no
suite execution. Its receipt and runner remain unchanged. This is a resource
preflight refusal, not a compiler/test failure or test result.

A separately named serial runner waited for unrelated Cargo/rustc to release,
then repeated the original strict no-process/no-target-handle admission. Only
then did it use the owned mutable target. It never terminated another process,
modified another target or relaxed admission. Successful evidence is under a new
`run-2/` child rather than overwriting the first receipt. No source fix, dependency
update/install, deliberate cache invalidation or global environment change was
needed. The shared `/Users/lhs/Desktop/mcr-build/backend` and frozen donor/Concert
build targets were not used for writes.

Retained environment: Rust1.88.0 toolchain, explicit RUSTC/RUSTDOC, existing
libprotoc33.4 at `/opt/homebrew/bin/protoc`, PATH with that toolchain plus
`/opt/homebrew/bin:/usr/bin:/bin`, existing Cargo/Rustup homes, `LC_ALL=C` and a new
private TMPDIR. `CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`, and dev/test DEBUG=0;
locked/offline dependencies only. No application/provider credential environment
was inherited. Full allowlisted values are retained in the successful receipt.

The runner polled disk every second and would stop only its own process group
below8GiB or after1800seconds. Minimum observed free space was16,714,887,168 bytes
(about15.57GiB); no floor/timeout termination occurred. Commands ran serially.
The successful runner interval, including admission and pin checks, was
`2026-11-03T18:51:31.506370+00:00` to
`2026-11-03T18:51:41.028213+00:00` (2026-11-04 KST).

## Actual commands and per-suite results

From the clean candidate checkout:

```sh
/Users/lhs/.cargo/bin/cargo +1.88.0 test \
  --manifest-path /Users/lhs/Desktop/mcr-worktrees/be-policy-combined/backend/Cargo.toml \
  --locked --offline -j 1 -p concert-domain -p concert-application \
  --lib --test intro_age_policy
```

Exit0,6.056seconds, including actual compilation and execution. Compiler stderr
records both Concert crates built from this candidate checkout. Counts were
matched to their actual Cargo Running executable paths, not inferred from source.

| Crate/suite | Passed | Failed | Ignored | Filtered |
| --- | ---: | ---: | ---: | ---: |
| concert-application library |3 |0 |0 |0 |
| concert-application intro_age_policy |14 |0 |0 |0 |
| concert-domain library |0 |0 |0 |0 |
| concert-domain intro_age_policy |11 |0 |0 |0 |
| Total |28 |0 |0 |0 |

The same command plus `--no-run --message-format=json` exited0 in1.012seconds to
identify all four exact test executables. Each was copied to the new private
output, SHA-compared against the emitted target artifact and set0500; tests were
not rerun merely to collect hashes.

The dedicated domain fixtures exercised paired/missing/null and half-open bounds,
unknown discovery versus sharing, canonical claim ordering before validity,
expired newest-claim nonfallback, malformed lifetime/ownership and supported age,
calendar/leap/Seoul rollover/expiry, draft/publication/event checks, contact
obfuscation and public-projection exclusion of private bounds/claim material.
The application fixtures exercised exact query/read-lease binding, mandatory
identity/request deadlines, wrong owner/event denial, block directions,
restrictions/membership/revision, direct/list consistency, unavailable ports and
empty lists, unbounded discovery without an age-provider call, safe own status,
expired/future/stale observations, duplicate/oversized pages and retained earlier
row invalidation on later block, claim revocation, birthday or deadline changes.
These are deterministic port fixtures, not real remote authority or verifier proof.

Pinned rustfmt1.88 `--check --edition 2021` on the four owned module roots
(including child modules) and both dedicated test files exited0 in1.013seconds.
It wrote no source. Exact formatting argv and empty output hashes are in the
receipt. No compiler/test failure required an owning-source correction.

## Source and immutable evidence binding

All1,446 tracked backend/contract source hashes were captured before/after the
successful run and rechecked afterward, including crate roots, manifests, lock,
proto/build inputs, contract files and the exact policy implementation/tests.
There was zero drift; the candidate stayed clean throughout execution.
Twenty-eight immutable earlier BE24 evidence/artifact/bundle files were rehashed
before/after and again at final binding, with no changes. The explicitly mutable
reused target's ordinary build products are excluded from that immutable claim;
BE24's privately copied executable, receipts, logs and outputs remain byte-exact.
No old BE24 receipt was rebound to this candidate.

Private evidence root (0700):
`/private/tmp/mcr-be25-combined-2e19fd9-20261104/`.
JSON/log/runner files are0600; copied executables0500. No live account, credential,
gate config, database or service was created. Final binding completed at
`2026-11-03T18:52:07.780261+00:00`.

| Evidence path relative to private root | SHA256 |
| --- | --- |
| `qualification-receipt.json` | `717499d4aa74f84ff756bd83c7d608acc62c4be442955be8d7e2e69eb682564b` |
| `qualify_once.py` | `0ec50ff6b15afef87eb3f4a855bdb115850b24cd848c25c4dd8febeb6c634469` |
| `qualify_serial.py` | `0e773b8c5de951a4261f14be9709167f6ba25fbb61fd413ac6b703fc20f3676b` |
| `provenance.json` | `d15d710d1c38191a5e18102d98425085b78d239711ec27c681db14ffcb5e4698` |
| `final-binding.json` | `0adc189e866fe73fc51f157a4c79d44fe4d1518a78566c8242f8975ac82e5434` |
| `run-2/qualification-receipt.json` | `300c051fe4f96db0307702761aca31f5ef39d8821747eb4a287620fd412d3ce0` |
| `run-2/source-before.json` | `9ce0934aae440183efccd823e8f9ecae4823ba9d7629186397a51386e27973d3` |
| `run-2/frozen-before.json` | `968661c4795af25e063f1381942aab6bce4b9f66a60c557cc325365173bd11e0` |
| `run-2/policy-tests.stdout` | `dbe543dbeed883b3a2fe14dcda46c6b35b84ef331e528de3ce9b84234af1aa9a` |
| `run-2/policy-tests.stderr` | `140ab2dad6b03ba0677cadfb91336dc9a5d1cfcdcfa25947242c7d9f60788f95` |
| `run-2/policy-no-run-json.stdout` | `5e975dee11681c1ff58eea3822ca6beb3093738741fcedc0de30272fa7b7d855` |
| `run-2/policy-no-run-json.stderr` | `916fddd3d45f3b836cfd5b48d4df8f54af503f6fff36874cdf22405316f9f3fe` |

| Copied executable under run-2/ | SHA256 | Bytes |
| --- | --- | ---: |
| `concert-application-intro-age-policy` | `14334da450d4c8f0c0a825f9690c0ab5ce8ebd361de5f7b4314a9b375ed8cdf1` |2721344 |
| `concert-application-lib` | `34c5f6e878cc81ebc99d11e25dedf0f26c7311b28a01d4341dfd7e3e8032ccb0` |1118944 |
| `concert-domain-intro-age-policy` | `5cc91d3a1c9ab37e95258c5ca8a779682880e567ea81b092acb46cf8e01745ae` |1806368 |
| `concert-domain-lib` | `bdbc33e6dc52ede806fcfe183e2a5203f79c4f3f67b0a43776b98a6dc98bdc79` |934400 |

Selected implementation pins (complete1,446-file map is in the receipt):

| Path | SHA256 |
| --- | --- |
| `backend/Cargo.lock` | `208bd6ae3e1657ee5cc6128241bf7fbc39c122efc467efa8deba30b98f2a1dc4` |
| `backend/crates/concert-application/src/age_policy/mod.rs` | `7a47d219bfa90a787fd1f2705556fcab63cee79d38eb100c695c74dc6a528591` |
| `backend/crates/concert-application/src/intro_policy/mod.rs` | `a1c86a896f17d787cb2a8b46f211d819d01144b6bb0f8c567c15ff239365474b` |
| `backend/crates/concert-application/tests/intro_age_policy.rs` | `936b9a734cc83883a43a1d2b553ab9cdd70bd57559303cd804636cbdaae5e563` |
| `backend/crates/concert-domain/src/age_policy/arithmetic.rs` | `6288bf98e724cbc36ee8e3d60303493adb42828d60db57413a44205e45b9a9d1` |
| `backend/crates/concert-domain/src/age_policy/mod.rs` | `8e0c8a2dba17a90075a52ab1a77d62f14a0aacd142e42a46a6c334802dd5f2f3` |
| `backend/crates/concert-domain/src/intro_policy/contact.rs` | `fd1543cea3204ca9a88f4965a6b7b43704f51a35f81c8eeea6b76f53882c1462` |
| `backend/crates/concert-domain/src/intro_policy/mod.rs` | `2923cc4547e57a89daadc49993bf4a5f778bff04738fe3389abf7007ad1e356b` |
| `backend/crates/concert-domain/tests/intro_age_policy.rs` | `6d66c4989ca414efbfeade86938055095aa51a0e0eb60bbc01efa1d0b1e0e74b` |
| `backend/crates/concert-domain/src/lib.rs` | `e4132d9b3b59f3b1b4ae510bcdffb0046dbd4fe4a888121fec223a8cdae15a11` |
| `backend/crates/concert-application/src/lib.rs` | `b5cc1021f17e39c825d410f480e310fe198ea5c57ed0ab431682de64bb332640` |

## Additional checks and acceptance limits

`python3 scripts/mcr/check-plan.py`: exit0, including all13 original archive
checksums and planning/export/task/IA regressions. Explicit staged
`git diff --check`: exit0. Report/source/evidence hash agreement and exact sole
new document were verified before the clean committed handoff. These checks do
not grant application integration acceptance.

This qualifies pure predicates and explicitly synthetic observation/lease ports
on the combined backend. Real verifier/provider callbacks and claims persistence,
PostgreSQL repositories, service authority fences, API/transport composition,
producer/consumer contracts, browser/device/load, real multiuser races, BE26,
BE27, IN03 and release remain NOT_RUN or unaccepted. The tests do not certify a
provider, current real age claim, remote policy lease, complete public DTO or an
introduction endpoint. Main can integrate the passing combined source/report
without an additional general review gate; real owner integrations retain their
own required validation.
