# Seven-service functional startup and bounded pool configuration

Status: **INTEGRATED startup evidence only**. Original IN03 remains **NOT_RUN**.
The [retained summary](../evidence/in03-seven-service-startup/r7-summary.json)
records the actual run, executable/source pins, prior failures and cleanup.

The successful R7 round ran the actual Auth, Friend, Profile, Post,
Notification, Gateway and Concert mains from source
`18076c951a7283354c869caaf64d40491ca52d75`. These are the retained executables
qualified by the Root functional cohort; their filenames ending in `-tests`
do not make them test doubles. No service was recompiled for this copied-native
round. The launcher used strict URI/date-time format validation and retained no
full C03 body or full-body digest.

Three separate real users completed Gateway email signup, private local mail
delivery, verification, login, Profile initialization and canonical C03 bootstrap.
All corresponding HTTP responses were 200. All seven readiness checks passed.
READY began at 2026-11-04 17:28:19 UTC. The owner requested protocol shutdown
before the unused 600-second hold expired because the social consumer lacked
its original scenario prerequisites. Full release completed at 17:35:40.742499
UTC, with supervisor exit 0, every service exiting normally, no prepared
transactions, no owned PostgreSQL/TMP/PKI, and no owned artifact handles.

The round's overall result is **PARTIAL**, as the original social journey did
not run. Maximum sampled runner processes were 39 against a cap of 44; maximum
sampled whole bytes were 7,753,814,016 against 10 GiB. The 48-process allocation
included four separately reserved supervisory processes. The run used no target
access or newly created executable.

## Fix and reproducibility

The launcher previously left most inherited database pools at their defaults.
Earlier real rounds reached 45 runner processes and correctly failed the
44-process guard. Lowering PostgreSQL's global connection ceiling to 28 kept
the process cap but subsequently lost World replay readiness. That observation
does not by itself prove the precise replay failure cause. Another attempted
configuration set Gateway's journal pool to one; the real parser correctly
rejected it because its minimum is two. A separate preflight refusal identified
copied private provenance files with mode 0644 instead of 0600. These failures
are retained, not replaced by the later result.

R7 applied the supported service-specific configuration below through an
operational wrapper. This patch places the same settings in
`backend/scripts/mcr/in03_stack_runtime.py::managed_features`, so future runs
need no wrapper to set these pool limits. The successful native result remains
attributed to source 18076 and that wrapper; it is not relabeled as a native
run of the subsequent helper commit.

| Service pool | Connections |
| --- | ---: |
| Auth | 3 |
| Friend | 3 |
| Profile | 2 |
| Notification | 2 |
| Post | 3 |
| Gateway application | 2 |
| Gateway journal | 2 |
| Gateway background | 1 |
| Concert, unchanged | 8 |

The owned ephemeral PostgreSQL server used 64 connections and 128 prepared
transactions, with autovacuum and logical replication workers disabled for this
functional round only. These settings are not production, maintenance,
replication, load or performance qualification. No process cap, business
assertion, TLS check, policy check or canonical schema assertion was relaxed.

Validation of the persisted helper patch:

```sh
cd backend/scripts/mcr
python -B -W error -m unittest test_in03_stack_runtime
# exit 0; 18 tests passed using the strict contracts Python environment
```

The native command was the existing `in03_stack_runtime` CLI through its
owned operational supervisor, with explicit pinned inputs/report, the original
1200/960/240-second total/work/cleanup clocks, 240-second startup,
120-second provisioning, 600-second maximum consumer hold, 10-GiB whole limit,
nonshrinking retained carry and a 44-process runner cap. The allocation changed
only the initial free-space floor from 16 to 12 GiB; the continuous floor stayed
8 GiB. All actual clocks and measured bounds are in the summary. Private
descriptor, credentials and control paths are intentionally excluded here.

## Remaining product boundaries

The actual retained C03 projection advertised World as available and Feed,
Social sharing and mutations as `FEATURE_DISABLED`, although their managed
handlers were configured. The server release/capability composition needs its
own implementation and qualification; a frontend HTTP-readability result cannot
replace it or create a Social policy baseline.

The descriptor lacked qualified Intro/Post/server-expired-request resources,
actual denial dispositions and an admitted same-origin frontend. No browser
slot was accepted for the original journey. No verified-age or operator provider
was manufactured. Approved legal publication, reviewed physical meeting places
and provider-specific facts remain separate from internal implementation.

This run does not qualify newer OwnReports, Party, Gift or Wall activation,
two-client IN04, complete multiuser privacy, a physical device, or load.
QA08/performance remains **DEFERRED_BY_USER**. The copied-native run and helper
change are newly authored MCR integration work; no source-pack originals,
`docs/source/`, dependencies or lockfile resolutions changed.
