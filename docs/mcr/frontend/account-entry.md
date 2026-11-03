# Account and avatar root composition

Status: IN_PROGRESS; bounded source consumer composition from `5c95ed4ddb243853b27fb625f726341b9771dadd`. Exact ownership: new `frontend/src/app/account-entry/**` and this document; separately delegated `router.tsx` existing four account route elements, `WorldShell.tsx` one stable account provider wrapper, and `ports.ts` only an optional typed `accountAssets` field/import. No main, constructors/defaults, world/entity, contract, source archive or dependency edits.

The actual frozen route table at this base uses `/me`, `/me/profile`, `/me/avatar`, `/me/settings`. No `/me/edit` or `/me/preferences` routes are introduced. Existing feature links remain valid.

`createAccountOwner` receives canonical Auth plus the already validated shell bootstrap and original privacy lease. It creates one ME controller and one lazy-started FE13 editor controller; it does not create an identity, query client, Phaser instance or world connection. The React provider owns setup/teardown in an effect and keeps the child tree stable. Exact auth/bootstrap/lease/assets bindings fence owner publication; disposal and canonical privacy cleanup erase both controllers. Merely opening the world sends no participant/editor request.

`AccountAssetsPort` contains only explicitly supplied reviewed binding/resources. Manifest and pin come from the exact retained C03 bootstrap document under the original lease. No port, absent manifest or rejected review produces an unavailable preview. The shared reviewed compositor and FE13 reviewed preview adapter enforce pack/binding/hash/skin constraints. There is no fallback art, generated substitute, global configuration or second bootstrap fetch. No product asset port is installed by this patch.

FE13 verified `onCommitted(avatar, scope)` calls `me.refreshAfterAvatar(scope, avatar.revision)`. FE34 rejects stale scope/revision notifications, retains dirty/pending drafts and requires explicit reload instead of silently rebasing them. World appearance remains authoritative server replication; this adapter does not write replicated entities or simulate a world update. Editor view mount/unmount uses its public start/stop lifecycle, including retained safe drafts and fresh explicit reconciliation on return. Ordinary panel navigation keeps the provider/ME controller alive.

Settings persistence uses exact C03 GET/PATCH through existing FE34. Existing runtime settings/FE06 theme adapters are not supplied implicitly; the feature truthfully states when live controls are unavailable. Optional ticket section/visibility remain outside qualified C03 and unaccepted. Root wiring is consumer composition, not production art/live HTTP/multiuser acceptance.

Validation and committed ancestry will be recorded after the focused/native root checks.
