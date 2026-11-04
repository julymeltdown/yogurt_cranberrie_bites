# SERVER-C03-AVAILABILITY-01

Status: **Four component tests and actual main build PASS; public C03 runtime NOT_RUN** (2026-11-05 KST).

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


Actual component disposition: source `46defd2346e2e73641c552b408623ce357500c15`
compiled with the five-test Social/Sharing cohort, including actual non-test Concert
main, exit 0. All four `bootstrap_features` cases passed, exit 0, in 0.09 seconds.
Report `/private/tmp/mcr-social-sharing-f_ut5nyi/features-report.json`, SHA256
`b4f4b2839951b4479cd87a7ea7432c75615907f22e808770beb058f68135da61`.
The final six executable hashes, source and tool readsets are frozen in that
root's `all-artifacts.json`. Actual final main inspection, stale final-pin startup
refusal and current public C03 availability/operation denial are **NOT_RUN** here.
Root supplied its retained R7 recipe as a launch reference; the old seven-main
cohort and its old C03 replies do not validate this changed source. The named
publishing developer retains the current stack/runtime integration gate.

Gift's implementation owner supplied a concrete future interface:
`GiftStartupSettings::load(bootstrap, participant, intro)` immediately after Intro
settings, and opaque `GiftFeatureReadiness::is_enabled()` obtained only through
that settings object's `readiness()`. Its API was uncommitted during this frozen
round, so Gifts remains unavailable in this source. A separate additive typed
join is required after that committed factory is available; it must precede the
same one release bind and must not grant Gift authorization or enable an off mode.
