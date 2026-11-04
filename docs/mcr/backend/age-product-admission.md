# BE26 Age Product RPC admission

Status: `CONTRACT_READY`; guarded RPC and actual signed mTLS qualification PASS
(21 tests including the existing CF03 regression suite). This patch admits transport inputs only. Actual
provider activation, positive external verification, Root composition, listener
activation, Gateway mounting and full IN03 acceptance remain separate.

Base: `f94602daf07aedb54e620435f008d29a23ed18b8`, branch
`codex/mcr-be-age-product-admission`, in the existing Age runtime worktree.
The original `codex/mcr-be-age-runtime` branch remains at
`763257c66eab0247f1aa8837784312fd7505e130`. The chat's historical full BE41 goal
is incompatible and remains `BLOCKED`; it is not reset or falsely completed.

## Operations and integration API

`product_rpc::AgeOperation::{GetStatus, Start, Callback}` is a public Copy/Eq enum.
Its `id()` returns the frozen contract ID, and `mutation()` is false only for
GetStatus. `Operation::Age(AgeOperation)` integrates with the existing
`operation(event, method, target)` parser and guarded `ProductRpcService`.
Existing constructors and the actual Age producer are unchanged.

| Variant | Method | Event-relative path | Frozen operation ID |
| --- | --- | --- | --- |
| GetStatus | GET | `/me/age-status` | `getConcertAgeStatus` |
| Start | POST | `/me/age-verifications` | `startConcertAgeVerification` |
| Callback | POST | `/me/age-verifications/callback` | `completeConcertAgeVerification` |

Provenance is `contracts/domain/v1/{operations,social.schema,common.schema}.json`
and the existing `age_provider::runtime::AgeRuntime` at the base above.
The assignment originally counted four public operations. Root corrected this
to the three evidenced frozen operations: private erasure is not a public
contract. No erase endpoint, contract or producer branch is invented.

Admission requires exact event path, uppercase method and no query, fragment,
trailing slash or encoded alias. Status has no body, content type or idempotency
key. Both POSTs require a canonical nonzero UUID key, exact `application/json`
and at most 8192 body bytes, matching the existing producer's wire ceiling.
The signed original bytes remain unchanged.

Start requires an object with exactly `return_target: "AGE_PRIVACY"`. Callback
requires exactly a canonical nonzero `verification_id`, `state_token` (1–512
Unicode characters), and `provider_response_token` (1–4096 characters); controls
are rejected. Top-level JSON sequences, unknown/missing/null/wrongly typed fields,
duplicate fields including escaped aliases, malformed UTF-8 and trailing JSON
are rejected before ProductOwner dispatch. Typed deserialization reads the
original bytes after the object-shape check, so duplicate keys are not lost.

Admission is not provider proof acceptance: callback tokens satisfying wire
bounds may still fail the unchanged producer's stricter state-token format,
provider-response byte bound, binding, expiry or independent verifier checks.
No public identity, DOB, issuer, accepted trust source or verified proof is added.
OwnerMedia explicitly refuses Age, and all original signature, deadline,
canonical owner and delivery requirements remain in force. Authoritative
unavailable status still requires the real Age owner and delivery composition;
no transport fallback returns a successful provider or status response.

## Validation and ownership

The new `tests/age_product_admission.rs` reuses the existing CF03
`product_feature_admission_support` fixture, signer, frame, workload guard,
deadline layer, recorder and inode/token-bound PKI loader without changing them.
The recorder checks exact actor/event/session/request, sealed signed parent,
operation and original deadline, then deliberately refuses. An ignored case
uses an actual loopback mTLS listener: Gateway reaches admission; a trusted
Friend certificate cannot impersonate Gateway. No PostgreSQL or peer services
are needed. The existing CF03 ten-operation suite is the regression target.

Assigned production changes are only `product_rpc/{mod.rs,query.rs,owner_media.rs}`
and new `product_rpc/age_admission.rs`. Additional paths are only the new test
and this document. There are no main/config, provider, schema, manifest, lockfile,
proto, contract, CI or frontend changes. Root owns shared composition and the
live opt-in; the Gateway owner owns subsequent public mounting.

## Qualified result

Qualified application/test source: `547eef93f72d3a72ae4af639d0aede7d3bbcfc2d`.
The final documentation successor records the results without changing executable
source. Implementation commits are `a2a5940bf90b677707c042f1f0735ffcb0503898`
and `547eef93f72d3a72ae4af639d0aede7d3bbcfc2d` from the base above.
Only the six assigned paths changed; the original Age branch remains preserved.

| Check | Result |
| --- | --- |
| Rust 1.88 formatting of new Rust files; exact six-path scope; frozen three-operation registry mapping; `git diff --check` | PASS; source checks only |
| Locked/offline exact two integration test artifacts, jobs 2, debug 0, incremental 0 | PASS; final Cargo command 19.97 seconds |
| Actual signed guarded Age RPC tests | 9 passed, 0 failed; 0.25 seconds |
| Actual signed Gateway/Concert mTLS admission and Friend certificate-role refusal | 1 passed, 0 failed; 0.02 seconds |
| Existing CF03 signed guarded regression suite | 10 passed, 0 failed; 0.63 seconds |
| Existing CF03 actual mTLS regression case | 1 passed, 0 failed; 0.03 seconds |
| Fresh private PKI cleanup, joined listener tasks, owned process/copy-handle and TMP cleanup | PASS |

The ordinary runs each initially ignore their one PKI-dependent test. The same
frozen executables then run those exact tests with `--ignored --exact`; both
pass, giving 21 executed tests in total, with no remaining ignored required case.
No PostgreSQL or external provider runs. Recorder responses, including sample
unavailable-status DTOs, cannot pass the real canonical delivery requirement;
these tests do not claim an authoritative provider-status or positive owner journey.

The exact no-run command from `backend/` was:

```sh
cargo +1.88.0 test --locked --offline -p concert-service \
  --test age_product_admission --test product_feature_admission \
  --no-run --message-format=json \
  --config profile.dev.package.contracts-build.opt-level=2
```

Environment explicitly set `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, the allocated BACKEND
target, and an owned TMP directory. Cargo's automatic ordinary Concert binary
and dev-dependency compilation are included in the retained Cargo JSON and whole
growth accounting; no explicit main, library or peer binary build/run was requested.

The final Cargo invocation emitted both test artifacts with `fresh=false`.
The actual Concert library was rebuilt (`fresh=false`) during the retained
earlier attempt at this same source and reused (`fresh=true`) in the final retry.
Dependency reuse is not described as recompiling all sources. All 2313 selected
tracked backend/contract/document file hashes were checked again before copied
execution; the full selected source archive, runtime/tool pins, complete Cargo
JSON and fixture hashes remain outside the shared target.

The existing `backend/scripts/mcr/product_feature_admission_smoke.py` supplies
the PKI and bounded test runner. A private copied controller changes only its
exact ignored-test filter for the Age artifact; the CF03 regression uses the
original runner unchanged. Each copied workflow is SHA-256 checked, runs the
ordinary suite and exact mTLS case, then removes its inode/token-bound PKI.

### Preserved failures and resource release

Fresh lease `AGE-PRODUCT-ADMISSION-BACKEND-202611041132Z` was explicitly
accepted at 11:34:05.758 UTC on 2026-11-04, with the original compile/freeze
cutoff 11:44:05.758, copied-test cutoff 11:46:05.758 and full-release cutoff
11:49:05.758. The supervisor installed process, absolute-time, free-space,
whole-growth and copied-output bounds before spawning children. Maximum
descendants were 20 for compilation and 5 for copied work; at most one ephemeral
loopback listener ran at once. All deadlines and original carry were retained.

Two attempts failed because cached custom build-script binaries from newer
checkouts requested `concert_delivery.proto`, absent from this frozen source.
The actual tracked scripts request `concert_inbox.proto`. Exact resource
amendments authorized one byte-identical mtime refresh of this worktree's
Concert `build.rs`, followed by one of its Notification dev-dependency
`build.rs` after the corresponding actual failure. Before/after SHA-256 receipts
match. No file content, proto, fingerprint, cache/dependency artifact or other
checkout was changed or deleted; the optional API Gateway repair was unused.
Both failures and cumulative positive growth remain preserved.

A retry preflight also refused a global compiler snapshot before spawning Cargo;
its retained correction scopes the compiler check to the offered BACKEND target,
consistent with the separately permitted MEDIA allocation. A release-receipt
assertion initially demanded the library rebuild in the final retry rather than
recognizing its prior same-source rebuild. The copied-run guard refused to start
until this receipt was corrected using retained Cargo records and kernel-wide
handle enumeration. No copied child ran during either guard refusal.

Target release was unconditional. The final release receipt at 11:40:52.300 UTC
records zero target handles and BACKEND compilers before all copied tests;
kernel-wide `lsof` filtering avoided further target filesystem access after the
release message. Both test executables were already frozen outside the target.
There was no subsequent target access. The complete copied workflow finished
in 4.889 seconds. Full release at 11:41:40.610 UTC records no owned process,
listener or open copy handle, removed PKI and TMP roots, and no remaining claim.

Conservative whole peak was 779,894,784 bytes, including all 633,774,080 bytes
of retained Age/BE26/BE41/event-cleanup carry, 74,022,912 bytes of cumulative
positive target growth and approximately 72 MB of retained source/runtime,
executable, controller and proof output. This stayed below the 3 GiB cap.
The two executable copies total 38,426,976 bytes, below 160 MiB. Minimum
observed free space was 20,090,150,912 bytes, above the continuous 8 GiB floor;
initial available space exceeded 16 GiB. Accounting is not reset on retry.

### Evidence pins

Private evidence directory: `mcr-age-product-admission-tnni20tq`, located through
the local `mcr-age-product-admission-current` locator. This contains failures,
metadata-repair receipts, source/runtime pins, copied runners and complete logs.
It contains no provider credentials or real verification proof.

| Artifact | SHA-256 |
| --- | --- |
| Age test executable (19,153,600 bytes) | `680114816e6758d3bade541e2d359e593e25c7223a2ff177a08f0afcea8d81ec` |
| CF03 regression executable (19,273,376 bytes) | `ffa80f032d549e8d9a0ee0752c75bbc498b46876617afdfb3c75f015eb441a28` |
| Frozen artifact/source binding | `36ea1de0b5b580cda52f4397e6fdb7bf71b3b974407f819bb46f9751cb9e4efb` |
| Full selected source archive | `6d01d229edc9a46d88d6fcc5a52bc621e2126cf7c75a7f05eba7ae34c52528cf` |
| Target release receipt | `3db6d48fd4de9ce1bf01b453515c09b06961dc28eb11f4e9bf32fc4798ae587c` |
| Combined qualification receipt | `69754502df99f6bed41e5034af0544f75175492db11792f0cba2ac31ed21c3f5` |
| Full release receipt | `113f2d95f02c347abb9d36b0a6077751cdbaad426cb7b43b04e18bae6366e711` |

Root is the designated publishing developer for this committed handoff. The
implementation owner checks the published postimages and remote ancestry after
publication; that check is separate from provider activation or IN03 acceptance.
