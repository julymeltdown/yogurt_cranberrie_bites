# The projection worker reaches the profile service

Status: **FIXED and VERIFIED on the real TLS stack.** `discovery_profiles` goes
from 0 rows to 3 on a live stack.

## 1. The defect

`concert-service/src/main.rs` built the worker's channel as:

```rust
let channel = tonic::transport::Channel::from_shared(endpoint)?
    .connect_lazy();
```

A plaintext channel with no TLS material and no service identity. The profile
service listens on HTTPS and its caller registry admits only the Concert
workload, so every round was refused before it reached the source.

It failed **silently**. `connect_lazy` succeeds — that is the trap — so the
worker started normally. Every round then failed, and because a round failure is
retried by design nothing surfaced. `discovery_profiles` stayed empty forever and
`last_failure` stayed null, so a failing round was indistinguishable from an idle
one.

That is why "did the worker start?" passed while the projection stayed empty.
The assertion in this work is a **row count**, not a process fact.

## 2. The fix

1. **`GrpcDiscoverySource::connect`** uses
   `platform::grpc_tls::lazy_channel_for_edge_with_optional_tls` addressed to the
   `Profile` workload, plus a `ServiceIdentityClientInterceptor` presenting the
   `Concert` workload credential — the same shape as
   `intro_posts::claims_adapter::ProfileClaimsAdapter::connect`.
2. **The settings carry the credential.** `DiscoveryProjectionSettings` now
   requires `CONCERT_SERVICE_CREDENTIAL` and refuses to start live without it, so
   an anonymous worker cannot be configured at all. Its `Debug` is hand-written
   to redact the credential.
3. **`main.rs` captures the TLS material** before `runtime` is moved into the gRPC
   closure, and passes it to `connect`.
4. **A transport failure is visible.** The worker already recorded round
   failures, but nothing proved a *transport* failure reached that path. Two
   tests now do, including one that points a real `GrpcDiscoverySource` at a
   closed port — the same "connect succeeds, every call fails" trap — and
   requires `last_failure = 'profile_source'`.

## 3. The evidence: a row appeared

This is the check the lead asked for — the query and its result, not a claim that
the worker ran.

The gate (`wp10_profile_social.py`) stands up the real seven services on a
private PostgreSQL 16 cluster with TLS, seeds three identities through the real
enrollment and NAVER flows, and runs the product scenario over real HTTPS.

| Query | Database | Before | After |
| --- | --- | --- | --- |
| `SELECT count(*) FROM discovery_profiles` | `mcr_concert` | **0** | **3** |

| Follow-up | Result |
| --- | --- |
| `SELECT nickname FROM discovery_profiles ORDER BY updated_at DESC LIMIT 1` | `WP-10 outsider` |
| `SELECT count(*) FROM discovery_favorite_items WHERE profile_id='818ea818-…'` | **1** (a favorites change propagated) |
| `SELECT count(*) FROM discovery_profiles WHERE account_id IS NULL OR account_id = '00000000-…'` | **0** (every row carries the account id the intro block filter joins on) |

The third row matters because of the dependency the lead described:
`intro_posts::facts::blocks` reads `discovery_profiles.account_id` for both the
viewer and the author and returns a **mutual block — a denial —** when either is
unmirrored. The projection is not adjacent to the intro surfaces; it is the
condition they need. The step asserts that condition directly rather than
inferring it from an HTTP status.

## 4. Verification commands and exit codes

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cd backend && cargo +1.88.0 check --locked --workspace` | **0** | clean |
| 2 | `discovery_projection_cluster.py --test discovery_projection` | **0** | 19 passed (unchanged) |
| 3 | `discovery_projection_cluster.py --test discovery_block_consumer` | **0** | 11 passed (unchanged) |
| 4 | `discovery_projection_cluster.py --test discovery_worker` | **0** | 14 passed (12 unchanged + 2 new) |
| 5 | `discovery_projection_cluster.py --test discovery_source_live` | **0** | 5 passed (unchanged) |
| 6 | `wp03_discovery_tests.py --concert-test discovery` | **0** | 17 passed (unchanged) |
| 7 | `wp02_migration_ledger.py --report /tmp/proj3-ledger.json` | **0** | heads 18 / 92 |
| 8 | `wp10_profile_social.py --bin-dir <abs>/backend/target/debug` | **1** | gate FAIL overall; **step6_discovery_projection PASS** |

### Why command 8 exits 1

The gate as a whole is FAIL because steps 2–4 (the intro surfaces) still fail and
step 5 is BLOCKED. Those are WP-05 composition gaps, not this defect. The step
this work owns — `step6_discovery_projection` — is PASS, and its recorded checks
are the row evidence above.

## 5. Deviations and limitations

1. **No migration was needed.** The fix is wiring; the schema is unchanged, so
   the concert head stays at 92.
2. **The gate's `seed_catalog` was fixed.** It previously only inspected the
   `catalog_items` table and never inserted anything, so the favorites
   propagation check was silently *skipped* rather than failing. It now inserts
   two deterministic catalog rows, and the check runs.
3. **The gate's step 6 is new.** It did not exist before; without it the gate
   could pass its other steps while the projection stayed empty.
4. **The intro surfaces remain broken.** This work removes their precondition
   failure; it does not implement them.
