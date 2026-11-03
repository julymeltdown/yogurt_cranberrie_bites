# BE04 guarded Profile self RPC prerequisite

Status: IN_PROGRESS. This integrator-owned candidate freezes the internal wire and composition required by the next Profile implementation increment. It is not a working endpoint or an accepted task. The temporary unavailable handler must be replaced, independently reviewed and tested before this candidate is published to master.

The base is published master `3c32eb6c933876a4669654269adba53a41689cec`. It includes the seven-path Profile owner candidate `81665a04b579d8709fa6cbe018c46b01db4edce0` as `b3262e2`; that owner is separately subject to integration review. Its exact source bytes are retained.

## Frozen internal interface

`/profile.v1.ProfileService/ReadConcertSelfProfile` uses the existing generated service, configured channel, caller registry and actor guard. Only registered Gateway may call it; field 1 (`user_id`) is actor-bound. Existing methods and the separate Post author-authority service retain their policies.

The request contains one exact canonical lowercase nonnil UUID. A successful response requires `profile`, containing only canonical `user_id`, `display_name` (nonblank, at most 40 Unicode scalars, no controls), optional `bio` (at most 160 scalars, no controls; exactly empty is absent), and `observed_at_unix_micros` from the owner's statement observation. The observation is not a revision, TTL or admission grant. No legacy artwork, username, counts, privacy state, account/session/provider/event authority may be added. The future Gateway consumer must cap decoded responses at 1024 bytes, validate all fields, and retain the original verified request context through consumption.

The assigned handler must require actual `VerifiedCaller(Gateway)`, `VerifiedActor` and the private `VerifiedReadAuthority` inserted by the retained signed actor guard. It must compare both actor identities to the canonical subject. Directly constructed caller/actor metadata alone cannot establish authority. The original call deadline, strict signed proof expiry and existing GatewayProfile local ceiling bound the entire owner await; use the retained edge budget minus `LOCAL_TIMEOUT_MARGIN`, never a newly invented TTL. Check current authority before dispatch, after await and before return, including a late noncooperative completion. Dropping a future does not prove instantaneous database cancellation.

Read the injected `AppState.concert_self_profile`, which wraps the exact minimal owner repository and existing configured pool. Never read through broad legacy Profile DTOs. ContentUnavailable maps uniformly to fixed NOT_FOUND `Profile content unavailable` (absent, deleted, fenced and invalid content); dependency failures map to fixed UNAVAILABLE `Profile owner unavailable`. Invalid canonical subject is fixed INVALID_ARGUMENT, identity mismatch PERMISSION_DENIED, missing verified context UNAUTHENTICATED, elapsed authority DEADLINE_EXCEEDED. Successful responses include `cache-control: no-store`. There is no retry or cache in this handler.

## Ownership and evidence boundary

The integrator owns the proto, complete method/actor policy, AppState field and construction, server delegation, module declaration, and two existing generated-service fixture stubs. The worker owns only the handler replacement, new exact RPC test/harness paths and its lane-specific implementation document; no central file permission is implied.

All additions are newly authored over retained port-rust source `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, destination `backend/`, with provenance in the import manifest. No dependency, lockfile, migration, public HTTP contract, frontend or `docs/source/` changes are made.

Protoc descriptor generation, planning consistency and a manual exact-path/cross-import audit pass as preparatory static evidence only. The worker boundary checker is not an integrator-proto approval gate; an initial invocation without its required arguments returned usage error and establishes no evidence. Rust compile, complete generated trait compatibility, actual signed transport, native PG, deadline/replay/refusal and combined integration checks remain NOT_RUN at this prerequisite commit. The Auth worker currently owns the shared Rust target; this candidate does not take it.
