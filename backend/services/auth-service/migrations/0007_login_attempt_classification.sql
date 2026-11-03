-- Distinguish a genuine credential failure from a refusal caused by an existing
-- lockout. Counting lockout refusals as failures let an attacker hold a victim's
-- account locked indefinitely by retrying before the window elapsed.
alter table login_audit
    add column if not exists credential_attempt boolean not null default true;

-- The lockout window query filters on user, outcome and attempt class.
create index if not exists login_audit_lockout_idx
    on login_audit (user_id, occurred_at desc)
    where success = false and credential_attempt;
