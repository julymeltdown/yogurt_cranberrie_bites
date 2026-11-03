# Community policy & meeting lock-order native qualification report

Status: **BLOCKED_ON_AUTHOR_CORRECTION**. Candidate commit `51807a08f399a2a681b584f2d1aed292f0f0b356` was compiled against private target `/Users/lhs/Desktop/mcr-build/community-native-51807/target` using the allocated resource grant (`cargo test --locked -j 1 -p concert-service --test qa_community_policy_lock_order --no-run`).

## 1. Scope & Frozen Boundaries
- **Worktree**: `/Users/lhs/Desktop/mcr-worktrees/qa-community-native`
- **Branch**: `codex/mcr-qa-community-native-51807`
- **Candidate Commit**: `51807a08f399a2a681b584f2d1aed292f0f0b356`
- **Sole Tracked Writable Path**: `docs/mcr/qa/community-native-qualification.md`
- **Source Status**: Frozen READ-ONLY. No application or test source files were modified.

---

## 2. Exact Compiler Diagnostic

Compilation of `backend/services/concert-service/tests/qa_community_policy_lock_order.rs` failed with exit code 101:

```
error: implementation of `Executor` is not general enough
   --> services/concert-service/tests/qa_community_policy_lock_order.rs:104:31
    |
104 |           .after_connect(|c, _| Box::pin(async move {
    |  _______________________________^
105 | |             sqlx::raw_sql("set statement_timeout='5s'; set lock_timeout='4s'; set idle_in_transaction_session_timeout='10s'")
106 | |                 .execute(c).await?;
107 | |             Ok(())
108 | |         }))
    | |__________^ implementation of `Executor` is not general enough
    |
    = note: `Executor<'_>` would have to be implemented for the type `&'0 mut PgConnection`, for any lifetime `'0`...
    = note: ...but `Executor<'1>` is actually implemented for the type `&'1 mut PgConnection`, for some specific lifetime `'1`
```

### Root Cause in Test Candidate:
In `qa_community_policy_lock_order.rs:104–108`, the `after_connect` closure passes `c` (which is `&mut PgConnection`) into `sqlx::raw_sql(...).execute(c)`. Under SQLx 0.8 with Rust 1.88, the higher-ranked trait bound (HRTB) for `for<'c> Executor<'c>` fails type inference because `c` is captured into the async block. In SQLx, executing queries inside `after_connect` requires `execute(&mut *c)` or `sqlx::query(...).execute(&mut *c)` to satisfy the higher-ranked lifetime bound.

---

## 3. Required Action from Candidate Author
Candidate author (`01a1a232-b91d-78f1-a13a-1361788ba51c`) must update line 104 in `backend/services/concert-service/tests/qa_community_policy_lock_order.rs` from:
```rust
        .after_connect(|c, _| Box::pin(async move {
            sqlx::raw_sql("set statement_timeout='5s'; set lock_timeout='4s'; set idle_in_transaction_session_timeout='10s'")
                .execute(c).await?;
            Ok(())
        }))
```
to:
```rust
        .after_connect(|c, _| Box::pin(async move {
            sqlx::raw_sql("set statement_timeout='5s'; set lock_timeout='4s'; set idle_in_transaction_session_timeout='10s'")
                .execute(&mut *c).await?;
            Ok(())
        }))
```
or remove the `after_connect` hook and set session timeouts directly on the acquired connections.

As the current QA assignment is strictly read-only on candidate source, this fix cannot be applied in this worktree and must be delivered by the author.

