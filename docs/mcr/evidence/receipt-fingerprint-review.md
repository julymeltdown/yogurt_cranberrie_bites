# C04 command fingerprint privacy correction

Date: 2026-11-03. C04 remains REVIEW_CANDIDATE. During BE15 review the parent
found that the initial canonical unkeyed request digest would persist an offline
guess verifier for low-entropy input such as a six-digit pickup code. Omitting
raw request bodies alone does not address that risk. The candidate also exposed
the digest in public `CommandReceipt`, although a browser needs no such value
to recover an authorized command outcome.

The candidate now requires server-only **HMAC-SHA256**, with a versioned format
and key identifier. HMAC uses a secret key for computation and verification;
the construction and key guidance are documented in [RFC 2104](https://www.rfc-editor.org/rfc/rfc2104).
The unkeyed-guessing finding is the integrator's threat analysis, not a claim that
HMAC encrypts or anonymizes the request. Private storage access, key protection,
online throttling and current authorization remain necessary.

`policy.json.idempotency` binds canonical actor/event/operation/key scope,
method, canonical path, validated/default-resolved query and typed JSON/absent
or binary input using unambiguous versioned framing. Keys are trusted injected
secrets of at least 32 random bytes, never browser/request/DB values or source
defaults. The durable internal format is
`hmac-sha256:v1:<key_id>:<64 lowercase hex>`, with a bounded strict key identifier.
All commands use keyed verification; there is no unkeyed or legacy fallback.

New commands use the active signing key. Authorized replay uses the receipt's
stored key identifier and retained verification material, with constant-time
library MAC verification. Active-key rotation alone does not create a conflict.
Missing verification material fails closed; expiry/retirement never permits
reexecution. Key rotation must retain old verification material for surviving
records or explicitly retire their verification capability. It must not silently
rehash an uncertain command under a new key.

The closed public `CommandReceipt` no longer has `request_hash`. It rejects that
legacy field, fingerprint, key material and canonical input. The two affected
fixtures were revised, and a negative fingerprint-leak fixture was added. New
tests reject those fields both at the source schema and on every assembled
pending-command response. Public receipt scope/status checks remain unchanged.

`bash scripts/mcr/check.sh contracts` exits 0: **137 tests** (21 asset, 23 core,
31 realtime, 47 domain, 15 combined assembly); 36 domain payload fixtures. The
combined 132-operation review artifact reproduces. Frozen C03, C05, assets and
inherited API artifacts are unchanged. This is contract/privacy evidence, not
runtime key provisioning, qualified command producers, PostgreSQL concurrency
or full C04 acceptance.

The BE15 chat received the explicit decision and an integrator-owned retained
`hmac` dependency edge. Its old unkeyed candidate is not integrated or published.
The worker must provide a new committed implementation and matching real
database receipt covering known answers, rotation, lost keys, secret-bearing
input conflicts, safe error output and rollback before runtime integration.
