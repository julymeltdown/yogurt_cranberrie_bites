//! Private composition of current Auth eligibility and self Profile content.
//! It is neither a portable grant nor an event, session or admission decision.
use super::{
    auth_client::{
        concert_admission::{
            AdmissionObservation, AdmissionReadError, BindingReference, BindingState,
        },
        GrpcAuthClient,
    },
    jwt::concert_access::ConcertAccessContext,
    profile_client::{concert_identity::ConcertProfileObservation, GrpcProfileClient},
};
use chrono::{DateTime, Utc};
use std::fmt;
use uuid::Uuid;

pub use crate::domain::concert_identity::ConcertIdentityReadError;

/// Two configured, already signed clients. The future caller owns AppState
/// construction; this value introduces no endpoint, signer or channel policy.
#[derive(Clone)]
pub struct ConcertIdentityReader {
    auth: GrpcAuthClient,
    profile: GrpcProfileClient,
}
impl ConcertIdentityReader {
    pub fn new(auth: GrpcAuthClient, profile: GrpcProfileClient) -> Self {
        Self { auth, profile }
    }

    pub async fn read<'a>(
        &self,
        context: &'a ConcertAccessContext,
        binding: Option<BindingReference>,
    ) -> Result<ConcertIdentityObservation<'a>, ConcertIdentityReadError> {
        let access = || {
            context
                .check()
                .map_err(|_| ConcertIdentityReadError::AccessExpired)
        };
        access()?;
        let authority = context
            .rpc_deadline()
            .map_err(|_| ConcertIdentityReadError::AccessExpired)?;
        // One absolute stop spans Auth, its negative gate and the later Profile
        // read. Child clients can only shorten this same stop further.
        let mut stop = authority.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let actor = context
            .user_id()
            .map_err(|_| ConcertIdentityReadError::AccessExpired)?;
        let joined = async {
            // Auth is the negative-fact gate. Every failure and every refusal
            // ends here, before Profile can be dispatched.
            let admission = self.auth.read_concert_admission(context, binding).await?;
            access()?;
            admission.check()?;
            if !admission.account_active()? || !admission.session_live()? {
                return Err(ConcertIdentityReadError::NotEligible);
            }
            match (binding, admission.binding_state()?) {
                (None, BindingState::NotRequested) => {}
                (Some(_), BindingState::Bound) => {}
                _ => return Err(ConcertIdentityReadError::NotEligible),
            }

            // The Profile client signs a fresh read with the same borrowed,
            // sealed context. Auth content is rechecked after this later await.
            let profile = self.profile.read_concert_self_profile(context).await?;
            access()?;
            admission.check()?;
            profile.user_id()?;
            profile.display_name()?;
            profile.bio()?;
            profile.observed_at()?;
            if profile.user_id()? != actor {
                return Err(ConcertIdentityReadError::Profile(
                    super::profile_client::concert_identity::ConcertProfileReadError::InvalidResponse,
                ));
            }
            let observation = ConcertIdentityObservation {
                context,
                stop,
                admission,
                profile,
                user_id: actor,
            };
            observation.check()?;
            if tokio::time::Instant::now() >= stop {
                return Err(ConcertIdentityReadError::DeadlineExceeded);
            }
            Ok(observation)
        };
        let bounded = platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, joined).await
        });
        match bounded.await {
            Ok(result) => result,
            Err(_) => Err(ConcertIdentityReadError::DeadlineExceeded),
        }
    }
}

/// Private checked current identity. Consuming it cannot outlive its Auth/Profile
/// children or the one verified request context. No constructor, Clone,
/// serialization, rebind, TTL or admission conversion is available.
/// ```compile_fail
/// use api_gateway::adapters::out::concert_identity::ConcertIdentityObservation;
/// let _ = ConcertIdentityObservation {};
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::concert_identity::ConcertIdentityObservation;
/// fn export(value: &ConcertIdentityObservation<'_>) { let _ = serde_json::to_string(value); }
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::concert_identity::ConcertIdentityObservation;
/// let _ = serde_json::from_str::<ConcertIdentityObservation<'_>>("{}");
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::concert_identity::ConcertIdentityObservation;
/// fn copy_owned<'a>(value: &ConcertIdentityObservation<'a>) -> ConcertIdentityObservation<'a> { (*value).clone() }
/// ```
pub struct ConcertIdentityObservation<'a> {
    context: &'a ConcertAccessContext,
    stop: tokio::time::Instant,
    admission: AdmissionObservation<'a>,
    profile: ConcertProfileObservation<'a>,
    user_id: Uuid,
}
impl fmt::Debug for ConcertIdentityObservation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConcertIdentityObservation([REDACTED])")
    }
}
impl ConcertIdentityObservation<'_> {
    pub fn check(&self) -> Result<(), ConcertIdentityReadError> {
        self.context
            .check()
            .map_err(|_| ConcertIdentityReadError::AccessExpired)?;
        if tokio::time::Instant::now() >= self.stop {
            return Err(ConcertIdentityReadError::DeadlineExceeded);
        }
        self.admission.check()?;
        self.profile.user_id()?;
        self.profile.display_name()?;
        self.profile.bio()?;
        self.profile.observed_at()?;
        if self.profile.user_id()? != self.user_id {
            return Err(ConcertIdentityReadError::Profile(
                super::profile_client::concert_identity::ConcertProfileReadError::InvalidResponse,
            ));
        }
        Ok(())
    }
    pub fn user_id(&self) -> Result<Uuid, ConcertIdentityReadError> {
        self.check()?;
        Ok(self.user_id)
    }
    pub fn display_name(&self) -> Result<&str, ConcertIdentityReadError> {
        self.check()?;
        Ok(self.profile.display_name()?)
    }
    pub fn bio(&self) -> Result<Option<&str>, ConcertIdentityReadError> {
        self.check()?;
        Ok(self.profile.bio()?)
    }
    /// The canonical profile id for this account, or `None` when the account has
    /// no live profile identity.
    ///
    /// This is not the account id in [`Self::user_id`]. A caller that addresses
    /// profile-owned tables treats `None` as a refusal.
    pub fn profile_id(&self) -> Result<Option<Uuid>, ConcertIdentityReadError> {
        self.check()?;
        Ok(self.profile.profile_id()?)
    }
    pub fn binding_state(&self) -> Result<BindingState, ConcertIdentityReadError> {
        self.check()?;
        Ok(self.admission.binding_state()?)
    }
    pub fn admission_observed_at(&self) -> Result<DateTime<Utc>, ConcertIdentityReadError> {
        self.check()?;
        Ok(self.admission.observed_at()?)
    }
    /// Current Profile statement observation; never a revision or TTL.
    pub fn profile_observed_at(&self) -> Result<DateTime<Utc>, ConcertIdentityReadError> {
        self.check()?;
        Ok(self.profile.observed_at()?)
    }
}
