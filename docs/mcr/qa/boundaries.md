# FE/BE worker boundary guard

Task: **QA-BOUNDARIES**, an integrator-delegated support increment. This is not
acceptance of C08, QA02, QA10, any feature, or a real multiuser flow. Integration
and parent task acceptance remain **NOT_RUN** in this worker checkout.

Assignment baseline: `69c17cc0e85eb7209e854a56da0f1388072e00b6` on
`codex/mcr-qa-boundaries`. Rules come from the root and lane `AGENTS.md`,
`MCR_IMPLEMENTATION_PLAN_V2_EN.md` §§10/C08, 13/QA02/QA10 and 15.2,
`docs/mcr/project-assessment.md`, and `docs/mcr/execution-plan.md` at that baseline.
The implementation is newly authored Python standard-library code. No upstream
application modules were copied, source checkouts changed, dependencies added,
contracts changed, or migrations performed.

## Run

Use Python 3.10+ and Git. Run from the worker repository or any subdirectory; the
script checks the Git repository of the **current directory**, not its own path.
The integrator supplies the actual assignment base commit and authorized lane:

```sh
python3 /absolute/path/to/check-boundaries.py \
  --base <worker-base-sha> --lane frontend \
  --allow-doc docs/mcr/frontend-task.md

python3 /absolute/path/to/check-boundaries.py \
  --base <worker-base-sha> --lane backend
```

`--allow-doc` is repeatable and grants only an exact repository-relative file with
suffix `.md`, `.rst`, or `.txt` under `docs/mcr/` or the selected lane. It grants
neither a directory nor a glob. Traversal, absolute paths, backslashes, opposite
lane files, original source documents, and reserved files cannot be exceptions.
Lane documentation also requires this explicit assignment. CLI inputs express
the integrator's assignment; the tool cannot authenticate who supplied them.

Exit codes: **0** passes the automated guard; **1** reports boundary violations;
**2** reports invalid CLI/configuration, base revision, repository, or Git failure.
An empty diff passes and prints zero changes; it is not implementation evidence.

## What is checked

- Both `git diff --cached <base>` and `git diff <base>`, plus non-ignored untracked
  files. This covers committed work since the base, staged work, and unstaged
  work. Index blobs are scanned separately, so an unstaged reversal cannot hide
  a staged violation. This is a direct comparison with the supplied commit,
  not a merge-base or patch limited to the latest commit.
- Writes must stay in the selected lane or exact assigned documents. Adds,
  modifications, deletions, type changes, and both old/new rename paths are
  checked. Unresolved merges fail. Git filename lists use NUL separators;
  subprocesses use argument arrays, and index lookup uses literal pathspecs.
  Spaces, tabs, newlines, option-like names, and POSIX non-UTF-8 filenames do
  not become commands or extra paths. Diagnostics quote filenames.
- Integrator-owned areas are denied even inside the selected lane: contract,
  proto, generated, CI, script, infrastructure/deployment directories;
  `backend/docs/api/`; manifests and locks including nested `Cargo.toml` and
  `package.json`; known environment/build configuration; `AGENTS.md` and Git
  policy files. Exact directory/name sets live in `CENTRAL_DIRS` and
  `CENTRAL_NAMES`; configuration patterns live in `central_reason`.
- Known root composition paths are denied by `COMPOSITION`: frontend entrypoint
  files, root/app router and routes files/directories, service entrypoint/root
  router files/directories, and HTTP adapter `mod.rs`/router/routes composition.
  Feature modules such as `frontend/src/features/...` and
  `backend/crates/.../src/...` are allowed unless another reserved rule applies.
- Changed/new JS/JSX/TS/TSX and module variants, and Rust source, are scanned for
  obvious cross-lane imports or embeds. Checks cover static import/export-from,
  side-effect import, literal `import(...)`, `require(...)`, `require.resolve`,
  Rust literal `include!`/`include_str!`/`include_bytes!`/`include_dir!`,
  `#[path = ...]`/`#[folder = ...]`, direct `use frontend::...` and
  `extern crate frontend` (and the reverse lane name). Relative paths are
  normalized from the importing file. Alias/absolute specifiers containing an
  explicit opposite-lane segment, such as `@backend/...`, are also flagged.
  Imports of `contracts/generated/...` public types are permitted; writing those
  generated files is not.
- The lexer ignores JS/Rust comments and ordinary unrelated string literals.
  Documentation/prose is not scanned as source. Static backtick and Rust raw
  string paths are supported. Non-deleted symlinks and non-regular changed files, including
  submodules, fail without following their contents. Unreadable or non-UTF-8
  recognized source files fail inspection.

## Required manual review and limits

This is a guard plus manual review, not a sandbox or security proof. It does not
prevent edits, authorize an assignment, lock a checkout, or enforce worktree
isolation. The integrator still compares every changed path with the exact task
assignment and checks worker base/commit ancestry before serial integration.
The lane is a broad ceiling; it does not grant a worker every file in that lane.

Reserved paths are conservative naming rules, not a discovered ownership graph.
New router/composition locations and alternative manifest/configuration names
need review and policy updates. Any explicitly delegated central-file patch is
reviewed by the integrator separately; this CLI deliberately has no general
override. It has only BE/FE modes; QA/art/integrator exceptions are not implied.

The lexer is not a JS/Rust parser, dependency resolver, or secret scanner. It
does not resolve project aliases without explicit lane names, package aliases,
computed/escaped/concatenated paths, nested macro expressions, arbitrary language
loaders, or transitive imports. Template interpolation, Rust nested imports or
comments, and JavaScript regex/JSX syntax can cause misses or false positives.
Other languages, binaries, and unchanged source are not import-scanned. Review
any reported case and inspect cross-directory imports manually. An old violation
elsewhere in the repository is not certified away by a passing worker diff.

Ignored untracked files and Git-hidden local modifications (for example,
`assume-unchanged`/`skip-worktree`) are not a complete filesystem inventory.
Net-reverted changes are absent from a final snapshot. The command does not
verify intermediate commits, ancestry correctness of the supplied base, or
remote delivery. Files/index must stay stable during the run; there is no
protection against concurrent or hostile filesystem/Git changes. For a handoff,
use the pinned assignment base in a quiescent worker checkout, review staged and
unstaged diffs, and repeat on the reviewed candidate as appropriate.

## Regression evidence

The fixture suite uses only disposable local Git repositories. It covers normal
BE/FE code; opposite-lane committed/staged/unstaged/untracked changes; protected
locks/config/contracts/router paths; deletions and renames in both directions;
renamed import content; exact documentation grants; import/embed syntaxes;
comments and legitimate public-contract references; unsafe and unusual filenames;
symlinks; invalid source encoding; unresolved merges; staged content hidden by an
unstaged reversal; bad base/option input; and invocation from subdirectories or
outside a repository. Fixtures do not inspect live databases, hardware, upstream
user data, or remote services.

```sh
python3 -B -m unittest discover -s scripts/mcr/tests -p test_boundaries.py -v
PYTHONDONTWRITEBYTECODE=1 python3 -B scripts/mcr/check-plan.py
git diff --check
```

Recorded worker run (2026-11-03, Python **3.14.3**, Git **2.55.0**):

| Command | Exit | Result |
|---|---|---|
| `python3 -B -m unittest discover -s scripts/mcr/tests -p test_boundaries.py -v` | 0 | 37 tests: 36 passed, 1 skipped; 7.734 seconds |
| `PYTHONDONTWRITEBYTECODE=1 python3 -B scripts/mcr/check-plan.py` | 0 | PASS; original source hashes and planning exports preserved |
| `python3 -B scripts/mcr/check-boundaries.py --help` | 0 | CLI options render |

The skipped fixture attempts a non-UTF-8 byte filename; this macOS filesystem
rejects it with `EILSEQ` before Git runs. That case remains **NOT_RUN** here.
All other boundary tests ran. Whitespace verification and the delivered local
commit are included in the handoff.

The parent integrator owns wiring into root checks/CI and master delivery;
those files are outside this increment's three-file scope. Application builds,
database/browser/device/load/release checks remain **NOT_RUN**.
