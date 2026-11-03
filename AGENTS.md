# MCR working agreement

## Authority and delivery

- The user's delivery repository is `julymeltdown/mcr`, branch `master`. Old references to delivery into Bigyeting in `docs/source/` are historical, not instructions for this repository.
- Read `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, `docs/mcr/project-assessment.md`, and `docs/mcr/execution-plan.md` before implementation. Product scope and acceptance come from the specification; analysis documents do not silently amend them.
- Preserve `docs/source/` byte for byte. Record provenance for copied code, source revisions, destination paths, dependencies, and regression evidence.
- Use English only in every chat/session, including other development chats: replies, progress updates, questions, reviews, handoffs and final answers, even when the user writes in Korean. Carry this explicit user instruction into delegated or continued chats. Use English for new chat titles, product navigation and engineering artifacts. Preserve exact source text, filenames, identifiers and quoted evidence where fidelity requires it.

## Parallel work and review

- The main chat tracks overall progress, allocation, and cross-team conflicts. Functional scrum leads track delivery and unblock their assigned teams. Neither the main chat nor a scrum lead is a mandatory code-review or approval gate (explicit user direction, 2026-11-04).
- The user explicitly requires separate visible Codex chats for backend and frontend development, not a subagent-only arrangement. Use the authorized chat-management tools, keep each writer on its assigned isolated worktree, and stop the former writer before transferring ownership. A designated delivery chat integrates validated handoffs without waiting for main-chat or scrum-lead approval.
- Preserve the user's selected model and reasoning setting in each chat. The user withdrew the earlier fixed `gpt-6.1-sol` / `high` requirement; omit model and reasoning overrides from coordination and continuation calls unless the user explicitly requests a new setting. Prefix chat titles with `mcr(be)`, `mcr(fe)`, `mcr(qa)`, `mcr(art)`, or `mcr(pm)` as appropriate; title failure must not block work.
- The user explicitly requires a persistent goal in each assigned development chat. Every assignment must include a concrete objective and completion criteria covering implementation, relevant validation, documentation, committed handoff, and review fixes. Ask the receiving chat to inspect its current goal and create one when no unfinished goal exists; reuse an aligned active goal and report incompatible goals without falsely completing them. Continue until the assigned scope is delivered, keep unfinished required work active, and verify actual goal status rather than treating a sent instruction as proof. Do not set a token budget unless the user requests one.
- Give each worker explicit task IDs and disjoint file ownership. Workers do not change central manifests, lockfiles, contract registries, root routers, or CI composition without coordination with the integrator.
- Start a task when its actual prerequisites are satisfied. Do not impose full-wave barriers or make FE feature implementation depend on matching BE feature completion; use frozen contracts and deterministic fixtures.
- Implementation owners validate their work and deliver committed handoffs. Use targeted peer/QA review for concrete risks and real vertical-flow gates; do not require an additional main-chat or scrum-lead review for every patch. Reassign or close idle, finished, duplicative, or conflicting agents. Reuse context when a follow-up is closely related.
- Only the integrator pushes reviewed work to master. Use explicit staged paths, preserve concurrent work, and never force-push.

## Frontend/backend separation

- Give every implementation worker its own Git worktree and task branch, such as `codex/mcr-be-BE03` or `codex/mcr-fe-FE09`. Concurrent workers must not edit the integration worktree or share a writable checkout, including workers within the same lane.
- Backend workers write only their assigned paths under `backend/`; frontend workers write only their assigned paths under `frontend/`. Lane-specific documentation may be explicitly assigned separately. Reading the other side for compatibility review does not grant permission to modify it.
- The delivery integrator coordinates single-writer ownership of shared HTTP/realtime/asset contracts, generated artifacts, central manifests and lockfiles, root composition, CI, and infrastructure. It may delegate an exact shared-file scope to an implementation owner and record the transfer; shared changes must not wait for main-chat or scrum-lead approval. No worker acquires these paths implicitly. This includes `backend/docs/api/` and `backend/proto/`.
- Frontend code must not import backend implementation files, server configuration, database access, or secrets. Backend code must not depend on frontend rendering, browser state, or frontend mocks. Connect through the agreed HTTP/WebSocket contracts and generated public types. Client validation does not replace server authorization or invariants.
- Split work spanning both sides into separate backend and frontend patches/commits. QA and art workers receive exact test or asset paths; their assignments do not grant general application-code ownership.
- Before integrating, compare every changed path against the assignment and inspect cross-directory imports. Return out-of-scope edits to the responsible lane or separate them into an integrator-owned patch. Record the worker's base commit and changed-path list; integrate reviewed worker commits serially, then run the relevant combined checks.

## Implementation boundaries

- All rendering/UI is newly authored. Only audited headless Bigyeting logic may be reused; keep old rendering and unneeded visual libraries out of the new application.
- Character artwork must come from the user's selected [source pack](frontend/src/entities/avatar/character-source-pack.json) and its reviewed derivatives. Previous/legacy character images are prohibited, including fallback images. See [character-source guidance](docs/mcr/character-art.md). The C06 synthetic fixtures are metadata tests, not approved product art. Preserve supplied originals byte for byte and do not silently replace them with newly generated designs.
- Never use decorative left accent lines, colored left borders, inset left rails or pseudo-element left bars in product UI, notices, error panels, toasts, skeletons or documentation-style panels. The user explicitly prohibits this pattern. Avoid generic AI-styled decoration and repetitive accented cards; use spacing, typography, clear labels and restrained semantic styling instead. Preserve necessary keyboard focus outlines and ordinary symmetric control borders.
- Use the fixed frontend stack and preserve retained lockfile resolutions. Import the selected port-rust backend foundation with its source history/provenance and add one concert service.
- One canonical Auth identity, one persistent Phaser instance, and one world connection per active shell. Access tokens stay in memory; private caches are scoped by user and event.
- Virtual coordinates never imply physical arrival. Real meeting descriptions require explicit audience, revision, expiry, and reviewed place provenance.
- Enforce capacity, inventory, privacy, authorization, and idempotency on the server. Use isolated MCR databases and volumes; live-data migration is outside this task.

## Evidence

- `generated/` is a planning baseline; regenerating it is not an implementation status update. Keep execution evidence in `docs/mcr/`.
- Use `NOT_STARTED`, `IN_PROGRESS`, `MOCK_READY`, `CONTRACT_READY`, `INTEGRATED`, `ACCEPTED`, `BLOCKED`, and `NOT_RUN` accurately. A mock or unit test cannot certify a real multiuser flow.
- Each accepted task records source/contract revisions, paths, behavior, commands and exit codes, evidence, limitations, and delivered commit ancestry.
- IN05 reviews all 112 required tasks (including its own decision) and all 61 required IA entries. Scope cuts require an explicit recorded user decision.
- Initial planning check: `python3 scripts/mcr/check-plan.py`. Application build, database, browser, device, load, and release checks are separate obligations.
