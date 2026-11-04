# Actual Social HTTP flow

Run only against an explicitly disposable runtime owned by the startup/composition
owner. This driver mutates both supplied principals' Instagram handles, requests
and event-local block state. It removes the test handles on success; failure can
leave fixture state for the runtime owner to discard. Use fresh disposable users.
No request is sent without a private plan.

The runtime must register all thirteen frozen Social operations and install the
managed keys, immutable accepted claim provenance, canonical owner authorities,
native resolver and `SocialReadSettler`. Both principals must be real canonical
Auth users admitted to the same active event, with supported, unexpired accepted
claims. The recipient must own the current published Intro named by the plan.
The event must allow normal mutations; READ_ONLY is separately qualified by the
retained native case. Handles must have ordinary metadata rows for both users.

Create an owner-only regular file under `/private/tmp`, mode 0600, with these
fields. UUID values below are placeholders and must be replaced by actual seed
identities. Set the two `MCR_...` environment variables privately; never place
tokens in the plan, command arguments, reports or Git.

```json
{
  "version": 1,
  "isolatedEnvironment": true,
  "disposablePrincipals": true,
  "allowSeedMutations": true,
  "origin": "http://127.0.0.1:8080",
  "eventId": "<canonical-event-uuid>",
  "introId": "<recipient-published-intro-uuid>",
  "introRevision": 1,
  "serviceRevision": "<actual-serving-commit>",
  "seedRevision": "<runtime-owner-seed-evidence>",
  "principals": {
    "requester": {"userId": "<canonical-requester-uuid>", "tokenEnv": "MCR_SOCIAL_REQUESTER_TOKEN"},
    "recipient": {"userId": "<canonical-recipient-uuid>", "tokenEnv": "MCR_SOCIAL_RECIPIENT_TOKEN"}
  },
  "claimProvenance": {
    "kind": "SYNTHETIC_ACCEPTANCE_FIXTURE",
    "issuer": "<explicitly-selected-accepted-issuer>",
    "sourceRevision": "<immutable-accepted-source-revision>",
    "ownerEvidence": "<runtime-owner-provenance-evidence>"
  }
}
```

Use `VERIFIABLE_PROVIDER` only with separately qualified provider evidence. A
synthetic fixture or direct seeded row never establishes external verification.
The driver reports only the provenance kind and reported source/seed revisions;
it does not attest process ancestry or the owner's evidence.

```sh
python3 backend/services/concert-service/tests/social_live_support/http_flow.py \
  --repo /Users/lhs/Desktop/mcr-worktrees/be-social-remaining \
  --plan /private/tmp/<owned-private-plan>.json \
  --report /private/tmp/<new-report>.json
```

All responses must carry no-store. Only the explicit reveal may contain its exact
counterpart canary; metadata, mutation results and denied responses must contain
none of the private handle canaries. Same-key replay must preserve the projected
result. The retained offline C04 oracle validates captured targets and response
DTOs/statuses, rejecting extra fields, duplicate JSON keys and non-finite numbers.
Install the existing `scripts/mcr/requirements-contracts.txt` dependencies in the
selected runtime environment; this driver neither installs packages nor fetches
schemas. Oracle and schema source hashes are included in the report.
The driver rejects redirects, uses direct connections, caps responses at
1 MiB and keeps raw private responses only in memory. Successful HTTP checks
produce `HTTP=PASS`, overall `PARTIAL`, exit 2. Missing setup produces `NOT_RUN`,
exit 2; failed assertions produce `FAIL`, exit 1. Overall success is deliberately
unavailable: seven-role authorization, canonical global Friend blocking,
provider qualification, Root EOF/cancellation/restart, WebSockets, cached product
UI and full migration/process qualification remain separate obligations.

## Social policy stream qualification (0051)

The HTTP driver does not open a World socket and cannot certify this gate. The
startup/socket and frontend owners run it against the same actual principals,
event and serving revision used above. Basic World retains its existing snapshot;
private Social views additionally require an explicit matching `policy.baseline`
frame after that snapshot, with protocol version 1.0.0, current stream/event,
scope SOCIAL and server policyRevision >= 1. There is no client default baseline.

With both streams active, reveal only after fresh authoritative availability and
the current baseline. Revoke consent, replace/remove a handle and block the peer;
observe metadata-only `policy.invalidate` with the affected canonical actor/event
generation. The counterpart must synchronously withdraw the full private tuple,
cancel pending reads and media URLs, and prevent older responses from refilling
the cache. The stream frame must contain no handle, resource ID, actor ID, receipt,
outbox ID or internal signal ID. A generation jump invalidates all private views;
an unknown stream/gap also requires a fresh baseline before private reads resume.

Use a real server-generated request/grant/Intro/claim deadline and wait for
PostgreSQL time to pass it. The startup owner must poll `emit_expired` then
`drain_pending`; an advancing browser clock is insufficient. Restart the server
with its database retained, reconnect, and verify the durable baseline rejects
the old private state. Overflow, unavailable publisher and baseline failure must
close/withdraw private views while basic World remains independently usable.
These observations require real WS and cached-UI evidence from their owners;
the native publisher fixture proves neither.
