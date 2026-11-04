fn main() -> Result<(), Box<dyn std::error::Error>> {
    let relationship = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proto/relationship/v1/relationship.proto");
    let protos = vec![
        relationship,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../proto/authority/v1/lease.proto"),
        contracts_build::source_authority_proto(),
        contracts_build::source_friend_proto(),
        contracts_build::source_profile_proto(),
    ];
    let includes = vec![
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../proto"),
        contracts_build::source_friend_include_dir(),
        contracts_build::source_profile_include_dir(),
    ];
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &includes)?;
    for proto in protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }
    Ok(())
}
