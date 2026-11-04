# Frozen v10 qualification

Candidate: `d5c0c9d188f68acb749d7ed2940706f34b2a8be1`.

- `capture-packet.json`: immutable SHA256/byte inventory for the 49 final native artifacts.
- `renderer-qualified/native.json`: three width cases, all three eras twice, strict zero-channel World/MAP/occlusion/landmark parity and resource/privacy cleanup.
- `root-verified/native.json`: 16 actual Root/default Phaser cases with synthetic source/HTTP/C05 inputs; includes the short viewport and marker status repairs.
- `qualification.json` and `checks/`: final 437-test regression, full TypeScript, lint, plan/UI-origin checks and actual application build receipt. Older checkpoint logs are explicitly historical.
- `historical-diagnostics.json`: hashes of preserved local superseded evidence. These runs are not final proof.

From the repository root, with Node 24.22.2 and the retained frontend dependencies available, set each output variable to a **new** absolute directory and run sequentially:

```sh
MCR_SCENE_V5_RENDERER_OUTPUT=<new-absolute-directory> node frontend/src/world/reference-scene/__tests__/spatial-v5/native.mjs
MCR_SCENE_V5_ROOT_OUTPUT=<another-new-absolute-directory> node frontend/src/app/world-entry/__tests__/reference-scene-v5/native.mjs
MCR_FRONTEND_DEPENDENCIES=<retained-frontend-directory> node frontend/src/world/avatar/__tests__/motion-adoption/build.mjs --application
```

Browser workflows require the existing Chrome installation and the coordinated resource allocation; drivers stop below the 8 GiB floor. No install or dependency change is required.

Backend source allocation is revision6; fixture revision7005 is synthetic. Original UNREVIEWED provenance, production-default denials and NOT_RELEASE_BOUND/NOT_ADMITTED remain truthful. This packet does not prove live Auth/server, real multiuser, physical iOS/Android, software keyboard/notch behavior, release readiness or full artist/product fidelity acceptance.
