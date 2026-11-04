# Policy disclosure and Social read-child source checkpoint

Status: `IN_PROGRESS`. This additive SDK source is not a native runtime or
production recovery qualification. No public route or feature is enabled here.

The exact `scope.rs` disclosure hook from Age owner commit
`c15b8a422cdf4b59c59ef1089368cc738a4b1ce7` is integrated. Its public API is
`OwnerGuardScope::limit_disclosure_until(DateTime<Utc>) -> Result<(), Status>`.
It only shortens the original held scope and preserves both wall expiry and the
monotonic stop captured at each limit. Repeated calls cannot renew an earlier
limit; sealed and expired scopes fail closed. `disclosure_ceiling` includes this
policy cap. Age producer integration and its actual native timing case remain
owned by the Age implementation chat and require fresh qualification.

`ChildRecoveryReference::SocialRead` contains only `query: ChildQuery`,
`child_nonce: Uuid`, `native: ChildNativeIdentity`, and
`valid_until_unix_micros: i64`. It has closed kind `SOCIAL_READ`, distinct key
namespace 4, and derived GID `mcr-social-read:{nonce.simple()}`. No supplied GID,
SQL, endpoint or private projection belongs in the reference. The child must
cover exactly the original deadline; nonpositive, shortened or extended values
are rejected. Native producer identity is checked at SDK and ledger admission.
An actor Social child is checked as a global fence when projecting peers.

Migration 0042 adds only this kind; published 0028 and 0037 remain unchanged.
Existing encrypted references, full scope binding, total 202-intent bound and
terminal completion guards remain in force. The actual Social adapter shares
the existing local Public advisory slot namespace and 101-slot budget; the SDK
kind does not create a separate pool or authorize PREPARE by itself.

`CompositeChildIntentSettler::with_social` accepts the real
`SocialReadIntentSettler`; missing configuration fails closed. Public Profile
recovery explicitly refuses this kind. The Social implementation must acquire
actual policy locks, capture native identity, persist this intent, then PREPARE
and attach the held child. Recovery requires actual terminal native evidence;
absence of a GID, memory, or an expired deadline is insufficient.

Validation at this checkpoint: Rust 1.88 rustfmt and diff whitespace checks.
Closed-reference regression source covers exact deadline, identity namespace,
round trip and arbitrary GID rejection. The SQL-only controller actually applied 0028/0037/0042 against private
PostgreSQL 16, admitted the closed Social kind, rejected unknown kinds, preserved
immutable/terminal guards and unresolved evidence across restart: 31 recorded
commands and owned cleanup passed. See `social-read-child-sql.json`. The first
attempt failed because the restart assertion still expected two children after
adding a third; the corrected three-child assertion passed. This is synthetic
ledger evidence, not actual Social native recovery. Rust compilation, the new
Rust test and real Social held-child/restart cases remain `NOT_RUN` until a
fresh named build allocation and actual adapter composition.
The earlier default rustfmt executable failed to load its rustc driver; the
explicit Rust 1.88 formatter succeeded without changing the toolchain.

A separate production restart drain remains required: current durable settlement
has live-scope call sites but no configured startup enumerator. `ROOT_FINISHED`
with outstanding children cannot imply completed delivery or free capacity after
restart. The canonical qualification owner records six concrete cases in
`tests/owner_authority_support/restart_contract.json` on commit `f4a15ba9`;
these remain requirements, not executed acceptance evidence.
