//! Auth-local snapshot only. No transport or production admission composition.
use async_trait::async_trait;
use auth_application::concert_identity::*;
use sqlx::{PgPool, Row};
use std::{fmt, future::Future, time::Duration};

#[derive(Clone)]
pub struct PgCurrentAdmission {
    pool: PgPool,
}
impl PgCurrentAdmission {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn snapshot(
        &self,
        request: AdmissionRead,
        budget: &ReadBudget,
    ) -> Result<CurrentAdmission, ReadError> {
        budget.check()?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| ReadError::Unavailable)?;
        budget.check()?;
        // Transaction-local only: never leave a modified pooled session behind.
        // The same absolute caller deadline bounds this await and every later one.
        let millis = budget.remaining()?.as_millis().max(1).to_string();
        sqlx::query("select set_config('statement_timeout', $1, true)")
            .bind(millis)
            .execute(&mut *tx)
            .await
            .map_err(|_| ReadError::Unavailable)?;
        budget.check()?;
        let binding = request.binding();
        // Omit the provider relation entirely for ordinary email-only reads,
        // including relation locks during parsing/planning.
        // All owner facts and DB time share this single statement's MVCC snapshot.
        let binding_clause = if binding.is_some() {
            // OAuth bounds match PgOnboardingTransactions' owner validation:
            // <=512 UTF8 bytes, nonblank Rust White_Space trim, no C0/C1 control.
            // Evaluate protected contents in the DB; never project them.
            r#"exists(select 1 from public.user_identities
                where id=$3 and user_id=$1 and provider_type=$4
                    and binding_epoch=$5 and deleted_at is null
                    and case when provider_type='EMAIL' then
                        email_verified=true and btrim(email)<>''
                    else
                        provider_subject is not null and octet_length(provider_subject)<=512
                        and btrim(provider_subject, U&'\0009\000A\000B\000C\000D\0020\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000')<>''
                        and provider_subject !~ U&'[\0001-\001F\007F-\009F]'
                    end)"#
        } else {
            "null::boolean"
        };
        let query = format!(
            r#"
            with owner_clock as materialized (select clock_timestamp() as observed_at)
            select owner_clock.observed_at,
                exists(select 1 from public.users where id=$1 and status='ACTIVE' and deleted_at is null) as active,
                exists(select 1 from public.refresh_tokens where user_id=$1 and family_id=$2
                    and revoked_at is null and expires_at > owner_clock.observed_at) as live,
                {binding_clause} as bound
            from owner_clock
        "#
        );
        let mut query = sqlx::query(&query)
            .bind(request.user_id())
            .bind(request.family_id());
        if let Some(binding) = binding {
            query = query
                .bind(binding.identity_id())
                .bind(binding.provider().owner_name())
                .bind(binding.epoch());
        }
        let row = query.fetch_one(&mut *tx).await.map_err(|error| {
            if error.as_database_error().and_then(|e| e.code()).as_deref() == Some("57014") {
                ReadError::DeadlineExceeded
            } else {
                ReadError::Unavailable
            }
        })?;
        budget.check()?;
        let observed_at = row
            .try_get("observed_at")
            .map_err(|_| ReadError::Unavailable)?;
        let active: bool = row.try_get("active").map_err(|_| ReadError::Unavailable)?;
        let live: bool = row.try_get("live").map_err(|_| ReadError::Unavailable)?;
        let bound: Option<bool> = row.try_get("bound").map_err(|_| ReadError::Unavailable)?;
        let binding = match (binding, bound) {
            (None, None) => BindingEligibility::NotRequested,
            (Some(_), Some(true)) => BindingEligibility::Bound,
            (Some(_), Some(false)) => BindingEligibility::NotBound,
            _ => return Err(ReadError::Unavailable),
        };
        tx.commit().await.map_err(|_| ReadError::Unavailable)?;
        budget.check()?;
        Ok(CurrentAdmission {
            observed_at,
            account: if active {
                AccountEligibility::Active
            } else {
                AccountEligibility::Ineligible
            },
            session: if live {
                SessionEligibility::Live
            } else {
                SessionEligibility::Ended
            },
            binding,
        })
    }
}
impl fmt::Debug for PgCurrentAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PgCurrentAdmission([REDACTED])")
    }
}

async fn bounded<T>(
    budget: &ReadBudget,
    future: impl Future<Output = Result<T, ReadError>>,
) -> Result<T, ReadError> {
    budget.check()?;
    let cancellation = async {
        loop {
            if let Err(error) = budget.check() {
                return Err(error);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::select! {
        biased;
        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(budget.deadline())) => Err(ReadError::DeadlineExceeded),
        result = cancellation => result,
        result = future => { budget.check()?; result }
    }
}
#[async_trait]
impl CurrentAdmissionPort for PgCurrentAdmission {
    async fn read_current(
        &self,
        request: AdmissionRead,
        budget: &ReadBudget,
    ) -> Result<CurrentAdmission, ReadError> {
        bounded(budget, self.snapshot(request, budget)).await
    }
}
