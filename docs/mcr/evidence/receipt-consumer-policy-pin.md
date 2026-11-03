# Receipt harness pin after consumer recovery review

The parent reviewed `ca9f955`: all 106 operation IDs/methods/paths, common receipt
schema, fingerprint policy, TTL and transaction requirements are unchanged.
The policy adds explicit consumer recovery/context rules: ambiguous status 404,
retirement or expiry never authorizes replay/new keys; successful metadata needs
an authorized resource reread. Party Member additionally carries event identity.

The BE15 smoke controller's policy pin now matches that reviewed candidate;
unknown drift is still refused. Its seven focused helper tests pass (exit 0).
No Rust/SQL/producer/outbox change or new native database run is claimed.
Previous BE15 native evidence remains bound to its original candidate. Production
command producers/HTTP status authority still require their own qualification.
