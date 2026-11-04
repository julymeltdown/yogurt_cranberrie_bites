# Party shared production composition

Status: IN_PROGRESS. Source is authored; compilation, native PostgreSQL qualification and actual main activation are NOT_RUN for this patch. No public functional journey or feature acceptance is inferred.

Task: PARTY-SHARED-COMPOSITION-01. Root owns the new config/party_runtime.rs, config/party_resources.rs and the party_runtime export in config/mod.rs. The existing exclusive Party producer developer owns PARTY-NATIVE-SETTLEMENT-01, its scope-guard native registration, immutable 0057 witness and resolver. The Wall developer owns Wall-only main/config/native-resolver integration; Root has stopped main.rs while that writer is active. No shared checkout is used.

The startup factory uses the existing Concert pool, canonical owner fence and delivery lifecycle, and the same PublicProfile reader. It creates no new identity, listener, transport, owner registry or policy fallback. Off/default reads no feature secrets. Live requires guarded gRPC and the shared canonical composition, explicit independent retained receipt keys and a separate cursor key. The overlay selects exactly the eight existing Party operations; unrelated Product operations retain their previous owner.

Party resources lock the actual event/settings rows in the transaction. Normal writes require enabled parties and OPEN/LIVE; enabled reads also allow READ_ONLY. Safety cancellation/removal remains possible after feature writes stop, subject to the actual existing actor/subject/ownership checks. CLOSED and unknown phases fail closed. The resource adapter refuses every Gift-specific operation. Physical-place selection uses BE09's actual current reviewed physical place and a nested savepoint; successful release retains the owner SHARE locks until outer COMMIT. Virtual position is never physical arrival.

## Provenance

Audited existing source baseline: `7d5d91bd1121b1a95b2302ca02e9224019a5deaa`. No source rendering is reused.

| Source | SHA256 at baseline |
| --- | --- |
| `backend/services/concert-service/src/config/report_runtime.rs` | `f72a7dac3a0aea01a6fc1bc03913de56f41ad07b894b0ea375c0c88cff5745ad` |
| `backend/services/concert-service/src/gift_live/resources.rs` | `48aa4f5b31cb4219fc1493588a932458ea80247e980347c11ece4ebe43238b4b` |

Report startup's closed key parsing and scoped-overlay pattern are adapted into config/party_runtime.rs. Gift resource transaction/place-lock logic is adapted into config/party_resources.rs with Party-only access and no media/meeting grant. Destination dependencies are existing anyhow, async-trait, serde, SQLx, UUID, concert-application, concert-domain and existing PartyLive/ScopedProductOwner APIs; no manifest or lockfile change.

## Required integration evidence

The actual main must load and compose this factory only with the same shared canonical assembly and its qualified Party native resolver. Until the resolver/0057 is integrated, the factory is not mounted. Native rollback/replay/cancellation/drain/restart evidence, real canonical owner and Profile checks, physical place rejection/lock interleavings, and the real public Party flow remain required. Source tests cover configuration/security boundaries and the feature/phase policy matrix; they are not native proof. Performance/load/200-user qualification is DEFERRED_BY_USER and does not gate this functional scope.

Initial source formatting and `git diff --check`: exit 0. Cargo/native/browser: NOT_RUN; fresh scheduler allocation is required before execution. All required unfinished implementation/validation stays active.
