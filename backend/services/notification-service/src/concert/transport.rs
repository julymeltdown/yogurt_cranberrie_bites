//! Dedicated typed transport modules. Mounting and public operations remain
//! disabled until the actual parent capability, owner adapter and settlement
//! implementation are composed and qualified.
pub mod lease {
    include!(concat!(
        env!("OUT_DIR"),
        "/owner_authority/authority.lease.v1.rs"
    ));
}
pub mod proto {
    tonic::include_proto!("notification.concert.v1");
}
