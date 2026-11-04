# Social moderation revocation

Task: BE39-SOCIAL-REVOCATION. Implementation owner: backend Social chat
`01a1a1fe-16e1-7ee3-a9dc-8b5f037cc24c`. Publishing owner: Root
`01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`.

Status: IN_PROGRESS. Formatting/parser and whitespace checks passed. Compilation,
database execution, actual producer recovery, and real browser/World/cache
validation are NOT_RUN. This source does not certify BE39 or IN03 completion.

## Frozen interface and ownership

Source base: `13baed63000b2c058e31e35ce60ab49602f39d94`.
Operations owner 205 froze the source journal ABI at
`3137f0c84af05e0162ced0bdae5b0df921715aba`, migration
`0076_moderation_public_native.sql`. That migration and the actual restriction,
World producer and resolver composition remain 205's scope. Apply 0076 before
0078. A table definition alone is not an applied owner effect.

The actual `moderateConcertUser` / `SUSPEND` command invokes the leaf only for
`EVENT_PRIVATE_AUTHORITY_REVOCATION`. `UNSUSPEND` cannot restore old sharing
authority. The parent BE15 receipt remains PENDING until all required owners
finish; consequently the leaf uses its own journal instead of admitting a
pending moderation command into ordinary successful-outbox publication.

Construct the leaf from the actual existing policy composition:

```rust
let leaf = incumbent_social_world_policy.moderation_revocation();
let outcome = leaf.apply(authority, intent, change).await?;
// Recovery uses leaf.recover with the current acquired authority/scope.
```

`SocialRevocationOutcome` is Pending or Applied(`SealedSocialRevocation`).
The sealed receipt exposes `owner_receipt()`, `source_revision()`,
`transaction() -> &ProducerTransaction`, and `coverage() == "SOCIAL_ONLY"`.
It has no public constructor and does not implement the whole moderation owner.
The aggregate must separately obtain every remaining private-owner and current
read fence before returning whole-step APPLIED. The leaf creates no transport,
registry, listener or background task, and uses the incumbent pool and sink.

Root additionally granted exactly one identifier addition (`moderation_live`)
to the existing reexport in `tests/social_live.rs`, because that target includes
the production Social module by path. No existing test expectation changes.

## Source, progress and acknowledgement

Every attempt requires a real canonical Root scope, a current role and target
role checked by the retained moderation authority, exact command body and
intent digest, unexpired PENDING parent receipt, current SUSPENDED admission,
and the actual preceding restriction and World APPLIED results. Each preceding
result must match its 0076 COMPLETE witness and native identity, actual database
COMMIT and confirmed Root transaction registration. Missing or uncertain source
stays Pending; snapshot authority cannot enter the SQL path.

0078 records an immutable source tuple, source-native identities, a separate
claim tied to the actual current scope/instance/request/deadline, a bounded
audience, and same-transaction native witnesses. Recovery resolves stored native
identities first, then uses a fresh current claim and actual tracked transaction.
An old claim never renews an expired Root scope. Active other-scope claims refuse
concurrent publication. Stored native witness inspection is bounded at 4096.

The event UPDATE lock excludes retained writers and Social read children through
their actual response terminal. Up to 4096 related request rows and affected
actors are admitted; excess remains Pending. Handles and consent history remain
stored; PENDING/APPROVED requests and ACTIVE grants involving the target become
REVOKED. Shared durable policy generations advance in the removal transaction.

Publication uses the same sealed incumbent publisher, checks actual serving
membership coverage, and records each acknowledgement only under a current
claim. A round attempts at most 32 audience rows. Refusal, timeout, cancellation,
lost acknowledgement, source change or expired authority cannot mint a receipt.
Completion requires no pending audience and no remaining active target sharing
authority. The final receipt carries the actual current COMPLETE transaction.

`SocialModerationNativeResolver::new(pool, instance)` implements the existing
`NativeTransactionResolver` port. 205 owns routing its exact native family into
the combined resolver. The reference has exactly seven JSON keys:
`kind`, `owner_receipt`, `command_receipt`, `intent_digest`, `source_revision`,
`scope_id`, `request_id`; kind is `MODERATION_PRIVATE_REVOCATION_V1` and digest
is lowercase hex. Phase PREPARE/PUBLISH/COMPLETE and claim are SQL witness fields.
Commit evidence requires the exact witness and registered identity under the
finished owning Root binding; a committed xid alone is insufficient. Actual
aborted/in-progress outcomes remain independently resolvable.

0051 and 0054 are unchanged, with SHA256 respectively
`193d73496f45cb3c2d5506d386f83a1bfc007d08c44740c7e99a46a745dbb004` and
`06615a4ef00102ac26e735616490afb810f4b80c4d0e87c6184122255e383ad0`.
No Sharing deletion source, ordinary source whitelist, raw metadata authority,
Auth global session or peer implementation is substituted.

## Validation remaining

Developer-owned validation must compile the new target and the existing
`social_live` by-path target; exercise canonical-scope/operation denials; apply
actual 0076/0078 migrations; run actual qualified restriction and World producers
into this leaf; cover current/expired claims, untracked or mismatched identities,
foreign serving membership refusal, partial publication, cancellation, restart,
and the same-intent retry; confirm revocation and no restoration on UNSUSPEND.
Real browser/World/cache coverage belongs to the corresponding combined flow.

No former resource offer authorizes this new build or database run. The previous
Social/Sharing allocation and target access are closed. The allocator has a
request for a new finite validation slot; current outstanding resource use is 0.
No earlier Social/Sharing PASS transfers to this new implementation.
