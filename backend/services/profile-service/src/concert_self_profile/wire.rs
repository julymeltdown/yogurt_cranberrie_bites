//! Local generated public contracts; no dependency on Concert implementation.
// Keep the package nesting required by Prost's imported-type references.
pub mod authority {
    pub mod lease {
        pub mod v1 {
            tonic::include_proto!("authority.lease.v1");
        }
    }
}
pub mod profile {
    pub mod self_profile {
        pub mod v1 {
            // Prost escapes the Rust keyword in its generated filename. The
            // protobuf package and RPC service name remain profile.self.v1.
            tonic::include_proto!("profile.self_.v1");
        }
    }
}
pub mod concert {
    pub mod participant {
        pub mod v1 {
            tonic::include_proto!("concert.participant.v1");
        }
    }
}
pub use authority::lease::v1 as lease;
pub use concert::participant::v1 as decision;
pub use profile::self_profile::v1 as proto;
