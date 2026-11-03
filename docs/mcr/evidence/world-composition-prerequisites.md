# World lifecycle composition prerequisites

Date: 2026-11-03. These are central integration changes for the isolated world
composition implementation, not a claim that production multiplayer is running.

`9919b79` passes the exact bootstrap privacy lease from WorldHost into the
WorldLifecyclePort mount context. It does not capture a newer lease or recreate
a world on ordinary panel navigation. Seven retained community composition tests
and pinned TypeScript build pass.

The C03 decoder now clones and freezes its input before asynchronous asset-hash
validation, then retains the validated document on ShellBootstrap. The resource
resolver can consume those same manifest/default bytes with the captured lease;
it need not issue another bootstrap request or accept independently restamped
metadata. Fixtures without a document remain usable for shell-only tests, but
cannot supply a production asset release. No artwork is generated or promoted.

Pinned Node24.22.2/pnpm11.7.0 validation: 73 tests across auth contract, session
and community composition pass, including a new input-mutation-during-validation
and nested immutability regression. `tsc -b --pretty false` passes. These changes
add no UI, asset fallback, world renderer, socket server or production release.
