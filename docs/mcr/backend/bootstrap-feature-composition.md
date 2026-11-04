# SERVER-C03-AVAILABILITY-01

Status: **IN_PROGRESS; validation NOT_RUN** (2026-11-05 KST).

Root assigned the server composition gap after its R7 run served three actual
C03 responses with Feed, Social sharing and mutations disabled despite configured
Intro/Social/Feed production handlers. The existing World settings constructed a
release with only World enabled, release inspection returned before managed
settings loaded, and release generation always emitted mutations=false. The
current C03 contract already permits mutations=true in OPEN/LIVE; it requires
false in READ_ONLY/ARCHIVED. No schema, public version or contract change is needed.

`ConfiguredBootstrapFeatures` accepts actual typed validated bootstrap, World,
Participant, Intro, Feed and Social startup settings. Its constructor has no
feature booleans or environment switches. Missing prerequisite owners fail closed;
Feed requires shared canonical Intro/Participant and Social additionally requires
World and its startup-validated Notification inbox/target. Meet, Gifts and Wall
remain disabled until their actual feature composition is explicitly connected.
Canonical Participant supplies general mutation availability.

These capabilities describe configured serving features, not user authorization.
They do not establish age, consent, eligibility, admission, a private handle grant
or an incumbent World SOCIAL baseline. Root153 and FE209 confirmed this boundary.
A configured Social service may advertise availability while its actual provider
mode still denies age-dependent operations. AgeProviderUnavailable is preserved.

The final release binds these feature decisions into the original validated
source: resources, schema checks, source geometry, selected characters and all
owner/listener/replay identities are retained. The final digest includes the
validated source digest and feature-bound document; an unchanged decision reuses
the original Arc/digest. The incumbent World settings, C03 and Participant use
one computed release object. Actual serving retains Participant's strict equality
between its configured FINAL release pin and this computed digest. Persisted
READ_ONLY/ARCHIVED responses always emit mutations=false/EVENT_READ_ONLY, without
promoting any other unavailable feature.

Inspection now loads and validates managed feature settings before returning the
computed final digest. Root explicitly transferred only Participant's load region
to share the existing closed settings/TLS/key/endpoint validator with an inspection
loader. This loader accepts the already validated base-source release to resolve
the configuration/digest cycle. It does not mutate environment, use a dummy hash,
substitute keys, compose owners, open a database/listener or serve. The main
inspection branch returns before composition. Runtime startup uses strict load().

Exact ownership: new config/bootstrap_features.rs, tests/bootstrap_features*.rs
and this document; config/mod.rs export only; bootstrap/mod.rs mutation readiness
field/default only; bootstrap/release.rs capability/digest/phase derivation only;
world_runtime.rs additive feature constructor after validated_release only;
participant_runtime.rs load/inspection region only; main early feature selection
and inspection ordering only. Party19f83's two main hunks, 205's Ops forwarding,
202's Wall mounting and 20f's motion forwarding remain separate. Sharing's named
publisher input and 207's single deletion composer/listener mount are a separate
handoff and are not evidence of a finished production deletion join here.

Prepared component tests check missing-owner defaults and Arc stability, closed
C03 flags with missing World assets, source/age preservation and changed digests,
persisted phase closure, wrong-event response rejection and real unavailable age
provider behavior. They use explicit metadata fixtures, not product art or real
Auth/admission. Required followup evidence includes a fresh locked offline build,
all corrected Social native cases without diagnostic exclusions, non-test main
inspection and actual C03 runtime availability/denial/stale-pin behavior. Source
work does not claim those checks passed. Root is the named publishing developer;
no king/SL review or acknowledgment is a prerequisite.
