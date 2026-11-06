//! Qualified command admission is sealed by the dedicated whole-frame guard.
use super::{
    command_wire,
    read_admission::{ReadAdmission, ReadOperation},
};
use crate::adapters::r#in::grpc::concert_feed::proto;
use platform::{
    actor_delegation::read_authority::VerifiedReadAuthority,
    concert_feed_command::FeedCommandRoute, grpc_auth::VerifiedCaller,
};
use post_domain::concert_feed::{Command, Error};
use sha2::{Digest, Sha256};

pub(crate) struct CommandAdmission {
    pub owners: ReadAdmission,
    pub authority: proto::CommandAuthority,
    pub wire_command: proto::Command,
    pub command: Command,
}
impl CommandAdmission {
    pub(crate) fn from_verified(
        caller: VerifiedCaller,
        proof: VerifiedReadAuthority,
        query: proto::ExecuteCommandRequest,
        authority: Option<proto::CommandAuthority>,
    ) -> Result<Self, Error> {
        let authority = authority.ok_or(Error::Authority)?;
        let context = command_wire::context(query.context)?;
        let wire_command = query.command.ok_or(Error::Invalid)?;
        let command = command_wire::command(wire_command.clone())?;
        context.binds(&command)?;
        let owners = ReadAdmission::from_verified_owner(
            caller,
            proof,
            context,
            authority.owners.clone().ok_or(Error::Authority)?,
        )?;
        let binding = owners.authority.binding.as_ref().ok_or(Error::Authority)?;
        let route = FeedCommandRoute::parse(
            command.event_id,
            &authority.original_http_method,
            &authority.original_http_target,
        )
        .map_err(|_| Error::Authority)?;
        match owners.operation {
            ReadOperation::Command(actual) if actual == route => {
                if binding.body_digest.as_slice()
                    != Sha256::digest(&authority.original_body).as_slice()
                    || authority.original_http_target != binding.http_target
                    || authority.original_http_method != binding.http_method
                {
                    return Err(Error::Authority);
                }
            }
            _ => return Err(Error::Authority),
        }
        command_wire::intent(route, &command, &authority)?;
        owners.check()?;
        Ok(Self {
            owners,
            authority,
            wire_command,
            command,
        })
    }
}
