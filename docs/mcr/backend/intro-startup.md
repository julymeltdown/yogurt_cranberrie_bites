# Shared Intro startup

Status: **INTEGRATED**. The Root-owned startup composes the qualified CF02 shared
Participant factory and CF03 signed Product admission. Fresh compilation and
the actual production Concert process passed at
`22497af9c315d4d74403467a2529ed95b5b79f06`. This qualifies the configured internal
service composition; public Gateway HTTP, browser behavior and full IN03 remain
separate open checks. Earlier component evidence is retained separately.

`CONCERT_INTRO_MODE` accepts `off` (default) or `live`. Live mode requires the
existing guarded gRPC and live Participant settings, including its canonical
producer instance, release digest, retained child keys, dedicated signatures,
workload credentials and real TLS. It uses CF02 `compose_shared` to construct one
canonical core, registry, ledger, fence, delivery publisher and closed native
resolver union. Standalone Participant mode retains its original composition.

Additional explicit settings are:

| Setting | Purpose |
| --- | --- |
| `CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT` | Existing exact SELF owner, also used for public Profile |
| `CONCERT_NOTIFICATION_GRPC_ENDPOINT` | Actual configured Notification owner for the common typed child settler |
| `CONCERT_INTRO_CURSOR_KEY_HEX` | Nonzero 32-byte lowercase hexadecimal Intro cursor material |
| `CONCERT_INTRO_RECEIPT_KEYS_JSON` | Active then retained receipt keys, closed `{id, secret_hex}` objects |
| `CONCERT_PICKUP_KEYS_JSON` | Active then retained pickup keys for the common typed Gift child settler |
| `CONCERT_CHILD_RECOVERY_*` | Existing bounded system recovery settings |

Both owner URLs require explicit HTTPS host and port, without credentials,
path or query. Key arrays contain one active and up to 31 retained keys; IDs
must satisfy the owning key constructors. Cursor, receipt and pickup secrets
must differ across active and retained entries. Missing material fails startup;
there are no default keys or invented endpoints. Off mode does not read these
optional inputs. The common factory's Notification/Gift adapters do not enable
their routes or authorize their content.

The configured Product owner admits only the six frozen Intro operations to
the existing `ScopedProductOwner`. Other product families remain unavailable.
The original signed request, two-second Product budget, real canonical fences,
registered native transactions and terminal HTTP delivery still apply. Main
mounts the shared settlement and delivery service once, preserves retained-body
middleware, and supervises both Participant recovery and the typed canonical
child recovery worker under the existing shutdown supervisor. The child worker
settles only durable finished-root children; it does not renew content authority
or recreate a lost live root.

Age proof production remains explicitly unavailable in this configuration. The
actual `AgeRuntime` uses `ProviderMode::Unavailable`, with no accepted issuer or
fabricated verification. Unrestricted Intro behavior may operate under the real
owner policy; age-restricted projection remains fail closed. Configuring this
runtime does not change Bootstrap capability declarations, enable all Meet
features, establish a successful Notification inbox, or close IN03.

New code is authored in this repository. It composes the existing audited
Intro, Age, canonical-owner and Participant implementations without copying
legacy rendering or changing dependencies, migrations, contracts or source
archives. CF02 and CF03 retain separate implementation ownership and evidence.

Fresh validation used the named finite
`ROOT-INTRO-COMBINED-MEDIA-202611041008Z` allocation. Exact evidence, committed
source inventory and copied Cargo artifact identities are recorded in
[the Root startup receipt](root-intro-startup.json). No dependency, migration,
contract or frontend change was needed for this startup patch.

| Check | Actual result |
| --- | --- |
| Rust 1.88 locked/offline compile, jobs 2, debug 0, incremental 0 | Exit 0; exactly the `intro_runtime` and `intro_startup` test artifacts plus automatically emitted production Concert main were frozen |
| Closed configuration and retained secret-role cases | 5 passed, exit 0 |
| Actual production executable with real Auth/Friend/Profile owners | 1 native test passed, exit 0; owner services in-process, Concert in its actual separate production process, genuine signed mutual TLS |
| Six Intro operations and Participant before/after | Real set, publish, own, list, detail and close; two Participant reads; eight delivered roots on one configured instance |
| Response terminal | Inner gRPC EOF retained each active root; the mounted delivery service completed all eight; zero pending child intents |
| Native persistence | Auth/Friend/Concert/Profile migration ledgers 18/18/38/9; all four stores had zero prepared transactions before teardown |
| Startup and shutdown | Missing required Intro material failed before a Concert listener; configured process exited 0 after its owned SIGTERM and listener closed |
| Resource release | Target zero-handle release preceded every copied test/PG/listener; host, postmaster, process group and owned roots absent at full release; no later target access |

Maximum sampled whole allocation was 1,119,145,984 bytes, including the
862,949,376-byte conservative retained artifact carry and 56,614,912 bytes of
fixed target growth, below the 2 GiB cap. Target-to-full-release time was
108.367 seconds, below the copied phase's 180-second allowance including its
60-second cleanup reserve. The runtime minimum sampled free space was
19,223,232,512 bytes; the compile guard enforced the 8 GiB floor but did not
store its minimum. Sampling does not prove unsampled peaks.

CF02 separately qualified shared and standalone Participant GET/PATCH, original
expiry, missing-key refusal and typed child recovery; CF03 separately qualified
ten guarded admission cases and one genuine mTLS case. This joined run adds
actual production startup and six positive Intro operations, without claiming
a successful Notification inbox, an accepted age issuer, independent owner
processes, a public HTTP/browser journey, production deployment or full IN03.
