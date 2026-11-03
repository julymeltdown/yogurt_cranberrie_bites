# Strict Gateway access context — bounded BE04 prerequisite

Status: **IN_PROGRESS**, assigned implementation/verification delivered pending
parent review. Whole BE04,
email-only participation and actual Gateway-to-Concert admission remain
incomplete. This increment supplies Gateway-local verified identity/time input;
it supplies no Auth owner facts, Profile result, event role or portable admission.

## Ownership and source

Assigned worktree `be-gateway-concert-access`, branch
`codex/mcr-be-gateway-concert-access`, clean published base
`b16a679c42ee54c4a3e73575263b390f9334e064`. The integrator alone publishes.
The four delivered paths are:

- `backend/services/api-gateway/src/adapters/out/jwt/concert_access.rs` (new)
- `backend/services/api-gateway/tests/concert_access.rs` (new)
- `backend/services/api-gateway/src/adapters/out/jwt.rs` (bounded exception)
- `docs/mcr/backend/gateway-concert-access.md` (this record)

The existing-file exception exports the new module and extracts one private
generic verified decoder, with a flattened additional-claim projection. The
legacy projection is empty. Its public signature, neutral `VerifiedAccessToken`
shape, optional/empty-session handling, role projection, header/key checks,
RS256/issuer/audience/access-type validation, library leeway and key install/
refresh behavior are preserved. No existing test is edited to fit the change.
There is no unsigned payload reparse or second verifier/key source.

New context/strict tests are newly authored. The cached RSA-2048/RS256 and JWKS
fixture adapts `backend/services/api-gateway/tests/unit/signing_key_support.rs`
at the base above (SHA-256
`dbd1d28988661234f36da0d2ffc7aa751e0f1a26977e083d7a4b3f868ac61066`).
No private production constructor is bypassed; every test context is obtained
through the public strict method and actual configured-key verification.
The imported backend's owning upstream is `julymeltdown/port-rust` revision
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`; see the unchanged
[source map](../source-map.md) and [import manifest](import-manifest.json).

Owning Auth `config/mod.rs::Claims` at this base signs integer `i64` `exp`,
canonical UUID `sub` and access-family `sid`, scalar issuer/audience and
`token_type="access"`. Its SHA-256 is
`2bcbe6909eeacdea2f9ed7f5a67ab8988fa585dd87f7c5ec270e7363405532e1`.
`AuthorityRpcDeadline` is reused unchanged from
`backend/crates/platform/src/actor_delegation/authority_deadline.rs`.

The retained manifest declares jsonwebtoken `9.3.0`; the actual lock resolution
is **9.3.1**, package checksum
`5a87cc7a48537badeae96744432de36f4be2b4a34a05a5ef32e9dd8a1c169dde`.
Inspection of that exact crate shows default 60-second expiry leeway, disabled
`nbf` validation and a NumericDate visitor that can round floating-point numbers
to `u64`. The shared decoder keeps those existing legacy settings. The strict
projection deserializes exact integer `i64` seconds, requires a scalar issuer,
checks conversion to microseconds and checks actual UTC time independently.
No library upgrade, dependency, manifest or lock edit is included. Retained
`backend/Cargo.lock` SHA-256:
`6f4a6388b0aff3b1f835e6f7f0f545f4c9c99a1bc51857a86cbee3daea4a7cd7`.

## Behavior and time contract

`JwtVerifier::verify_concert_access(token, original_stop)` uses the existing
verifier and its shared installed key set. It rejects missing/elapsed original
stops and tokens over the strict-only 16-KiB parse/work bound before verification.
Unknown `kid` remains `Token(TokenRejection::UnknownKid)`, the existing safely
refreshable denial. Remote or embedded header keys, wrong issuer/audience/type/
algorithm, invalid signatures and malformed claims cannot construct a context.

Successful verification requires exact lowercase hyphenated, nonnil Auth user
and mandatory session-family UUIDs. Signed `exp` must be an integer representable
in checked `i64` microseconds and strictly live at use. Missing, fractional,
string, null, overflow or expired expiry fails closed; no arithmetic clamps an
invalid expiry into longer authority. Future expiry is normal. There is no `iat`
requirement or arbitrary JWT maximum lifetime. Optional `nbf` must be an integer
representable in the same units and must not be in the future. Its lower wall
bound is retained and rechecked as well. Foreign actor, provider, Profile and
event fields supply no authority; global roles are discarded, never mapped to
event roles.

Before decoding, the method samples monotonic time first and then UTC. The
captured monotonic JWT stop equals that earlier monotonic sample plus the signed
expiry minus the UTC sample. Verification and sampling time consume authority.
Checked subtraction/addition reject representation failure; they never extend
or clamp the signed lifetime. `AuthorityRpcDeadline::within_original` captures
the effective deadline once, bounded by both the exact supplied original stop
and the captured JWT stop. The exact signed UTC expiry is retained separately.

The context has private fields and no public constructor, `Clone`, serialization
or deserialization. Debug output is fixed/redacted; no raw JWT, roles, claims,
provider subject, Profile field or key material is stored or logged. Identity
accessors, `check`, `remaining` and `rpc_deadline` recheck the original stop,
captured JWT stop and current signed wall bounds. `rpc_deadline` returns the
same captured deadline; a later resource ceiling can only shorten `remaining`.
The signed-expiry accessor reports the authenticated time, not authority to use
expired identity. The context is process-local verified input, not an owner
result, Auth-signed proof, event admission or a transferable capability.

Trusted request entry must capture its original monotonic stop **before** JWT
verification, unknown-key refresh, channel readiness or owner waits. It must
pass that exact stop, never browser-supplied time and never a fresh duration
after an await. If refresh is needed, a subsequent verification must reuse it.
Every slower await and the final signing/dispatch boundary require a fresh
context recheck, along with the relevant owner/evidence/policy rechecks. Holding
copied UUIDs or an earlier successful `check` does not authorize later work.
These resource bounds are not product revocation SLOs.

## Verification

Rust **1.88.0**, locked/offline dependencies, two build jobs, debug info disabled;
retained target `/Users/lhs/Desktop/mcr-build/backend`. Exact commands/exits and
log/source hashes accompany the committed handoff. Commands run from this
worktree with `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo +1.88.0 check --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --lib
cargo +1.88.0 test --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --test concert_access --test unit --test verification_transport -- --test-threads=2
cargo +1.88.0 test --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --doc
cargo +1.88.0 build --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p api-gateway --bin api-gateway
rustfmt +1.88.0 --edition 2021 --check backend/services/api-gateway/src/adapters/out/jwt.rs backend/services/api-gateway/src/adapters/out/jwt/concert_access.rs backend/services/api-gateway/tests/concert_access.rs
python3 scripts/mcr/check-plan.py
python3 scripts/mcr/check-boundaries.py --base b16a679c42ee54c4a3e73575263b390f9334e064 --lane backend --allow-doc docs/mcr/backend/gateway-concert-access.md
git diff --check
```

The new cases use actual RSA signatures and configured JWKS, covering strict
UUID/family shape, private/foreign-claim canaries, live future expiry, missing/
elapsed budget, preverification and later waits, captured JWT expiry with wall
time still live, actual signed wall expiry with a live paused monotonic clock,
legacy-skew rejection, numeric overflow/fractions, future/invalid `nbf`, header
key injection, signature/algorithm/issuer/audience/type rejection, malformed/
huge input, canonical clone/install/rotation/withdrawal at verification and
redaction. Tokio's test-owned paused clock/advance isolates the two clock fences;
there is no production clock override. Compile-fail doctests enforce private
construction and absent serialization/deserialization. Retained Gateway unit
tests exercise the unchanged verifier, signing-key refresh, high-risk sessions
and existing HTTP/middleware consumers; verification-transport tests exercise
the existing loopback client contract.

The final combined command exited **0**: **13 strict context**, **209 retained
Gateway unit** and **13 verification transport** cases passed, with zero failed,
ignored or filtered cases. All three compile-fail doctests passed, exit **0**.
The final locked library check and Gateway binary build passed, exit **0**, with
no warnings. Rustfmt, whitespace, planning provenance and the four-path backend
boundary check passed, exit **0**. Exact ownership/protected-input and shared
decoder comparison are recorded in the handoff receipt. No native PostgreSQL
suite is required or claimed for this local JWT increment.

External receipt:
`/Users/lhs/Desktop/mcr-build/gateway-concert-access-handoff.json`. Final logs use
the `gateway-concert-access-` prefix in that same directory: `check-final.log`,
`relevant-tests-final.log`, `doctests.log` and `build.log`. The initial 13-case
new suite also passed. A follow-up adding retained `nbf` rechecks briefly failed
compilation because its field was inserted at the wrong call site; that
development failure is preserved as `relevant-tests.log`, exit **101**, and the
corrected combined run is separate. No failing invocation is represented as PASS.

## Limits and follow-up ownership

No Concert route/client, HTTP entry wiring, Auth owner RPC orchestration, Profile
projection, event eligibility, provider-binding assertion, admission signature,
production mTLS or accepting Concert policy is enabled. No Auth RPC/schema,
domain type, central config, proto, platform, manifest/lock, frontend or shared
fixture is modified. No database, Docker or global service is started.

Installed-key withdrawal affects the next verification. An already-created
context does not reconsult the key set; the test explicitly demonstrates this
boundary. Key withdrawal across awaits, account/session/provider revocation,
fresh owner/Profile results, event epochs and signed Concert admission need the
parent's explicit composition/recheck protocol. This type cannot certify them
or an instantaneous revocation guarantee. Production stream invalidation,
restore/replica safety and whole-BE04 acceptance remain unqualified. No new owner
wire constraint is asserted or silently treated as solved.

The older full Auth library failure in its unchanged Docker-only restricted-role/
schema fixture remains a separate evidence gap. This Gateway-only increment
does not rerun it or claim all-project tests pass. Parent review/fixes and clean
committed delivery remain required; only the parent publishes to master.
