//! Request-local qualified read bindings. Successful construction does not
//! authorize storage: both owner views and actual Concert subject callback must
//! still be checked and the real transaction registered before protected reads.
use crate::adapters::r#in::grpc::concert_feed::proto;
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, read_authority::VerifiedReadAuthority,
    },
    grpc_auth::{VerifiedCaller, Workload},
};
use post_application::concert_feed::Context;
use post_domain::concert_feed::Error;
use std::{collections::BTreeSet, time::Duration};
use uuid::Uuid;

pub(crate) struct ReadAdmission {
    pub context: Context,
    pub authority: proto::ReadAuthority,
    pub deadline: AuthorityRpcDeadline,
    pub operation: ReadOperation,
    pub target_post: Option<Uuid>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadOperation {
    Feed,
    Post,
    Comments,
}
impl ReadAdmission {
    /// Only the dedicated guarded server calls this after whole-envelope
    /// signature/replay/body/actor verification. A VerifiedCaller alone fails.
    pub(crate) fn from_verified(
        caller: VerifiedCaller,
        proof: VerifiedReadAuthority,
        context: Context,
        authority: proto::ReadAuthority,
    ) -> Result<Self, Error> {
        if caller.0 != Workload::Concert || proof.actor() != context.actor_id {
            return Err(Error::Authority);
        }
        context.validate(chrono::Utc::now())?;
        let binding = authority.binding.as_ref().ok_or(Error::Authority)?;
        let auth = authority.auth.as_ref().ok_or(Error::Authority)?;
        let friend = authority.friend.as_ref().ok_or(Error::Authority)?;
        let root = authority.root.as_ref().ok_or(Error::Authority)?;
        let scope = root.scope.as_ref().ok_or(Error::Authority)?;
        if binding.actor_id != context.actor_id.to_string()
            || binding.event_id != context.event_id.to_string()
            || binding.session_family_id != context.session_family_id.to_string()
            || binding.request_id != context.request_id.to_string()
            || binding.parent_digest.as_slice() != context.original_authority
            || binding.expires_unix_micros != context.expires_at.timestamp_micros()
            || binding.http_method != "GET"
            || root.binding.as_ref() != Some(binding)
            || scope.binding_digest != auth.binding_digest
            || auth.binding_digest != friend.binding_digest
            || !digest(&auth.binding_digest)
            || !digest(&root.view_secret)
            || !digest(&auth.view_secret)
            || !digest(&friend.view_secret)
            || auth.expires_unix_micros > binding.expires_unix_micros
            || friend.expires_unix_micros > binding.expires_unix_micros
            || !digest(&binding.parent_body_digest)
            || !digest(&binding.body_digest)
        {
            return Err(Error::Authority);
        }
        for value in [
            &auth.guard_id,
            &friend.guard_id,
            &scope.instance_id,
            &scope.scope_id,
        ] {
            canonical_id(value)?;
        }
        if binding.subject_ids.len() > 100 {
            return Err(Error::Authority);
        }
        let mut subjects = BTreeSet::new();
        for value in &binding.subject_ids {
            if !subjects.insert(canonical_id(value)?) {
                return Err(Error::Authority);
            }
        }
        let (operation, target_post) = target(
            binding.operation.as_str(),
            &binding.http_target,
            context.event_id,
        )?;
        let proof_deadline = proof
            .deadline(Duration::from_secs(5))
            .map_err(|_| Error::Authority)?;
        let deadline = AuthorityRpcDeadline::within_original(
            binding
                .expires_unix_micros
                .min(auth.expires_unix_micros)
                .min(friend.expires_unix_micros)
                .min(proof_deadline.expires_micros()),
            proof_deadline.stop().into_std(),
            Duration::from_secs(5),
        )
        .map_err(|_| Error::Authority)?;
        Ok(Self {
            context,
            authority,
            deadline,
            operation,
            target_post,
        })
    }
    pub(crate) fn check(&self) -> Result<(), Error> {
        self.context.validate(chrono::Utc::now())?;
        self.deadline
            .remaining(
                Duration::from_secs(5),
                self.context.expires_at.timestamp_micros(),
            )
            .map_err(|_| Error::Authority)?;
        Ok(())
    }
    pub(crate) fn parent(&self, post: Uuid) -> Result<(), Error> {
        if self.target_post.is_some_and(|target| target != post) {
            Err(Error::Forbidden)
        } else {
            Ok(())
        }
    }
}
fn digest(value: &[u8]) -> bool {
    value.len() == 32 && value.iter().any(|byte| *byte != 0)
}
fn canonical_id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or(Error::Authority)
}
fn target(
    operation: &str,
    target: &str,
    event: Uuid,
) -> Result<(ReadOperation, Option<Uuid>), Error> {
    if target.len() > 4096 || target.chars().any(char::is_control) || target.contains('#') {
        return Err(Error::Authority);
    }
    let path = target.split_once('?').map_or(target, |(path, _)| path);
    let prefix = format!("/api/concert/events/{event}/");
    let path = path.strip_prefix(&prefix).ok_or(Error::Authority)?;
    match operation {
        "listConcertFeed" if path == "feed" => Ok((ReadOperation::Feed, None)),
        "getConcertPost" if !target.contains('?') => {
            let post = path.strip_prefix("posts/").ok_or(Error::Authority)?;
            Ok((ReadOperation::Post, Some(canonical_id(post)?)))
        }
        "listConcertComments" => {
            let post = path
                .strip_prefix("posts/")
                .and_then(|v| v.strip_suffix("/comments"))
                .ok_or(Error::Authority)?;
            Ok((ReadOperation::Comments, Some(canonical_id(post)?)))
        }
        _ => Err(Error::Authority),
    }
}
