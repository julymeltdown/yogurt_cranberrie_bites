# Browser contract and event configuration

The browser consumes `contracts/generated/mcr-core.openapi.json` as a read-only
public artifact. It is byte-identical to `backend/docs/api/mcr-core.openapi.json`.
Both outputs come from the same `build_document()` in
`backend/scripts/docs/generate-mcr-core.py`, which uses the retained final
inherited assembler plus the frozen C03 and C06 inputs. Neither output may be
edited manually. This exposes a contract, not backend application implementation.

Run `python3 backend/scripts/docs/generate-mcr-core.py` to regenerate both.
The existing `--check` command and `scripts/mcr/check.sh contracts` reject drift
in either artifact. There is no independent schema/SDK authority. Runtime and
OAuth implementation status metadata retain their existing meaning; publication
of this artifact does not qualify unimplemented endpoints.

The wider `contracts/generated/mcr-domain.openapi.json` is a **review candidate**,
assembled by `backend/scripts/docs/generate-mcr-domain.py` through that same core
and retained final pipeline. It adds the 106 C04 domain and two C05 world HTTP
operations to the 24 core operations. It preserves the core paths/components and
does not switch FE09 to an unfrozen contract. Domain/realtime definitions are
namespaced; all schema references resolve locally. Closed query schemas, path
types, command receipts, status/reason restrictions and policy metadata remain
attached. WebSocket frames remain the C05 JSON Schema authority; their presence
as bundle components does not invent an HTTP WebSocket endpoint.

Regenerate this candidate with `python3 backend/scripts/docs/generate-mcr-domain.py`.
The contract command checks both outputs through their owning assemblers. Neither
is manually maintained. C04 freeze, per-operation producer/consumer fixtures,
independent reviews and generated client adoption remain separate work; this
candidate is not a second independently authored wire contract or an approved SDK.

FE09 may consume the public artifact from its feature-local decoder. Fail closed
for unsupported validation keywords and invalid/context-mismatched responses;
do not claim complete JSON Schema support from a narrow decoder. Semantic C03
checks (canonical identity, asset versions/hashes/geometry, policy and capability
relationships, UTF-8 password bounds) still apply beyond structural validation.
Keep token-bearing responses in memory and out of query persistence/logs.

The approved production event input is `VITE_MCR_EVENT_ID`, read only in
`frontend/src/main.tsx` and passed explicitly into the session composition. It is
public build-time configuration, never a credential. Validate a canonical UUID;
missing or invalid input leaves event entry unavailable. Do not infer an event
from a fixture, local storage, arbitrary query parameter or invented discovery
endpoint. The production event value is not supplied yet. No checked-in default
event or user identity is authorized. Build/test fixtures must stay isolated from
the application entry. Event changes still replace canonical session scope and
clear private state; a configuration value is not authorization to join an event.
