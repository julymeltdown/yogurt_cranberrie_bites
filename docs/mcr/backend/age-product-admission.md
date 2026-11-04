# BE26 Age Product RPC admission

Status: implementation prepared; build and runtime qualification `NOT_RUN` pending
a fresh resource allocation. This patch admits transport inputs only. Actual
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

No Cargo, target, copied test executable, PKI or listener is used before a fresh
named offer and explicit acceptance. Qualification and final source/evidence
pins will be recorded after that bounded round.
