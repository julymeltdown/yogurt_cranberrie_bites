fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto = contracts_build::source_concert_proto();
    let relationship = contracts_build::proto_root().join("relationship/v1/relationship.proto");
    let profile = contracts_build::source_profile_proto();
    let world_proto = contracts_build::source_world_proto();
    let product_proto = contracts_build::proto_root().join("concert/v1/product.proto");
    let public_profile_proto =
        contracts_build::proto_root().join("profile/v1/concert_public_profile.proto");
    let self_profile_proto =
        contracts_build::proto_root().join("profile/v1/concert_self_profile.proto");
    // The profile service's internal discovery-source read. The projection
    // writer pulls changed profiles through it rather than joining across
    // databases (domain-contract.md §1.1).
    let discovery_source_proto =
        contracts_build::proto_root().join("profile/v1/discovery_source.proto");
    let post_concert_proto = contracts_build::proto_root().join("post/v1/concert_feed.proto");
    // The concert-side profile product transport. Fifteen frozen profile
    // operations read concert-owned tables, so this service serves them under
    // its own authority discipline; see `concert_service::profile_product`.
    let profile_product_proto =
        contracts_build::proto_root().join("concert/v1/profile_product.proto");
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    let lease_proto = contracts_build::proto_root().join("authority/v1/lease.proto");
    let authority_output =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("owner_authority");
    std::fs::create_dir_all(&authority_output)?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&authority_output)
        .compile_protos(&[lease_proto.clone()], &[contracts_build::proto_root()])?;
    println!("cargo:rerun-if-changed={}", lease_proto.display());
    let deletion_source_proto = contracts_build::proto_root().join("authority/v1/deletion_source.proto");
    let deletion_source_output =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("deletion_source");
    std::fs::create_dir_all(&deletion_source_output)?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&deletion_source_output)
        .compile_protos(
            &[deletion_source_proto.clone()],
            &[contracts_build::proto_root()],
        )?;
    println!("cargo:rerun-if-changed={}", deletion_source_proto.display());
    let deletion_apply_proto = contracts_build::proto_root().join("concert/v1/account_deletion.proto");
    let deletion_apply_output =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("account_deletion");
    std::fs::create_dir_all(&deletion_apply_output)?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&deletion_apply_output)
        .compile_protos(
            &[deletion_apply_proto.clone()],
            &[contracts_build::proto_root()],
        )?;
    println!("cargo:rerun-if-changed={}", deletion_apply_proto.display());
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .extern_path(
            ".authority.lease.v1",
            "crate::adapters::out::relationship_authority::guarded::proto",
        )
        .compile_protos(
            &[
                proto.clone(),
                world_proto.clone(),
                product_proto.clone(),
                post_concert_proto.clone(),
                relationship.clone(),
                profile.clone(),
                public_profile_proto.clone(),
                self_profile_proto.clone(),
                discovery_source_proto.clone(),
                profile_product_proto.clone(),
            ],
            &[contracts_build::proto_root()],
        )?;
    println!("cargo:rerun-if-changed={}", proto.display());
    println!("cargo:rerun-if-changed={}", world_proto.display());
    println!("cargo:rerun-if-changed={}", product_proto.display());
    println!("cargo:rerun-if-changed={}", public_profile_proto.display());
    println!("cargo:rerun-if-changed={}", self_profile_proto.display());
    println!("cargo:rerun-if-changed={}", discovery_source_proto.display());
    println!("cargo:rerun-if-changed={}", post_concert_proto.display());
    println!("cargo:rerun-if-changed={}", profile_product_proto.display());
    println!("cargo:rerun-if-changed={}", relationship.display());
    println!("cargo:rerun-if-changed={}", profile.display());
    let participant_proto = contracts_build::proto_root().join("concert/v1/participant.proto");
    let participant_output =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("participant");
    std::fs::create_dir_all(&participant_output)?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&participant_output)
        .extern_path(".concert.product.v1", "crate::product_rpc::proto")
        .extern_path(
            ".authority.lease.v1",
            "crate::adapters::out::relationship_authority::guarded::proto",
        )
        .compile_protos(
            &[participant_proto.clone()],
            &[contracts_build::proto_root()],
        )?;
    println!("cargo:rerun-if-changed={}", participant_proto.display());
    let notification_proto =
        contracts_build::proto_root().join("notification/v1/concert_inbox.proto");
    let delivery_proto =
        contracts_build::proto_root().join("notification/v1/concert_delivery.proto");
    let notification_output =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("notification_authority");
    std::fs::create_dir_all(&notification_output)?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&notification_output)
        .extern_path(
            ".authority.lease.v1",
            "crate::adapters::out::relationship_authority::guarded::proto",
        )
        .compile_protos(
            &[notification_proto.clone(), delivery_proto.clone()],
            &[contracts_build::proto_root()],
        )?;
    println!("cargo:rerun-if-changed={}", notification_proto.display());
    println!("cargo:rerun-if-changed={}", delivery_proto.display());
    Ok(())
}
