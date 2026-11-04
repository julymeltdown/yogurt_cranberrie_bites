# Shared Intro startup

Status: **IN_PROGRESS**. The Root-owned startup patch is being composed with the
CF02 shared Participant factory and CF03 signed Product admission. Formatting
and source inspection are complete; compilation, native joined flow and actual
production-process startup for this patch are **NOT_RUN** until a fresh usable
resource allocation. Earlier Intro component results do not certify this patch.

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

Planned validation includes closed settings/secret-role failures, actual signed
Intro read/write dispatch against the shared canonical stores, original EOF and
child settlement, standalone Participant regression, malformed pre-listener
configuration, and production-process shutdown. Results and exact tested
source/artifact pins will replace this pending record after execution.
