# Root composition intake

Status: IN_PROGRESS. These are local integration source changes, not a serving
runtime or task acceptance. Existing immutable qualifications retain their own
source pins. Fresh coherent compilation and relevant native/public checks remain
required before these changes are published as qualified runtime work.

## Reviewed motion avatar v2

Producer base `980fc4d8`, required candidate Rust prerequisite `83b23afd`, and
ordered payload through `5f426f8e` were integrated serially. The implementation
and native-test paths match the qualified producer source `917b2d80` exactly;
central domain exports preserve the Root union. All eight handoff evidence,
schema, manifest and Gateway patch hashes were independently checked. The
producer receipt records 19 normal and six actual native PostgreSQL tests,
two physical restarts and clean owned teardown, with public activation NOT_RUN.
Its implicit extra Cargo binary is disclosed, not counted as a frozen/run output.

The exact bounded `gateway-response-v2.patch` SHA256
`d231d50a208adb660af78cb250f743536f82920d1953ca38648bb43acd00ea8f`
applied cleanly to the retained `3710a029` response blob `c1d01237`. It adds only
the closed reviewed-v2 branch to `avatar()` and three dedicated regression
groups; legacy response branches remain unchanged. Gateway compilation and
execution of those tests are NOT_RUN at this checkpoint. No active release,
operator registry, frontend implementation or admission input was switched.

Root source checks: direct Rust 1.88 rustfmt check passes; independent candidate
validation passes all 15 positive/16 negative fixtures, 38 resource occurrences,
source/raster checks and nine tamper refusals. The candidate remains denied for
production. The planning checker and whitespace check pass; neither is a
substitute for coherent Rust or actual public runtime qualification.
