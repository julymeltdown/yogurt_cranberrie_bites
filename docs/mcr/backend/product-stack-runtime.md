# Original IN03 product stack runtime helper

Status: **IN_PROGRESS**. The source launcher and **17 pure tests pass**. Actual seven-service startup, positive public product behavior and the original IN03 journey are **NOT_RUN**. No execution lease is held. Startup readiness is not IN03 acceptance.

## Ownership, transfer and provenance

IN03-STACK-01 grants exactly these three new paths:

- `backend/scripts/mcr/in03_stack_runtime.py`
- `backend/scripts/mcr/test_in03_stack_runtime.py`
- `docs/mcr/backend/product-stack-runtime.md`

The current writer uses its exclusive backend worktree on `codex/mcr-be-product-stack-runtime`, based on `5f72fcf00fa7ba12599d6417d2f4744c5c49fbcb`. The previous Social branch is preserved. FE209 stopped backend authorship and restored its frontend branch before this transfer. Its unchanged two-file draft `2d7200b5b6b673e7d6a42ed1d3634e9eb77ec573` was imported as `ba3dae545712d60d0b0fed3062c1968872c25e68`; the test file did not exist in that packet. The draft had no runtime or test qualification. Its helper SHA-256 was `a022bb03bfb4380fadd910e63dd353425a5e7d8f1b8cd08eb384cc796ce39789`; its document SHA-256 was `f6f6ab9979cdedd274f1a3ec607576a58a355182d3dca7d5cc31d73df34db48d`.

Existing helpers are imported read-only, without copying their journey probes or altering their files. Root corrected an earlier reference to a nonexistent `Preparation` class: the actual interfaces are `DevStack`, `Cluster` and functions in `world_in01_prepare.py`. The updated producer source is `3a340909d324a2cb06aef9a66ceb717ee80f10be` (Root import `336c3cf4`, published as `ab780c03`). Managed parser mappings were read from Root's `921e0b1b` / `ef6a5853` and subsequent source. They are not writable here.

| Read-only component | Reused behavior |
| --- | --- |
| `dev_stack.py` | Exclusive owned stack, private HTTP mail sink, six foundation services, Unix control protocol, cleanup |
| `migration_smoke.py` | Marker/inode-owned fresh Cluster, actual migrate/verify commands, SQLx checksum ledgers and physical restart |
| `runtime_smoke.py` | Port reservations, PID-owned listeners, explicit environments, generated JWT/delegation keys, child stops |
| `world_in01_prepare.py` | Actual Gateway signup/mail verification/login, public profile initialization, approved catalog/event/room configuration |
| `world_in01_live.py` | Private fixture PKI, TLS mappings, resource/deadline supervision, public C03 transport and safe projection |
| `participant_live_join_smoke.py` | Independent ephemeral Ed25519 seed/verifier pairs |

Root retains shared application/configuration/contract/binary ownership and normal publication. This owner retains helper validation and fixes. There is no king/SL review or acknowledgment gate. The existing goal remains historically `BLOCKED`; it was inspected, not reset or falsely completed.

## Runtime behavior

The launcher admits exactly seven explicit executable pins. Gateway, Concert, Post and Notification require the `IN03_ADVANCED_COMBINED` source/build input designation. Auth, Friend and Profile require `WORLD_AUTH_FOUNDATION`. These designations mean eligible input cohorts, **not positive IN03 qualification**. Every executable has its own path, SHA-256, source revision and bound private producer receipt. Neither a filename nor a receipt label proves compilation; the publishing producer must retain real build/source evidence. Required production composition symbols are also checked in the immutable advanced source; relabelling the old World cohort is refused.

SQLx SHA-384 manifests come from each executable's own immutable Git source, including the advanced Post and Concert migrations. The actual native migrate/verify/physical-restart checks and ledgers must match those manifests. The selected helper Python closure, release bundle and Concert schema probe must match their pinned Git revision in the supplied read-only source checkout. This permits that checkout's HEAD to advance while refusing changed helper bytes or additional shadow Python modules. Approved release resources still pass the existing bundle hash/size/source-binding checks. No migration, main, config, lockfile or contract file is changed by this helper.

Seven-service composition adds actual Concert gRPC/metrics/World listeners to the existing six services, with a single owned cleanup controller. Workload credentials and keys are fresh and independent. Actual mTLS is configured for Auth/Friend/Profile authority and settlement, the shared Participant/Intro owners, qualified Post reads and callbacks, Notification inbox/target/delivery, and Social. Intro/Social/inbox/target are `live`; Feed and Post are `read`; uploader media stays `off`. Requested modes and HTTPS loopback endpoint mappings are checked explicitly. Concert's database readiness schema is parsed separately from the foundation services' peer schemas.

The existing two-room configuration profile provides two seats in each room for three actual actors. This invokes no QA05 probe and makes no QA05 claim. Participants, admissions and avatars are not seeded. Three distinct principals come from real Gateway signup, private HTTP mail delivery, verification and login. Access tokens stay in process memory. Profiles are initialized through actual HTTP. Each actor's actual no-store JSON C03 is checked against the canonical actor/event contract in memory.

No verified-age issuer, provider claim, operator privilege, arbitrary principal, fake readiness, forged availability or application-clock alteration is supplied. Missing producers remain unavailable. The existing producer's `@example.test` addresses identify the isolated private mail sink; they are not renamed to evade frontend admission. The frontend owner has a separately authorized narrow provenance-admission follow-up.

## Private input schema

The launcher takes a mode-0600 JSON file with exactly:

```json
{
  "version": 1,
  "helper_source": {"root": "/absolute/read-only/source-checkout", "revision": "<40 lowercase hex>"},
  "binaries": {
    "api-gateway": {
      "path": "/absolute/frozen/api-gateway",
      "sha256": "<64 lowercase hex>",
      "source": "<40 lowercase hex>",
      "qualification": "IN03_ADVANCED_COMBINED",
      "evidence": {"path": "/absolute/private/producer-receipt.json", "sha256": "<64 lowercase hex>"}
    }
  },
  "pg_bin_dir": "/absolute/qualified/postgresql/bin",
  "redis_binary": "/absolute/qualified/redis-server",
  "openssl_binary": "/absolute/qualified/openssl"
}
```

The abbreviated `binaries` example must be expanded to all seven services, without extras. Each separate mode-0600 producer receipt has exactly `service`, `binary_sha256`, `source_revision`, `result`; `result` is `PASS` for the producer's scoped build/source qualification. This compact receipt binds an already retained build packet; it cannot upgrade negative/component qualification into positive public-flow evidence. The helper parses its closed schema before hashing it and will not hash an unrelated authenticated C03 supplied as a receipt.

The CLI requires every allocation bound explicitly: `--allocation-id`, `--allocation-work-deadline-epoch`, `--allocation-release-deadline-epoch`, `--whole-timeout-seconds`, `--startup-timeout-seconds`, `--provision-timeout-seconds`, `--hold-seconds`, `--whole-byte-limit`, `--retained-carry-bytes`, `--max-owned-processes`, plus `--inputs`, `--report` and the explicit loopback `--browser-origin`. A named allocation argument is not permission to start: the owner must first accept an actual fresh 21d offer for these ready inputs.

The report parent must already be an owner-only directory. Binary reads stream and close their handles; changed hashes/identities, symlinks, shared binary inodes and group/world-writable executables are refused. Private JSON custody checks use owner/mode, no-follow open, regular-file/size and open/read/post-read identity checks. No input values are placed in exception diagnostics.

## FE209 handoff contract

On actual readiness the helper emits the location of private `in03-live-inputs.json`, and writes `<report>.ready.json`. The private descriptor preserves the existing control kind, root/owner/inode/socket fields for `dev_stack.control_request`; `status` and `stop` use the owned Unix protocol. It includes actual `gateway_url`, `browser_origin`, event, rooms, World WebSocket URL, approved release inputs, per-role principal-file paths, exact source revisions, allocation/lifetime and control cleanup ownership. The descriptor expires on owned shutdown. A retained ready receipt is historical evidence, not a live lease; stale control roots are refused.

`authProducer` has the agreed closed versioned shape:

- `version: mcr-local-real-auth/v1`, `kind: CANONICAL_GATEWAY_PRIVATE_HTTP_MAIL`, `tool: world_in01_prepare.py`.
- `sourceRevision`: actual helper source; `binaryRevision`: actual qualified Auth executable source.
- Equal private `mailOwner`/`controlOwner` from the owned Cluster token, actual browser `origin`/`eventId`, actual UTC `issuedAt`/`expiresAt`, `custody: PRIVATE_MODE_0600`.
- Exactly `requester`, `recipient`, `restricted` actors, each bound to the actual `userId`, private `email`, event, `method: GATEWAY_SIGNUP_HTTP_MAIL_VERIFY_LOGIN` and observed `signupStatus`, `verifyStatus`, `loginStatus`, `bootstrapStatus`, all 200.

Independent correlation fields are `source.authProducerRevision`, `source.authBinaryRevision` and `runtime: {controlOwner, origin, eventId, expiresAt, private:true}`. They are private preflight correlations, not independent evidence that self-asserted 200 statuses prove HTTP. The actual browser journey still performs canonical Auth/C03 checks. No producer receipt or private descriptor is a public artifact.

The original entrypoint remains `frontend/e2e/real/social.spec.ts` with `frontend/e2e/real/social/playwright.config.ts`. FE209 owns that journey, same-origin TS serving and production availability wiring with Root153. This backend helper launches no frontend server, browser, new journey or second test harness. The consumer uses `MCR_IN03_REAL_CONFIG`, the named `MCR_IN03_PUBLIC_ALLOCATION` and a private `MCR_IN03_ARTIFACT_ROOT` only under its accepted allocation.

The descriptor truthfully lists missing original prerequisites: same-origin frontend serving, production availability, provider-qualified actual Intro/Post/server-expired-request resources and contract denial dispositions. It never fabricates those IDs or `frontendRevision`/`contractRevision`. The test creates its own comment/request and keeps their IDs in its private cleanup attachment; deleting the owned ephemeral Cluster removes those resources. There is no claim of a privileged meeting or physical arrival.

Full authenticated C03 bodies remain in RAM: no persistence, copying or full-body digest. Only the existing safe public resource/capability projection may be retained, explicitly labelled `NOT_BYTE_EXACT_FULL_C03`; its hash covers projection bytes only. Tokens, identity/preferences/age fields and private actor/control data do not enter that public projection. Actual capability outcomes are unchanged.

## Resource and cleanup boundaries

Actual native work requires the fresh accepted offer and all ready binary pins. The existing guard clamps phase/work/release timing to the original absolute deadlines and reserves 240 seconds for bounded direct-child and PostgreSQL cleanup. It enforces the offered total bytes including historical retained carry, owned descendants, a 16 GiB start floor and 8 GiB continuous floor. Report/output and the new owned Cluster are counted; frozen binaries, input/build evidence and historical growth outside those roots must be included in the allocator-approved carry. The Social owner's previous 221,478,912 bytes of historical growth must not be reset. No Cargo build, target access, cache cleanup, helper installation or implicit resource extension is performed.

All seven service children, mail and Redis use the existing owned cleanup controller. Cleanup is attempted even after a SQL observation or control-close failure. An unavailable/nonzero prepared-transaction observation, failed/forced/unexpected process stop, incomplete cleanup or exceeded hard deadline prevents success. The work sampler is stopped when cleanup begins; direct-child and Cluster stop methods retain their individual bounded timeouts and the final absolute release cutoff is checked. A cleanup failure retains the existing owned evidence rather than deleting files under an unconfirmed process. The helper cannot enforce an external allocator's consent or recover an uncatchable host/process termination.

## Validation and pending qualification

On Python 3.14, source checks run without bytecode writes:

```text
PYTHONDONTWRITEBYTECODE=1 python3 -W error -m unittest discover -s backend/scripts/mcr -p test_in03_stack_runtime.py -v
17 tests PASS; exit 0; 0.131 seconds
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/in03_stack_runtime.py --help
exit 0; no native helper or service startup
```

Pure cases cover binary/hash/inode/custody changes, FIFO/oversize/replaced private input, receipt/source/service binding, refusing a full-bootstrap-shaped receipt before hashing, duplicate/nonfinite JSON, old World composition refusal, immutable binary-revision SQL despite a changed checkout, independent managed keys/callback directions, refusing off/plaintext/unqualified-provider configurations, original work/release cutoffs, Concert readiness consistency, cleanup after control/SQL failure, and private Auth provenance without C03/bearer retention or hashing. Temporary Git/credential facsimiles and mocked transports in these tests are explicitly not live evidence.

Root supplied a new Concert binary on compiled source `e1531f40`, published source `89f26e6edf916cfd5bcdae481c24091597027448`: 61,759,872 bytes, SHA-256 `132166f394b36c16e49e61e6c9702c04b136ca1642930f1d7b5a5f5a65ed2e05`. Its scope is **COMPILED_STARTUP_NEGATIVE**, not positive advanced-stack/IN03 qualification. Its retained packet is referenced by `docs/mcr/backend/social-main-build.json`. Advanced Gateway/Post/Notification pins remain pending; the old World cohort is not substituted. No actual launcher, PostgreSQL, seven-service, frontend, browser, positive provider or original IN03 result is claimed by this document. A ready source handoff may be published normally while the responsible owner retains these runtime duties.
