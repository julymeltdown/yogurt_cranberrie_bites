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
    let post_concert_proto = contracts_build::proto_root().join("post/v1/concert_feed.proto");
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
            ],
            &[contracts_build::proto_root()],
        )?;
    println!("cargo:rerun-if-changed={}", proto.display());
    println!("cargo:rerun-if-changed={}", world_proto.display());
    println!("cargo:rerun-if-changed={}", product_proto.display());
    println!("cargo:rerun-if-changed={}", public_profile_proto.display());
    println!("cargo:rerun-if-changed={}", self_profile_proto.display());
    println!("cargo:rerun-if-changed={}", post_concert_proto.display());
    println!("cargo:rerun-if-changed={}", relationship.display());
    println!("cargo:rerun-if-changed={}", profile.display());
    let participant_proto = contracts_build::proto_root().join("concert/v1/participant.proto");
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
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
    Ok(())
}
