# Character runtime ownership

Status: CONTRACT_READY for the root lifetime registry; application wiring and genuine-character route acceptance remain IN_PROGRESS.

`frontend/src/app/character-entry/registry.ts` retains one borrowed character runtime for the current canonical Auth privacy scope and manifest pin. Header, Me, Customize and World will obtain the same runtime through this application owner. Closing a view releases that view's render leases. Only registry teardown, a replacement release, canonical privacy cleanup or runtime invalidation releases the shared runtime. The registry creates no Auth, requests, Phaser instance or world connection.

The injected runtime factory owns full reviewed manifest, source, hash and image validation. The registry requires a current complete privacy scope and matching bootstrap participant/event before construction or reuse. It rejects synchronous identity changes during construction and destroys the abandoned runtime. Factory failures remain unavailable and do not install a partial owner. The registry is not an independent authorization source or asset fallback.

Five focused tests pass (exit0), using the actual canonical Query privacy coordinator and session: shared view borrowing; synchronous policy withdrawal and stale-scope refusal; cross-user bootstrap refusal before allocation; identity replacement during construction; and World borrowing only an already-admitted exact manifest pin. Whole-frontend strict TypeScript passes (exit0). These tests establish lifetime boundaries only. Selected source pixels, HTTP/cache/decode counts, renderer integration, saved identity, backend services, devices and multiuser acceptance are separate obligations.

The whole-character producer is owned by its isolated frontend runtime chat under `entities/avatar/whole-character/`; the root registry does not copy or publish that worker's source. The canonical representation is the published C06 1.1.0 contract at `3398d8331863775ecc0180e5eccebddd179fedd5`. Retained layered 1.0.0 behavior remains separate.
