fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto = contracts_build::source_profile_proto();
    let include_dir = contracts_build::source_profile_include_dir();
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                proto,
                contracts_build::source_authority_proto(),
                contracts_build::proto_root().join("profile/v1/concert_public_profile.proto"),
                contracts_build::proto_root().join("profile/v1/concert_self_profile.proto"),
                contracts_build::proto_root().join("authority/v1/lease.proto"),
                contracts_build::proto_root().join("concert/v1/participant.proto"),
            ],
            &[include_dir],
        )?;

    println!(
        "cargo:rerun-if-changed={}",
        contracts_build::source_profile_proto().display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        contracts_build::source_authority_proto().display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        contracts_build::proto_root()
            .join("profile/v1/concert_public_profile.proto")
            .display()
    );
    for relative in [
        "profile/v1/concert_self_profile.proto",
        "authority/v1/lease.proto",
        "concert/v1/participant.proto",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            contracts_build::proto_root().join(relative).display()
        );
    }
    Ok(())
}
